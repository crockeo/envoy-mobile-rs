//! Provides a async/await based interface to interact with envoy-mobile, rather than with
//! callbacks.

use std::marker::PhantomData;

use super::bridge_util::{Data, HTTPError, Headers};

// TODO: import these when i'm not in the air & can download from crates.io
// use futures::{Future, Stream};
// TODO: delete these after doing the above
struct Future<T> {
    _t: PhantomData<T>,
}
struct Stream<T> {
    _t: PhantomData<T>,
}

/// A context to be passed into the an [super::engine::EngineBuilder]
struct EngineFuturesContext {}

impl EngineFuturesContext {
    fn on_engine_running(&self) -> Future<()> {
        todo!()
    }
    fn on_exit(&self) -> Future<()> {
        todo!()
    }
}

struct StreamFuturesContext {}

impl StreamFuturesContext {
    fn on_headers(&self) -> Stream<Headers> {
        todo!()
    }
    fn on_data(&self) -> Stream<Data> {
        todo!()
    }
    fn on_metadata(&self) -> Future<Headers> {
        todo!()
    }
    fn on_trailers(&self) -> Future<Headers> {
        todo!()
    }
    fn on_error(&self) -> Future<HTTPError> {
        todo!()
    }
    fn on_complete(&self) -> Future<()> {
        todo!()
    }
    fn on_cancel(&self) -> Future<()> {
        todo!()
    }
}
