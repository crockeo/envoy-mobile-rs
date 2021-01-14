mod bridge_util;
mod engine;
mod log_level;
mod result;
mod stream;

use std::sync::{Arc, Condvar, Mutex};

use bridge_util::Headers;
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

struct StreamContext {}

impl StreamContext {
    fn new() -> Self {
        Self {}
    }
}

fn main() -> Result<()> {
    let context = Arc::new(NullCtx::default());

    let engine = EngineBuilder::<NullCtx>::new(context.clone(), LogLevel::Info)
        .add_on_engine_running(|context| context.running.notify_one())
        .build()?;

    {
        let running_lock = context.running_lock.lock().unwrap();
        let _ = context.running.wait(running_lock).unwrap();
    }

    let stream_context = Arc::new(StreamContext::new());
    let stream = engine
        .stream_builder(stream_context)
        .set_on_headers(|context, headers, end_stream| todo!())
        .set_on_data(|context, data, end_stream| todo!())
        .set_on_error(|context, error| todo!())
        .set_on_complete(|context| todo!())
        .set_on_cancel(|context| todo!())
        .build()?;

    stream.send_headers(
        Headers::new()
            .add(":method", "GET")
            .add(":scheme", "https")
            .add(":authority", "www.google.com")
            .add(":path", "/"),
        true,
    )?;

    engine.terminate();

    Ok(())
}
