mod engine;
mod log_level;
mod result;

use std::sync::{Arc, Condvar, Mutex};

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

fn main() -> Result<()> {
    let context = Arc::new(NullCtx::default());

    let engine = EngineBuilder::<NullCtx>::new(context.clone(), LogLevel::Info)
        .add_on_engine_running(on_engine_running)
        .build()?;

    {
        let running_lock = context.running_lock.lock().unwrap();
        let _ = context.running.wait(running_lock).unwrap();
    }

    engine.terminate();

    Ok(())
}

fn on_engine_running(context: &Arc<NullCtx>) {
    context.running.notify_one();
}
