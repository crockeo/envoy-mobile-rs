mod bridge;
mod event;
mod sys;

use std::net::IpAddr;
use std::sync::Arc;

pub use bridge::{
    Data, EventTrackerTrack, Headers, HistogramStatUnit, LogLevel, LoggerLog, StatsTags,
};

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

pub struct Engine {
    engine: bridge::Engine,
    context: EngineContext,
}

impl Engine {
    pub fn new_stream(&'_ self, explicit_flow_control: bool) -> Stream<'_> {
        let stream_context = StreamContext::new();
        Stream {
            stream: self
                .engine
                .new_stream(stream_context.clone())
                .with_on_headers(|ctx, headers, end_stream, stream_intel| todo!())
                .with_on_data(|ctx, data, end_stream, stream_intel| todo!())
                .with_on_metadata(|ctx, metadata, stream_intel| todo!())
                .with_on_trailers(|ctx, trailers, stream_intel| todo!())
                .with_on_error(|ctx, error, stream_intel| todo!())
                .with_on_complete(|ctx, stream_intel| todo!())
                .with_on_cancel(|ctx, stream_intel| todo!())
                .start(explicit_flow_control),
        }
    }

    pub fn record_counter_inc<S: AsRef<str>>(&self, elements: S, tags: StatsTags, count: usize) {
        self.engine.record_counter_inc(elements, tags, count)
    }

    pub fn record_gauge_set<S: AsRef<str>>(&self, elements: S, tags: StatsTags, value: usize) {
        self.engine.record_gauge_set(elements, tags, value)
    }

    pub fn record_gauge_add<S: AsRef<str>>(&self, elements: S, tags: StatsTags, amount: usize) {
        self.engine.record_gauge_add(elements, tags, amount)
    }

    pub fn record_gauge_sub<S: AsRef<str>>(&self, elements: S, tags: StatsTags, amount: usize) {
        self.engine.record_gauge_sub(elements, tags, amount)
    }

    pub fn record_histogram_value<S: AsRef<str>>(
        &self,
        elements: S,
        tags: StatsTags,
        amount: usize,
        unit_measure: HistogramStatUnit,
    ) {
        self.engine
            .record_histogram_value(elements, tags, amount, unit_measure)
    }

    pub fn flush_stats(&self) {
        self.engine.flush_stats()
    }

    pub fn dump_stats(&self) -> Data {
        self.engine.dump_stats()
    }

    pub fn terminate(self) -> event::EventFuture<()> {
        self.engine.terminate();
        event::EventFuture::new((), self.context.engine_terminated)
    }
}

#[derive(Clone)]
struct StreamContext {}

impl StreamContext {
    fn new() -> Self {
        StreamContext {}
    }
}

pub struct Stream<'a> {
    stream: bridge::Stream<'a>,
}

impl Stream<'_> {
    pub fn send_headers<T: Into<Headers>>(&mut self, headers: T, end_stream: bool) {
        self.stream.send_headers(headers, end_stream)
    }

    pub fn send_data<T: Into<Data>>(&mut self, data: T, end_stream: bool) {
        self.stream.send_data(data, end_stream)
    }

    pub fn send_metadata<T: Into<Headers>>(&mut self, metadata: T) {
        self.stream.send_metadata(metadata)
    }

    pub fn send_trailers<T: Into<Headers>>(&mut self, trailers: T) {
        self.stream.send_trailers(trailers)
    }

    pub fn read_data(&mut self, bytes_to_read: usize) {
        self.stream.read_data(bytes_to_read)
    }
}

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
