mod bridge;
mod event;
mod sys;

use std::sync::Arc;

pub use bridge::LogLevel;

pub struct EngineBuilder {
    builder: bridge::EngineBuilder<EngineContext>,
    context: EngineContext,
}

impl EngineBuilder {
    pub fn new() -> Self {
        let context = EngineContext::new();
        Self {
            builder: bridge::EngineBuilder::new(context.clone())
                .with_on_engine_running(|ctx| ctx.engine_running.set())
                .with_on_exit(|ctx| ctx.engine_terminated.set()),
            context,
        }
    }

    pub fn build(self, log_level: LogLevel) -> event::EventFuture<Engine> {
	event::EventFuture::new(
	    Engine {
		engine: self.builder.build(log_level),
		context: self.context.clone(),
	    },
	    self.context.engine_running,
	)
    }
}

#[derive(Clone)]
struct EngineContext {
    engine_running: Arc<event::Event>,
    engine_terminated: Arc<event::Event>,
}

impl EngineContext {
    fn new() -> Self {
        Self {
            engine_running: Arc::new(event::Event::new()),
            engine_terminated: Arc::new(event::Event::new()),
        }
    }
}

pub struct Engine {
    engine: bridge::Engine,
    context: EngineContext,
}

impl Engine {
    pub fn terminate(self) -> event::EventFuture<()> {
	self.engine.terminate();
	event::EventFuture::new((), self.context.engine_terminated)
    }
}

struct StreamContext {
}

pub struct Stream {}

#[cfg(test)]
mod tests {
    use tokio::test;

    use super::*;

    #[test]
    async fn engine_lifecycle() {
        let engine = EngineBuilder::new().build(LogLevel::Debug).await;
        engine.terminate().await;
    }
}
