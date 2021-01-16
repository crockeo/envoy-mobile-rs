mod bridge_util;
mod engine;
mod log_level;
mod result;
mod stream;

use std::sync::{Arc, Condvar, Mutex};

use bridge_util::{Data, Headers};
use engine::EngineBuilder;
use log_level::LogLevel;
use result::Result;

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

fn main() -> Result<()> {
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
        .set_on_headers(|_, _, _| {
            println!("headers");
        })
        .set_on_data(|_, data, _| {
            if let Ok(s) = data.as_str() {
                println!("{}", s);
            }
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
        Headers::new()
            .add(":method", "GET")
            .add(":scheme", "https")
            .add(":authority", "www.google.com")
            .add(":path", "/"),
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
