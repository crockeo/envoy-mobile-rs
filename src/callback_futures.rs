use futures::future::FusedFuture;
use futures::task::{Context, Poll, Waker};
use futures::{Future, Stream};

use std::mem;
use std::pin::Pin;
use std::sync::Mutex;

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

impl<T> CallbackFuture<EnvoyResult<T>> {
    pub fn put_and_report(&self, new_value: EnvoyResult<T>) {
        if let Err(e) = self.put(new_value) {
            self.put(Err(e)).expect("put_and_report failed to report");
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
    values: Vec<Option<T>>,
    closed: bool,
    index: usize,
    waker: Option<Waker>,
}

pub struct CallbackStream<T> {
    state: Mutex<CallbackStreamState<T>>,
}

impl<T> CallbackStream<T> {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(CallbackStreamState {
                values: Vec::new(),
                closed: false,
                index: 0,
                waker: None,
            }),
        }
    }

    pub fn put(&self, value: T) -> EnvoyResult<()> {
        let mut state = self.state.lock().unwrap();
        if state.closed {
            return Err(EnvoyError::AlreadyClosed);
        }
        state.values.push(Some(value));
        if let Some(waker) = &state.waker {
            waker.wake_by_ref();
        }
        Ok(())
    }

    pub fn close(&self) -> EnvoyResult<()> {
        let mut state = self.state.lock().unwrap();
        if state.closed {
            return Err(EnvoyError::AlreadyClosed);
        }
        state.closed = true;
        if let Some(waker) = &state.waker {
            waker.wake_by_ref();
        }
        Ok(())
    }

    pub fn maybe_close(&self) {
        let mut state = self.state.lock().unwrap();
        let was_closed = state.closed;
        state.closed = true;
        if !was_closed {
            if let Some(waker) = &state.waker {
                waker.wake_by_ref();
            }
        }
    }

    fn poll_next_impl(&self, ctx: &mut Context<'_>) -> Poll<Option<T>> {
        let mut state = self.state.lock().unwrap();
        if state.index >= state.values.len() {
            if state.closed {
                Poll::Ready(None)
            } else {
                state.waker = Some(ctx.waker().clone());
                Poll::Pending
            }
        } else {
            let index = state.index;
            state.index += 1;
            let value = mem::replace(state.values.get_mut(index).unwrap(), None).unwrap();
            Poll::Ready(Some(value))
        }
    }
}

impl<T> CallbackStream<EnvoyResult<T>> {
    pub fn put_and_report(&self, value: EnvoyResult<T>) {
        if let Err(e) = self.put(value) {
            self.put(Err(e))
                .expect("failed to report in CallbackStream::put_and_report");
        }
    }
    pub fn close_and_report(&self) {
        if let Err(e) = self.close() {
            self.put(Err(e))
                .expect("failed to report in CallbackStream::close_and_report");
        }
    }
}

impl<T> Stream for CallbackStream<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_next_impl(ctx)
    }
}

impl<T> Stream for &CallbackStream<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_next_impl(ctx)
    }
}
