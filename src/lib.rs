use {
    std::{
        future::Future,
        sync::{Condvar, Mutex, Arc},
        task::{Wake, Waker, Context, Poll},
        cell::RefCell,
        collections::VecDeque,
    },
    futures::future::BoxFuture,
};

#[macro_use]
extern crate scoped_tls;

// Runnable queue中存放的是Task，一个Task对应一个Future
struct Task {
    future: RefCell<BoxFuture<'static, ()>>,
    signal: Arc<Signal>,        // Signal由同一executor上的所有Task共享
}

unsafe impl Send for Task {}
unsafe impl Sync for Task {}

scoped_thread_local!(static SIGNAL: Arc<Signal>);
// RUNNABLE中存放所有spawn出的从future
scoped_thread_local!(static RUNNABLE: Mutex<VecDeque<Arc<Task>>>);

// 为Task实现Wake后，就不需要为Signal实现Wake了
impl Wake for Task {
    fn wake(self: Arc<Self>) {
        RUNNABLE.with(|runnable|
            runnable.lock().unwrap().push_back(self.clone())
        );
        self.signal.notify();
    }
}


// State of Executor
struct Signal {
    state: Mutex<State>,        // executor的状态
    // executor没有可poll的future时，wait该条件变量
    // 外部通过条件变量来唤醒等待的executor
    cond: Condvar,               
}

#[derive(Debug)]
enum State {
    Empty,
    Waiting,
    Notified,
}

impl Signal {
    fn new() -> Self {
        Signal {
            state: Mutex::new(State::Empty),
            cond: Condvar::new(),
        }
    }

    // 当executor没有可poll的future时，调用Signal::wait让出CPU
    fn wait(&self) {
        eprintln!("Signal::wait");
        let mut state = self.state.lock().unwrap();
        match *state {
            State::Empty => {
                *state = State::Waiting;
                while let State::Waiting = *state {
                    state = self.cond.wait(state).unwrap();
                }
            },
            State::Waiting => {
                panic!("multiple wait");
            },
            State::Notified => {
                *state = State::Empty;
            }
        }
    }

    // 外部调用notify来唤醒executor
    fn notify(&self) {
        eprintln!("Signal::notify");
        let mut state = self.state.lock().unwrap();
        eprintln!("state before notify: {:?}", state);
        match *state {
            State::Empty => {
                *state = State::Notified;
            },
            State::Waiting => {
                *state = State::Empty;
                self.cond.notify_one();
            },
            State::Notified => {}
        }
    }
}

// Signal可以作为Waker使用，对Waker::wake()最终落到Signal::notify
impl Wake for Signal {
    fn wake(self: Arc<Self>) {
        // 当前executor只支持运行单个future，因此不需要维护runnable队列，这里只需要notify
        eprintln!("Signal::wake");
        self.notify();
    }
}

// Executor
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut main_fut = std::pin::pin!(future);
    let signal = Arc::new(Signal::new());
    let waker = Waker::from(signal.clone());
    let mut cx = Context::from_waker(&waker);
    let runnable : Mutex<VecDeque<Arc<Task>>> = Mutex::new(VecDeque::with_capacity(1024));
    SIGNAL.set(&signal, || {
        RUNNABLE.set(&runnable, || {
            loop {
                if let Poll::Ready(output) = main_fut.as_mut().poll(&mut cx) {
                    return output;
                }
                while let Some(task) = runnable.lock().unwrap().pop_front() {
                    let waker = Waker::from(task.clone());
                    let mut cx = Context::from_waker(&waker);
                    let _ = task.future.borrow_mut().as_mut().poll(&mut cx);
                }
                signal.wait();
            }
        })
    })
}

pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + 'static + Send,
{ 
    let boxed_future = Box::pin(future);
    SIGNAL.with(move |signal| {
        RUNNABLE.with(move |runnable| {
            let t = Task {
                future: RefCell::new(boxed_future),
                signal: signal.clone()
            };
            runnable.lock().unwrap().push_back(Arc::new(t));
            signal.notify();
        })
    });
}