mod bridge_util;
mod engine;
#[cfg(feature = "use-futures")]
mod futures;
mod headers;
mod log_level;
mod request_method;
mod result;
mod scheme;
mod stream;

use std::sync::{Arc, Condvar, Mutex};

use engine::EngineBuilder;
use headers::RequestHeaders;
use log_level::LogLevel;
use request_method::RequestMethod;
use result::EnvoyResult;
use scheme::Scheme;

struct NullCtx {
    running: Condvar,
    running_lock: Mutex<()>,
}

impl Default for NullCtx {
    fn default() -> Self {
        Self {
            running: Condvar::new(),
            running_lock: Mutex::new(()),
        }
    }
}

struct StreamContext {
    complete: Mutex<bool>,
    complete_cond: Condvar,
}

impl StreamContext {
    fn new() -> Self {
        Self {
            complete: Mutex::new(false),
            complete_cond: Condvar::new(),
        }
    }
}

fn main() -> EnvoyResult<()> {
    let context = Arc::new(NullCtx::default());

    let engine = EngineBuilder::<NullCtx>::new(context.clone(), LogLevel::Error)
        .add_on_engine_running(|context| context.running.notify_one())
        .build()?;

    {
        let running_lock = context.running_lock.lock().unwrap();
        let _ = context.running.wait(running_lock).unwrap();
    }

    let stream_context = Arc::new(StreamContext::new());
    let mut stream = engine
        .stream_builder(stream_context.clone())
        .set_on_headers(|_, headers, _| {
            for (key, value) in headers.into_iter() {
                println!("{}: {}", key, value);
            }
        })
        .set_on_data(|_, data, _| {
            if let Ok(s) = data.as_str() {
                println!("{}", s);
            }
        })
        .set_on_metadata(|_, metadata| {
            println!("metadata: {:?}", metadata);
        })
        .set_on_trailers(|_, trailers| {
            println!("trailers: {:?}", trailers);
        })
        .set_on_error(|context, _| {
            println!("error");
            set_complete(context);
        })
        .set_on_complete(|context| {
            println!("complete");
            set_complete(context);
        })
        .set_on_cancel(|context| {
            println!("cancel");
            set_complete(context);
        })
        .build()?;

    stream.send_headers(
        RequestHeaders::new()
            .add_method(RequestMethod::GET)
            .add_scheme(Scheme::HTTPS)
            .add_authority("www.delta.com")
            .add_path("/")
            .as_headers(),
        true,
    )?;

    {
        let complete = stream_context.complete.lock().unwrap();
        let _ = stream_context.complete_cond.wait(complete);
    }

    engine.terminate();

    Ok(())
}

fn set_complete(context: &Arc<StreamContext>) {
    let mut complete = context.complete.lock().unwrap();
    *complete = true;
    context.complete_cond.notify_one();
}
