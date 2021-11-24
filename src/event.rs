use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Waker};

// TODO: introducing the waker here makes things more complicated
// and i'm not sure that everything is water tight
// e.g. what happens if two different calls try to wait on the same event?

pub struct Event {
    occurred: Mutex<bool>,
    condvar: Condvar,
    waker: Mutex<Option<Waker>>,
}

impl Event {
    pub fn new() -> Self {
        Self {
            occurred: Mutex::new(false),
            condvar: Condvar::new(),
            waker: Mutex::new(None),
        }
    }

    pub fn set(&self) {
        let mut guard = self.occurred.lock().unwrap();
        if !*guard {
            (*guard) = true;
            self.condvar.notify_all();

            let mut waker_guard = self.waker.lock().unwrap();
            if let Some(waker) = waker_guard.take() {
                waker.wake();
            }
        }
    }

    pub fn is_set(&self) -> bool {
        let guard = self.occurred.lock().unwrap();
        *guard
    }

    pub fn wait(&self) {
        let guard = self.occurred.lock().unwrap();
        let _ = self.condvar.wait_while(guard, |occurred| !(*occurred));
    }

    pub fn set_waker(&self, waker: Waker) {
        let mut guard = self.waker.lock().unwrap();
        (*guard) = Some(waker);
    }
}

pub struct EventFuture<T> {
    value: Option<T>,
    occurred: Arc<Event>,
}

impl<T> EventFuture<T> {
    pub fn new(value: T, occurred: Arc<Event>) -> Self {
        EventFuture {
            value: Some(value),
            occurred,
        }
    }
}

impl<T: Unpin> Future for EventFuture<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.occurred.is_set() {
            Poll::Ready(
                self.value
                    .take()
                    .expect("FutureValue must not be polled after marking itself as Ready"),
            )
        } else {
            self.occurred.set_waker(ctx.waker().clone());
            Poll::Pending
        }
    }
}
