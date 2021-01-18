use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

pub struct CallbackFuture<T> {
    value_waker: Mutex<(Option<T>, Option<Waker>)>,
}

impl<T> CallbackFuture<T> {
    pub fn new() -> Self {
        Self {
            value_waker: Mutex::new((None, None)),
        }
    }

    pub fn put(&self, new_value: T) {
        let mut value_waker = self.value_waker.lock().unwrap();
        value_waker.0 = Some(new_value);
        if let Some(waker) = &value_waker.1 {
            waker.wake_by_ref();
        }
    }
}

impl<T: Clone> Future for CallbackFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut value_waker = self.value_waker.lock().unwrap();
        if let Some(value) = &value_waker.0 {
            Poll::Ready(value.clone())
        } else {
            value_waker.1 = Some(ctx.waker().clone());
            Poll::Pending
        }
    }
}

impl<'a, T: Clone> Future for &'a CallbackFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut value_waker = self.value_waker.lock().unwrap();
        if let Some(value) = &value_waker.0 {
            Poll::Ready(value.clone())
        } else {
            value_waker.1 = Some(ctx.waker().clone());
            Poll::Pending
        }
    }
}
