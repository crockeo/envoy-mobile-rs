use std::{
    future::Future,
    pin::Pin,
    sync::Mutex,
    task::{Context, Poll, Waker},
};

pub struct Channel<T> {
    contents: Mutex<(Vec<Option<T>>, Vec<Waker>)>,
}

impl<T> Default for Channel<T> {
    fn default() -> Self {
        Self {
	    contents: Mutex::default(),
        }
    }
}

impl<T> Channel<T> {
    fn put_impl(&self, value: Option<T>) {
	let mut guard = self.contents.lock().unwrap();
	guard.0.push(value);
	if guard.1.len() > 0 {
	    guard.1.remove(0).wake();
	}
    }

    fn try_recv(&self, waker: Waker) -> Result<Option<T>, ()> {
	let mut guard = self.contents.lock().unwrap();
	if guard.0.len() == 0 {
	    guard.1.push(waker);
	    Err(())
	} else {
	    Ok(guard.0.remove(0))
	}
    }

    pub fn put(&self, value: T) {
        self.put_impl(Some(value))
    }

    pub fn close(&self) {
        self.put_impl(None)
    }

    pub fn poll(&self) -> ChannelFuture<'_, T> {
        ChannelFuture(self)
    }
}

pub struct ReadOnlyChannel<'a, T>(&'a Channel<T>);

impl<'a, T> ReadOnlyChannel<'a, T> {
    pub fn new(channel: &'a Channel<T>) -> Self {
        Self(channel)
    }

    pub fn poll(&self) -> ChannelFuture<'_, T> {
        self.0.poll()
    }
}

pub struct ChannelFuture<'a, T>(&'a Channel<T>);

impl<'a, T> Future for ChannelFuture<'a, T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
	match self.0.try_recv(ctx.waker().clone()) {
	    Ok(value) => Poll::Ready(value),
	    Err(_) => Poll::Pending,
	}
    }
}
