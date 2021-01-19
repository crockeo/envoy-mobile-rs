use futures::future::Future;
use futures::task::{Context, Poll, Waker};

use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::result::{EnvoyError, EnvoyResult};

pub struct CallbackFuture<T> {
    value_waker: Mutex<(Option<T>, Option<Waker>)>,
}

impl<T> CallbackFuture<T> {
    pub fn new() -> Self {
        Self {
            value_waker: Mutex::new((None, None)),
        }
    }

    pub fn put(&self, new_value: T) -> EnvoyResult<()> {
        let mut value_waker = self.value_waker.lock().unwrap();
        if value_waker.0.is_some() {
            return Err(EnvoyError::AlreadyClosed);
        }
        value_waker.0 = Some(new_value);
        if let Some(waker) = &value_waker.1 {
            waker.wake_by_ref();
        }
        Ok(())
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

struct CallbackStreamState<T> {
    values: Vec<T>,
    index: usize,
    closed: bool,
    waker: Option<Waker>,
}

impl<T> CallbackStreamState<T> {
    fn new() -> Self {
        Self {
            values: Vec::new(),
            index: 0,
            closed: false,
            waker: None,
        }
    }
}

pub struct CallbackStream<T> {
    state: Arc<Mutex<CallbackStreamState<T>>>,
}

impl<T> CallbackStream<T> {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(CallbackStreamState::new())),
        }
    }

    pub fn put(&self, new_value: T) -> EnvoyResult<()> {
        let mut state = self.state.lock().unwrap();
        if state.closed {
            Err(EnvoyError::AlreadyClosed)
        } else {
            state.values.push(new_value);
            Ok(())
        }
    }

    pub fn close(&self) -> EnvoyResult<()> {
        let mut state = self.state.lock().unwrap();
        if state.closed {
            Err(EnvoyError::AlreadyClosed)
        } else {
            state.closed = true;
            Ok(())
        }
    }
}

impl<T> Future for CallbackStream<T> {
    type Output = CallbackStreamResult<T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        // TODO: try to come up with a way to fan out pieces of the future as they become
        // available, instead of only handing the whole thing out when it's done
        let mut state = self.state.lock().unwrap();
        if state.closed {
            Poll::Ready(CallbackStreamResult::new(self.state.clone()))
        } else {
            state.waker = Some(ctx.waker().clone());
            Poll::Pending
        }
    }
}

pub struct CallbackStreamResult<T> {
    state: Arc<Mutex<CallbackStreamState<T>>>,
}

impl<T> CallbackStreamResult<T> {
    fn new(state: Arc<Mutex<CallbackStreamState<T>>>) -> Self {
        Self { state }
    }
}

impl<T> Iterator for CallbackStreamResult<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}
