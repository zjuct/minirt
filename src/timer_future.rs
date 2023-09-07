use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    thread,
    time::Duration,
};

pub struct TimerFuture {
    shared_state: Arc<Mutex<SharedState>>,
}

struct SharedState {
    completed: bool,
    waker: Option<Waker>,
}

impl Future for TimerFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut ss = self.shared_state.lock().unwrap();
        if ss.completed {
            Poll::Ready(())
        } else {
            ss.waker = Some(cx.waker().clone());
            Poll::Pending 
        }
    }
}

impl TimerFuture {
    pub fn new(duration: Duration) -> Self {
        let ss = Arc::new(Mutex::new(SharedState {
            completed: false,
            waker: None,
        }));

        let thread_shared_state = ss.clone();
        thread::spawn(move || {
            thread::sleep(duration);
            println!("sleep end");
            let mut ss = thread_shared_state.lock().unwrap();
            ss.completed = true;
            if let Some(waker) = ss.waker.take() {
                println!("wake");
                waker.wake()
            }
        });

        TimerFuture { shared_state: ss }
    }
}
