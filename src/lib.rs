mod bridge;
mod event;
mod sys;

use std::net::IpAddr;
use std::sync::Arc;

pub use bridge::{EventTrackerTrack, LogLevel, LoggerLog};

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

    pub fn with_log(mut self, log: LoggerLog) -> Self {
        self.builder = self.builder.with_log(log);
        self
    }

    pub fn with_track(mut self, track: EventTrackerTrack) -> Self {
        self.builder = self.builder.with_track(track);
        self
    }

    pub fn with_connect_timeout_seconds(mut self, connect_timeout_seconds: usize) -> Self {
        self.builder = self
            .builder
            .with_connect_timeout_seconds(connect_timeout_seconds);
        self
    }

    pub fn with_dns_refresh_rate_seconds(mut self, dns_refresh_rate_seconds: usize) -> Self {
        self.builder = self
            .builder
            .with_dns_refresh_rate_seconds(dns_refresh_rate_seconds);
        self
    }

    pub fn with_dns_fail_interval(
        mut self,
        dns_fail_base_interval_seconds: usize,
        dns_fail_max_interval_seconds: usize,
    ) -> Self {
        self.builder = self.builder.with_dns_fail_interval(
            dns_fail_base_interval_seconds,
            dns_fail_max_interval_seconds,
        );
        self
    }

    pub fn with_dns_query_timeout_seconds(mut self, dns_query_timeout_seconds: usize) -> Self {
        self.builder = self
            .builder
            .with_dns_query_timeout_seconds(dns_query_timeout_seconds);
        self
    }

    pub fn with_enable_interface_binding(mut self, enable_interface_binding: bool) -> Self {
        self.builder = self
            .builder
            .with_enable_interface_binding(enable_interface_binding);
        self
    }

    pub fn with_h2_connection_keepalive_idle_interval_seconds(
        mut self,
        h2_connection_keepalive_idle_interval_seconds: usize,
    ) -> Self {
        self.builder = self
            .builder
            .with_h2_connection_keepalive_idle_interval_seconds(
                h2_connection_keepalive_idle_interval_seconds,
            );
        self
    }

    pub fn with_h2_connection_keepalive_timeout(
        mut self,
        h2_connection_keepalive_timeout: usize,
    ) -> Self {
        self.builder = self
            .builder
            .with_h2_connection_keepalive_timeout(h2_connection_keepalive_timeout);
        self
    }

    pub fn with_stats_domain(mut self, stats_domain: IpAddr) -> Self {
        self.builder = self.builder.with_stats_domain(stats_domain);
        self
    }

    pub fn with_stats_flush_interval_seconds(
        mut self,
        stats_flush_interval_seconds: usize,
    ) -> Self {
        self.builder = self
            .builder
            .with_stats_flush_interval_seconds(stats_flush_interval_seconds);
        self
    }

    pub fn with_statsd_host(mut self, statsd_host: IpAddr) -> Self {
        self.builder = self.builder.with_statsd_host(statsd_host);
        self
    }

    pub fn with_statsd_port(mut self, statsd_port: u16) -> Self {
        self.builder = self.builder.with_statsd_port(statsd_port);
        self
    }

    pub fn with_stream_idle_timeout_seconds(mut self, stream_idle_timeout_seconds: usize) -> Self {
        self.builder = self
            .builder
            .with_stream_idle_timeout_seconds(stream_idle_timeout_seconds);
        self
    }

    pub fn with_per_try_idle_timeout_seconds(
        mut self,
        per_try_idle_timeout_seconds: usize,
    ) -> Self {
        self.builder = self
            .builder
            .with_per_try_idle_timeout_seconds(per_try_idle_timeout_seconds);
        self
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

struct StreamContext {}

pub struct Stream {}

#[cfg(test)]
mod tests {
    use tokio::test;

    use super::*;

    #[test]
    async fn engine_lifecycle() {
        let engine = EngineBuilder::new()
            .with_log(Box::new(|data| {
                print!("{}", String::try_from(data).unwrap());
            }))
            .build(LogLevel::Debug)
            .await;
        engine.terminate().await;
    }
}
