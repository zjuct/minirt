use {
    std::{
        future::Future,
        sync::{Condvar, Mutex, Arc},
        task::{Wake, Waker, Context, Poll},
        cell::RefCell,
    },
    futures::{
        task::waker,
        future::BoxFuture,
    },
};

// Runnable queue中存放的是Task，一个Task对应一个Future
struct Task {
    future: RefCell<BoxFuture<'static, ()>>,
    signal: Arc<Signal>,        // Signal由同一executor上的所有Task共享
}

unsafe impl Send for Task {}
unsafe impl Sync for Task {}

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
    let mut fut = std::pin::pin!(future);
    let signal = Arc::new(Signal::new());
    let waker = Waker::from(signal.clone());

    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(output) = fut.as_mut().poll(&mut cx) {
            return output;
        }
        signal.wait();
    }
}
