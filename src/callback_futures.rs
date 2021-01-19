use futures::future::{FusedFuture, Future};
use futures::task::{Context, Poll, Waker};

use std::mem;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::result::{EnvoyError, EnvoyResult};

struct CallbackFutureState<T> {
    value: Option<T>,
    waker: Option<Waker>,
    sent: bool,
}

pub struct CallbackFuture<T> {
    state: Mutex<CallbackFutureState<T>>,
}

impl<T> CallbackFuture<T> {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(CallbackFutureState {
                value: None,
                waker: None,
                sent: false,
            }),
        }
    }

    pub fn put(&self, new_value: T) -> EnvoyResult<()> {
        let mut state = self.state.lock().unwrap();
        if state.value.is_some() || state.sent {
            return Err(EnvoyError::AlreadyClosed);
        }

        state.value = Some(new_value);
        if let Some(waker) = &state.waker {
            waker.wake_by_ref();
        }
        Ok(())
    }

    fn poll_impl(&self, ctx: &mut Context<'_>) -> Poll<T> {
        let mut state = self.state.lock().unwrap();
        if state.sent {
            panic!("attempting to re-consume future output after it was sent");
        }

        let value = mem::replace(&mut state.value, None);
        if let Some(value) = value {
            state.sent = true;
            Poll::Ready(value)
        } else {
            state.waker = Some(ctx.waker().clone());
            Poll::Pending
        }
    }
}

impl<T> Future for CallbackFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_impl(ctx)
    }
}

impl<T> Future for &CallbackFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_impl(ctx)
    }
}

impl<T> FusedFuture for CallbackFuture<T> {
    fn is_terminated(&self) -> bool {
        self.state.lock().unwrap().sent
    }
}

impl<T> FusedFuture for &CallbackFuture<T> {
    fn is_terminated(&self) -> bool {
        self.state.lock().unwrap().sent
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
