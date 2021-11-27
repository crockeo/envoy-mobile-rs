use std::{
    future::Future,
    pin::Pin,
    sync::{mpsc, Mutex},
    task::{Context, Poll, Waker},
};

pub struct Channel<T> {
    tx: mpsc::Sender<Option<T>>,
    rx: mpsc::Receiver<Option<T>>,
    wakers: Mutex<Vec<Waker>>,
}

impl<T> Default for Channel<T> {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            wakers: Mutex::new(Vec::new()),
        }
    }
}

impl<T> Channel<T> {
    fn put_impl(&self, value: Option<T>) {
	let mut wakers_guard = self.wakers.lock().unwrap();
        self.tx.send(value).expect("failed to send over Channel");
        if wakers_guard.len() > 0 {
            wakers_guard.remove(0).wake();
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

    fn with_wakers<U>(&self, func: impl FnOnce(&mut Vec<Waker>) -> U) -> U {
	let mut guard = self.wakers.lock().unwrap();
	func(&mut *guard)
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
	self.0.with_wakers(|wakers| {
	    match self.0.rx.try_recv() {
		Ok(value) => Poll::Ready(value),
		Err(_) => {
		    wakers.push(ctx.waker().clone());
		    Poll::Pending
		}
	    }
	})
    }
}
