mod data;
mod map;

use std::ffi;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

use crate::channel::{Channel, ReadOnlyChannel};
use crate::event::{Event, EventFuture};
use crate::sys;

pub use data::*;
pub use map::*;

// some things i don't like and want to work on
//
// - represent states of a stream in types
//   instead of no-opping
//
// - check that envoy_status_t is good
//   and error if it's not
//
// - extend the lifetime of data passed across program boundaries
//   a la string views

/// Entrypoint to envoy-mobile. Used to construct an [Engine].
pub struct EngineBuilder {
    logger: Logger,
    event_track: Option<EventTrack>,
    connect_timeout_seconds: usize,
    dns_refresh_rate_seconds: usize,
    dns_fail_base_interval_seconds: usize,
    dns_fail_max_interval_seconds: usize,
    dns_query_timeout_seconds: usize,
    // TODO
    // - &dns_preresolve_hostnames []
    enable_interface_binding: bool,
    h2_connection_keepalive_idle_interval_seconds: usize,
    h2_connection_keepalive_timeout: usize,
    // TODO
    // - &metadata {}
    stats_domain: IpAddr,
    stats_flush_interval_seconds: usize,
    // TODO
    // - &stats_sinks []
    statsd_host: IpAddr,
    statsd_port: u16,
    stream_idle_timeout_seconds: usize,
    per_try_idle_timeout_seconds: usize,
    // TODO
    // - &virtual_clusters []
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self {
            logger: Logger::default(),
            event_track: None,
            connect_timeout_seconds: 30,
            dns_refresh_rate_seconds: 60,
            dns_fail_base_interval_seconds: 2,
            dns_fail_max_interval_seconds: 10,
            dns_query_timeout_seconds: 25,
            enable_interface_binding: false,
            h2_connection_keepalive_idle_interval_seconds: 100000,
            h2_connection_keepalive_timeout: 10,
            stats_domain: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            stats_flush_interval_seconds: 60,
            statsd_host: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            statsd_port: 8125,
            stream_idle_timeout_seconds: 15,
            per_try_idle_timeout_seconds: 15,
        }
    }
}

impl EngineBuilder {
    pub fn with_log(mut self, log: LoggerLog) -> Self {
        self.logger.log = Some(log);
        self
    }

    pub fn with_track(mut self, track: EventTrack) -> Self {
        self.event_track = Some(track);
        self
    }

    pub fn with_connect_timeout_seconds(mut self, connect_timeout_seconds: usize) -> Self {
        self.connect_timeout_seconds = connect_timeout_seconds;
        self
    }

    pub fn with_dns_refresh_rate_seconds(mut self, dns_refresh_rate_seconds: usize) -> Self {
        self.dns_refresh_rate_seconds = dns_refresh_rate_seconds;
        self
    }

    pub fn with_dns_fail_interval(
        mut self,
        dns_fail_base_interval_seconds: usize,
        dns_fail_max_interval_seconds: usize,
    ) -> Self {
        self.dns_fail_base_interval_seconds = dns_fail_base_interval_seconds;
        self.dns_fail_max_interval_seconds = dns_fail_max_interval_seconds;
        self
    }

    pub fn with_dns_query_timeout_seconds(mut self, dns_query_timeout_seconds: usize) -> Self {
        self.dns_query_timeout_seconds = dns_query_timeout_seconds;
        self
    }

    pub fn with_enable_interface_binding(mut self, enable_interface_binding: bool) -> Self {
        self.enable_interface_binding = enable_interface_binding;
        self
    }

    pub fn with_h2_connection_keepalive_idle_interval_seconds(
        mut self,
        h2_connection_keepalive_idle_interval_seconds: usize,
    ) -> Self {
        self.h2_connection_keepalive_idle_interval_seconds =
            h2_connection_keepalive_idle_interval_seconds;
        self
    }

    pub fn with_h2_connection_keepalive_timeout(
        mut self,
        h2_connection_keepalive_timeout: usize,
    ) -> Self {
        self.h2_connection_keepalive_timeout = h2_connection_keepalive_timeout;
        self
    }

    pub fn with_stats_domain(mut self, stats_domain: IpAddr) -> Self {
        self.stats_domain = stats_domain;
        self
    }

    pub fn with_stats_flush_interval_seconds(
        mut self,
        stats_flush_interval_seconds: usize,
    ) -> Self {
        self.stats_flush_interval_seconds = stats_flush_interval_seconds;
        self
    }

    pub fn with_statsd_host(mut self, statsd_host: IpAddr) -> Self {
        self.statsd_host = statsd_host;
        self
    }

    pub fn with_statsd_port(mut self, statsd_port: u16) -> Self {
        self.statsd_port = statsd_port;
        self
    }

    pub fn with_stream_idle_timeout_seconds(mut self, stream_idle_timeout_seconds: usize) -> Self {
        self.stream_idle_timeout_seconds = stream_idle_timeout_seconds;
        self
    }

    pub fn with_per_try_idle_timeout_seconds(
        mut self,
        per_try_idle_timeout_seconds: usize,
    ) -> Self {
        self.per_try_idle_timeout_seconds = per_try_idle_timeout_seconds;
        self
    }

    pub fn build(self, log_level: LogLevel) -> EventFuture<Arc<Engine>> {
        let context = EngineContext::default();
        let (envoy_callbacks, envoy_event_tracker) =
            EngineCallbacks::new(context.clone()).into_envoy_engine_callbacks();
        let envoy_logger = self.logger.into_envoy_logger();
        let handle;
        unsafe {
            handle = sys::init_engine(envoy_callbacks, envoy_logger, envoy_event_tracker);
        }

        let config_header = format!(
            r"!ignore default_defs:
- &connect_timeout {connect_timeout}s
- &dns_refresh_rate {dns_refresh_rate}s
- &dns_fail_base_interval {dns_fail_base_interval}s
- &dns_fail_max_interval {dns_fail_max_interval}s
- &dns_query_timeout {dns_query_timeout}s
- &enable_interface_binding {enable_interface_binding}
- &h2_connection_keepalive_idle_interval {h2_connection_keepalive_idle_interval}s
- &h2_connection_keepalive_timeout {h2_connection_keepalive_timeout}s
- &stats_domain {stats_domain}
- &stats_flush_interval {stats_flush_interval}s
- &statsd_host {statsd_host}
- &statsd_port {statsd_port}
- &stream_idle_timeout {stream_idle_timeout}s
- &per_try_idle_timeout {per_try_idle_timeout}s",
            connect_timeout = self.connect_timeout_seconds,
            dns_refresh_rate = self.dns_refresh_rate_seconds,
            dns_fail_base_interval = self.dns_fail_base_interval_seconds,
            dns_fail_max_interval = self.dns_fail_max_interval_seconds,
            dns_query_timeout = self.dns_query_timeout_seconds,
            enable_interface_binding = self.enable_interface_binding,
            h2_connection_keepalive_idle_interval =
                self.h2_connection_keepalive_idle_interval_seconds,
            h2_connection_keepalive_timeout = self.h2_connection_keepalive_timeout,
            stats_domain = self.stats_domain,
            stats_flush_interval = self.stats_flush_interval_seconds,
            statsd_host = self.statsd_host,
            statsd_port = self.statsd_port,
            stream_idle_timeout = self.stream_idle_timeout_seconds,
            per_try_idle_timeout = self.per_try_idle_timeout_seconds,
        );

        let config_body;
        unsafe {
            config_body = ffi::CStr::from_ptr(sys::config_template);
        }
        let config_body = config_body.to_str().unwrap();

        let config = format!("{}\n{}", config_header, config_body);
        let config_cstr = ffi::CString::new(config).unwrap();
        let log_level = ffi::CString::new(log_level.into_envoy_log_level()).unwrap();
        unsafe {
            sys::run_engine(handle, config_cstr.as_ptr(), log_level.as_ptr());
        }

        let engine_running = context.engine_running.clone();
        EventFuture::new(Arc::new(Engine { handle, context }), engine_running)
    }
}

/// Handle to interact with envoy-mobile. Only one may exist per-process.
pub struct Engine {
    handle: isize,
    context: EngineContext,
}

impl Engine {
    /// Create a new [Stream]. If `explicit_flow_control` is set to true,
    /// the consumer must manually control reads and writes
    /// by using [Stream::read_data] and [Stream::send_window_available].
    pub fn new_stream(self: &Arc<Self>, explicit_flow_control: bool) -> Stream {
        let handle;
        unsafe {
            handle = sys::init_stream(self.handle);
        }

        let stream_context = Arc::<StreamContext>::default();
        let stream_callbacks = StreamCallbacks::new(stream_context.clone());
        unsafe {
            sys::start_stream(
                handle,
                stream_callbacks.into_envoy_http_callbacks(),
                explicit_flow_control,
            );
        }

        Stream::new(self.clone(), handle, stream_context)
    }

    /// Increments a counter to be emitted to the local statsd instance
    /// whose address and port are defined by [EngineBuilder::with_statsd_host]
    /// and [EngineBuilder::with_statsd_port].
    pub fn record_counter_inc<S: AsRef<str>>(
        self: &Arc<Self>,
        elements: S,
        tags: StatsTags,
        count: usize,
    ) {
        let bytes = Vec::from_iter(elements.as_ref().bytes());
        let elements_cstr = ffi::CStr::from_bytes_with_nul(&bytes).unwrap();

        let envoy_stats_tags = tags.into_envoy_map();
        unsafe {
            sys::record_counter_inc(
                self.handle,
                elements_cstr.as_ptr(),
                envoy_stats_tags,
                count.try_into().unwrap(),
            );
        }
    }

    /// Set a gauge to be emitted to the local statsd instance
    /// whose address and port are defined by [EngineBuilder::with_statsd_host]
    /// and [EngineBuilder::with_statsd_port].
    pub fn record_gauge_set<S: AsRef<str>>(
        self: &Arc<Self>,
        elements: S,
        tags: StatsTags,
        value: usize,
    ) {
        let bytes = Vec::from_iter(elements.as_ref().bytes());
        let elements_cstr = ffi::CStr::from_bytes_with_nul(&bytes).unwrap();

        let envoy_stats_tags = tags.into_envoy_map();
        unsafe {
            sys::record_gauge_set(
                self.handle,
                elements_cstr.as_ptr(),
                envoy_stats_tags,
                value.try_into().unwrap(),
            );
        }
    }

    /// Adds to a gauge to be emitted to the local statsd instance
    /// whose address and port are defined by [EngineBuilder::with_statsd_host]
    /// and [EngineBuilder::with_statsd_port].
    pub fn record_gauge_add<S: AsRef<str>>(
        self: &Arc<Self>,
        elements: S,
        tags: StatsTags,
        amount: usize,
    ) {
        let bytes = Vec::from_iter(elements.as_ref().bytes());
        let elements_cstr = ffi::CStr::from_bytes_with_nul(&bytes).unwrap();

        let envoy_stats_tags = tags.into_envoy_map();
        unsafe {
            sys::record_gauge_add(
                self.handle,
                elements_cstr.as_ptr(),
                envoy_stats_tags,
                amount.try_into().unwrap(),
            );
        }
    }

    /// Subtracts from a gauge to be emitted to the local statsd instance
    /// whose address and port are defined by [EngineBuilder::with_statsd_host]
    /// and [EngineBuilder::with_statsd_port].
    pub fn record_gauge_sub<S: AsRef<str>>(
        self: &Arc<Self>,
        elements: S,
        tags: StatsTags,
        amount: usize,
    ) {
        let bytes = Vec::from_iter(elements.as_ref().bytes());
        let elements_cstr = ffi::CStr::from_bytes_with_nul(&bytes).unwrap();

        let envoy_stats_tags = tags.into_envoy_map();
        unsafe {
            sys::record_gauge_sub(
                self.handle,
                elements_cstr.as_ptr(),
                envoy_stats_tags,
                amount.try_into().unwrap(),
            );
        }
    }

    /// Records a value belonging to a histogram which will be emitted to the local statsd instance
    /// whose address and port are defined by [EngineBuilder::with_statsd_host]
    /// and [EngineBuilder::with_statsd_port].
    pub fn record_histogram_value<S: AsRef<str>>(
        self: &Arc<Self>,
        elements: S,
        tags: StatsTags,
        amount: usize,
        unit_measure: HistogramStatUnit,
    ) {
        let bytes = Vec::from_iter(elements.as_ref().bytes());
        let elements_cstr = ffi::CStr::from_bytes_with_nul(&bytes).unwrap();

        let envoy_stats_tags = tags.into_envoy_map();
        let envoy_histogram_stat_unit = unit_measure.into_envoy_histogram_stat_unit();
        unsafe {
            sys::record_histogram_value(
                self.handle,
                elements_cstr.as_ptr(),
                envoy_stats_tags,
                amount.try_into().unwrap(),
                envoy_histogram_stat_unit,
            );
        }
    }

    /// Flushes all buffered stats to the statsd instance
    /// whose address and port are defined by [EngineBuilder::with_statsd_host]
    /// and [EngineBuilder::with_statsd_port].
    pub fn flush_stats(self: &Arc<Self>) {
        unsafe {
            sys::flush_stats(self.handle);
        }
    }

    /// Dumps the current stats out to a series of bytes.
    pub fn dump_stats(self: &Arc<Self>) -> Data {
        let envoy_data;
        unsafe {
            envoy_data = sys::envoy_nodata;
            sys::dump_stats(
                self.handle,
                &envoy_data as *const sys::envoy_data as *mut sys::envoy_data,
            );
            Data::from_envoy_data(envoy_data)
        }
    }

    /// First drains connections from the Engine and then terminates it.
    /// Consumes a reference to the Engine, so the primary place of use can no longer use it.
    /// This must be called only once per-Engine.
    pub fn terminate(self: Arc<Self>) -> EventFuture<()> {
        unsafe {
            sys::drain_connections(self.handle);
            sys::terminate_engine(self.handle);
        }
        EventFuture::new((), self.context.engine_terminated.clone())
    }
}

/// Handle to an individual request/response.
/// Used to send data and receive data across the wire.
pub struct Stream {
    _engine: Arc<Engine>,
    handle: isize,
    context: Arc<StreamContext>,
}

impl Stream {
    fn new(engine: Arc<Engine>, handle: isize, context: Arc<StreamContext>) -> Self {
        Self {
            _engine: engine,
            handle,
            context,
        }
    }

    pub fn send_headers<T: Into<Headers>>(&mut self, headers: T, end_stream: bool) {
        let headers = headers.into();
        let envoy_headers = headers.into_envoy_map();
        unsafe {
            sys::send_headers(self.handle, envoy_headers, end_stream);
        }
    }

    pub fn send_data<T: Into<Data>>(&mut self, data: T, end_stream: bool) {
        let envoy_data = data.into().into_envoy_data();
        unsafe {
            sys::send_data(self.handle, envoy_data, end_stream);
        }
    }

    pub fn send_metadata<T: Into<Headers>>(&mut self, metadata: T) {
        let envoy_metadata = metadata.into().into_envoy_map();
        unsafe {
            sys::send_metadata(self.handle, envoy_metadata);
        }
    }

    pub fn send_trailers<T: Into<Headers>>(&mut self, trailers: T) {
        let envoy_trailers = trailers.into().into_envoy_map();
        unsafe {
            sys::send_trailers(self.handle, envoy_trailers);
        }
    }

    pub fn read_data(&mut self, bytes_to_read: usize) {
        // TODO: make this error if we're not
        // in explicit flow control mode?
        // or BETTER YET
        // make this only available on a specific form of Stream
        // that has explicit flow control mode enabled
        unsafe {
            sys::read_data(self.handle, bytes_to_read.try_into().unwrap());
        }
    }

    pub fn reset_stream(self) {
        unsafe {
            sys::reset_stream(self.handle);
        }
    }

    pub fn headers(&'_ self) -> ReadOnlyChannel<'_, Headers> {
        ReadOnlyChannel::new(&self.context.headers)
    }

    pub fn data(&'_ self) -> ReadOnlyChannel<'_, Data> {
        ReadOnlyChannel::new(&self.context.data)
    }

    pub fn metadata(&'_ self) -> ReadOnlyChannel<'_, Headers> {
        ReadOnlyChannel::new(&self.context.metadata)
    }

    pub fn trailers(&'_ self) -> ReadOnlyChannel<'_, Headers> {
        ReadOnlyChannel::new(&self.context.trailers)
    }

    pub fn send_window_available(&'_ self) -> ReadOnlyChannel<'_, ()> {
        ReadOnlyChannel::new(&self.context.send_window_available)
    }

    pub fn completion(&'_ self) -> ReadOnlyChannel<'_, Completion> {
        ReadOnlyChannel::new(&self.context.completion)
    }
}

#[derive(Clone)]
struct EngineContext {
    engine_running: Arc<Event>,
    engine_terminated: Arc<Event>,
}

impl Default for EngineContext {
    fn default() -> Self {
        Self {
            engine_running: Arc::default(),
            engine_terminated: Arc::default(),
        }
    }
}

type OnEngineRunning = fn(&EngineContext);
type OnExit = fn(&EngineContext);
type EventTrack = fn(Map);

struct EngineCallbacks {
    on_engine_running: OnEngineRunning,
    on_exit: OnExit,
    event_track: EventTrack,
    context: EngineContext,
}

impl EngineCallbacks {
    fn new(context: EngineContext) -> Self {
        Self {
            on_engine_running: |ctx| ctx.engine_running.set(),
            on_exit: |ctx| ctx.engine_terminated.set(),
            event_track: |_| {},
            context,
        }
    }

    fn into_envoy_engine_callbacks(
        self,
    ) -> (sys::envoy_engine_callbacks, sys::envoy_event_tracker) {
        let engine_callbacks = Box::into_raw(Box::new(self));

        let envoy_engine_callbacks = sys::envoy_engine_callbacks {
            on_engine_running: Some(EngineCallbacks::c_on_engine_running),
            on_exit: Some(EngineCallbacks::c_on_exit),
            context: engine_callbacks as *mut ffi::c_void,
        };

        let envoy_event_tracker = sys::envoy_event_tracker {
            track: Some(EngineCallbacks::c_track),
            context: engine_callbacks as *mut ffi::c_void,
        };

        (envoy_engine_callbacks, envoy_event_tracker)
    }

    unsafe extern "C" fn c_on_engine_running(context: *mut ffi::c_void) {
        let engine_callbacks = context as *mut EngineCallbacks;
        ((*engine_callbacks).on_engine_running)(&(*engine_callbacks).context);
    }

    unsafe extern "C" fn c_on_exit(context: *mut ffi::c_void) {
        let engine_callbacks = Box::from_raw(context as *mut EngineCallbacks);
        ((*engine_callbacks).on_exit)(&engine_callbacks.context);
    }

    unsafe extern "C" fn c_track(envoy_map: sys::envoy_map, context: *const ffi::c_void) {
        let engine_callbacks = context as *const EngineCallbacks;
        ((*engine_callbacks).event_track)(Map::from_envoy_map(envoy_map));
    }
}

/// Provided at the end of a [Stream] via [Stream::completion]
/// to indicate the final state of the request/response.
#[derive(Debug, PartialEq)]
pub enum Completion {
    /// The stream completed by being cancelled,
    /// either by [Stream::reset_stream] or the upstream.
    Cancel,

    /// The stream completed successfully.
    Complete,

    /// The stream completed in error.
    Error(Error),
}

struct StreamContext {
    headers: Channel<Headers>,
    data: Channel<Data>,
    metadata: Channel<Headers>,
    trailers: Channel<Headers>,
    send_window_available: Channel<()>,
    completion: Channel<Completion>,
}

impl Default for StreamContext {
    fn default() -> Self {
        Self {
            headers: Channel::default(),
            data: Channel::default(),
            metadata: Channel::default(),
            trailers: Channel::default(),
            send_window_available: Channel::default(),
            completion: Channel::default(),
        }
    }
}

impl StreamContext {
    fn close_channels(&self) {
        self.headers.close();
        self.data.close();
        self.metadata.close();
        self.trailers.close();
        self.send_window_available.close();
        self.completion.close();
    }
}

type OnHeaders = fn(&StreamContext, Headers, bool);
type OnData = fn(&StreamContext, Data, bool);
type OnMetadata = fn(&StreamContext, Headers);
type OnTrailers = fn(&StreamContext, Headers);
type OnError = fn(&StreamContext, Error);
type OnComplete = fn(&StreamContext);
type OnCancel = fn(&StreamContext);
type OnSendWindowAvailable = fn(&StreamContext);

struct StreamCallbacks {
    on_headers: OnHeaders,
    on_data: OnData,
    on_metadata: OnMetadata,
    on_trailers: OnTrailers,
    on_error: OnError,
    on_complete: OnComplete,
    on_cancel: OnCancel,
    on_send_window_available: OnSendWindowAvailable,
    context: Arc<StreamContext>,
}

impl StreamCallbacks {
    fn new(context: Arc<StreamContext>) -> Self {
        Self {
            on_headers: |ctx, headers, _| {
                ctx.headers.put(headers);
            },
            on_data: |ctx, data, _| {
                ctx.headers.close();
                ctx.data.put(data);
            },
            on_metadata: |ctx, metadata| {
                ctx.metadata.put(metadata);
            },
            on_trailers: |ctx, trailers| {
                ctx.headers.close();
                ctx.data.close();
                ctx.trailers.put(trailers);
            },
            on_error: |ctx, error| {
                ctx.completion.put(Completion::Error(error));
                ctx.close_channels();
            },
            on_complete: |ctx| {
                ctx.completion.put(Completion::Complete);
                ctx.close_channels();
            },
            on_cancel: |ctx| {
                ctx.completion.put(Completion::Cancel);
                ctx.close_channels();
            },
            on_send_window_available: |ctx| {
                ctx.send_window_available.put(());
            },
            context,
        }
    }

    fn into_envoy_http_callbacks(self) -> sys::envoy_http_callbacks {
        let http_callbacks = Box::into_raw(Box::new(self));
        sys::envoy_http_callbacks {
            on_headers: Some(StreamCallbacks::c_on_headers),
            on_data: Some(StreamCallbacks::c_on_data),
            on_metadata: Some(StreamCallbacks::c_on_metadata),
            on_trailers: Some(StreamCallbacks::c_on_trailers),
            on_error: Some(StreamCallbacks::c_on_error),
            on_complete: Some(StreamCallbacks::c_on_complete),
            on_cancel: Some(StreamCallbacks::c_on_cancel),
            on_send_window_available: Some(StreamCallbacks::c_on_send_window_available),
            context: http_callbacks as *mut ffi::c_void,
        }
    }

    unsafe extern "C" fn c_on_headers(
        envoy_headers: sys::envoy_headers,
        end_stream: bool,
        _: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const StreamCallbacks;
        let headers = Headers::from_envoy_map(envoy_headers);
        ((*http_callbacks).on_headers)(&(*http_callbacks).context, headers, end_stream);
        context
    }

    unsafe extern "C" fn c_on_data(
        envoy_data: sys::envoy_data,
        end_stream: bool,
        _: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const StreamCallbacks;
        let data = Data::from_envoy_data(envoy_data);
        ((*http_callbacks).on_data)(&(*http_callbacks).context, data, end_stream);
        context
    }

    unsafe extern "C" fn c_on_metadata(
        envoy_metadata: sys::envoy_headers,
        _: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const StreamCallbacks;
        let metadata = Headers::from_envoy_map(envoy_metadata);
        ((*http_callbacks).on_metadata)(&(*http_callbacks).context, metadata);
        context
    }

    unsafe extern "C" fn c_on_trailers(
        envoy_trailers: sys::envoy_headers,
        _: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const StreamCallbacks;
        let trailers = Headers::from_envoy_map(envoy_trailers);
        ((*http_callbacks).on_trailers)(&(*http_callbacks).context, trailers);
        context
    }

    unsafe extern "C" fn c_on_error(
        envoy_error: sys::envoy_error,
        _: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const StreamCallbacks;
        let error = Error::from_envoy_error(envoy_error);
        ((*http_callbacks).on_error)(&(*http_callbacks).context, error);
        context
    }

    unsafe extern "C" fn c_on_complete(
        _: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = Box::from_raw(context as *mut StreamCallbacks);
        ((*http_callbacks).on_complete)(&(*http_callbacks).context);
        context
    }

    unsafe extern "C" fn c_on_cancel(
        _: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = Box::from_raw(context as *mut StreamCallbacks);
        ((*http_callbacks).on_cancel)(&(*http_callbacks).context);
        context
    }

    unsafe extern "C" fn c_on_send_window_available(
        _: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = Box::from_raw(context as *mut StreamCallbacks);
        ((*http_callbacks).on_send_window_available)(&(*http_callbacks).context);
        context
    }
}

/// The level at which the underlying envoy-mobile Engine instance logs
/// and, therefore, which logs are provided to the logger specified in
/// [EngineBuilder::with_log].
#[derive(Clone, Copy)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Critical,
    Off,
}

impl LogLevel {
    fn into_envoy_log_level(self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
            LogLevel::Critical => "critical",
            LogLevel::Off => "off",
        }
    }
}

enum Status {
    Success,
    Failure,
}

impl Status {
    fn from_envoy_status(envoy_status: sys::envoy_status_t) -> Self {
        match envoy_status {
            0 => Status::Success,
            _ => Status::Failure,
        }
    }
}

/// Used to indicate what kind of stat is being provided
/// to [Engine::record_histogram_value].
#[derive(Clone, Copy)]
pub enum HistogramStatUnit {
    Unspecified,
    Bytes,
    Microseconds,
    Milliseconds,
}

impl HistogramStatUnit {
    fn into_envoy_histogram_stat_unit(self) -> sys::envoy_histogram_stat_unit_t {
        match self {
            HistogramStatUnit::Unspecified => 0,
            HistogramStatUnit::Bytes => 1,
            HistogramStatUnit::Microseconds => 2,
            HistogramStatUnit::Milliseconds => 3,
        }
    }
}

/// An enumeration of the kinds of errors that envoy-mobile can report.
#[derive(Copy, Debug, Clone, PartialEq)]
pub enum ErrorCode {
    UndefinedError,
    StreamReset,
    ConnectionFailure,
    BufferLimitExceeded,
    RequestTimeout,
}

impl From<ErrorCode> for &'static str {
    fn from(error_code: ErrorCode) -> &'static str {
        match error_code {
            ErrorCode::UndefinedError => "undefined_error",
            ErrorCode::StreamReset => "stream_reset",
            ErrorCode::ConnectionFailure => "connection_failure",
            ErrorCode::BufferLimitExceeded => "buffer_limit_exceeded",
            ErrorCode::RequestTimeout => "request_timeout",
        }
    }
}

impl ErrorCode {
    fn from_envoy_error_code(envoy_error_code: sys::envoy_error_code_t) -> Self {
        match envoy_error_code {
            0 => ErrorCode::UndefinedError,
            1 => ErrorCode::StreamReset,
            2 => ErrorCode::ConnectionFailure,
            3 => ErrorCode::BufferLimitExceeded,
            4 => ErrorCode::RequestTimeout,
            _ => panic!(),
        }
    }
}

/// Various kinds of network, e.g. WLAN vs. WWAN.
pub enum Network {
    /// The default. Used where network characteristics are unknown.
    Generic,
    /// WiFi and other LANs.
    Wlan,
    /// Mobile phone networks.
    Wwan,
}

impl Network {
    fn into_envoy_network(self) -> sys::envoy_network_t {
        match self {
            Network::Generic => 0,
            Network::Wlan => 1,
            Network::Wwan => 2,
        }
    }
}

/// The underlying error type returned by envoy-mobile when a request fails.
/// Provided in [Completion::Error] upon stream completion.
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub error_code: ErrorCode,
    pub message: String,
    pub attempt_count: i32,
}

impl Error {
    unsafe fn from_envoy_error(envoy_error: sys::envoy_error) -> Self {
        let error = Self {
            error_code: ErrorCode::from_envoy_error_code(envoy_error.error_code),
            message: Data::from_envoy_data_no_release(&envoy_error.message)
                .try_into()
                .expect("envoy_mobile must return a string for envoy_error.message"),
            attempt_count: envoy_error.attempt_count,
        };
        sys::release_envoy_error(envoy_error);
        error
    }
}

// TODO: provide a context to this thing
type LoggerLog = fn(Data);

struct Logger {
    log: Option<LoggerLog>,
}

impl Default for Logger {
    fn default() -> Self {
        Self { log: None }
    }
}

impl Logger {
    fn into_envoy_logger(self) -> sys::envoy_logger {
        let logger = Box::into_raw(Box::new(self));
        sys::envoy_logger {
            log: Some(Logger::c_log),
            release: Some(Logger::c_release),
            context: logger as *mut ffi::c_void,
        }
    }

    unsafe extern "C" fn c_log(envoy_data: sys::envoy_data, context: *const ffi::c_void) {
        let logger = context as *const Logger;
        let data = Data::from_envoy_data(envoy_data);
        if let Some(log) = &(*logger).log {
            log(data);
        }
    }

    unsafe extern "C" fn c_release(context: *const ffi::c_void) {
        let _ = Box::from_raw(context as *mut Logger);
    }
}

/// Update the network interface to the preferred network for opening new streams.
pub fn set_preferred_network(network: Network) {
    unsafe {
        sys::set_preferred_network(network.into_envoy_network());
    }
}

/// [HTTP request method](https://developer.mozilla.org/en-US/docs/Web/HTTP/Methods).
#[derive(Clone, Copy)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
    Patch,
}

impl TryFrom<&str> for Method {
    type Error = &'static str;

    fn try_from(string: &str) -> Result<Self, <Self as TryFrom<&str>>::Error> {
        let string = string.to_lowercase();
        Ok(match string.as_str() {
            "get" => Method::Get,
            "head" => Method::Head,
            "post" => Method::Post,
            "put" => Method::Put,
            "delete" => Method::Delete,
            "connect" => Method::Connect,
            "options" => Method::Options,
            "trace" => Method::Trace,
            "patch" => Method::Patch,
            _ => return Err("invalid method string"),
        })
    }
}

impl Method {
    pub fn into_envoy_method(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Head => "HEAD",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Connect => "CONNECT",
            Method::Options => "OPTIONS",
            Method::Trace => "TRACE",
            Method::Patch => "PATCH",
        }
    }
}

/// Request scheme, only HTTP and HTTPS are supported.
pub enum Scheme {
    Http,
    Https,
}

impl TryFrom<&str> for Scheme {
    type Error = &'static str;

    fn try_from(string: &str) -> Result<Self, Self::Error> {
        let string = string.to_lowercase();
        Ok(match string.as_ref() {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            _ => return Err("invalid scheme string"),
        })
    }
}

impl Scheme {
    pub fn into_envoy_scheme(self) -> &'static str {
        match self {
            Scheme::Http => "http",
            Scheme::Https => "https",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tokio::test;

    use super::*;

    #[test]
    async fn engine_lifecycle() {
        let engine = EngineBuilder::default()
            .with_log(|data| {
                print!("{}", String::try_from(data).unwrap());
            })
            .build(LogLevel::Debug)
            .await;
        engine.terminate().await;
    }

    async fn make_request(mut stream: Stream) {
        stream.send_headers(
            Headers::new_request_headers(
                Method::Get,
                Scheme::Http,
                "localhost:8080",
                "/",
            ),
            true,
        );

        while let Some(headers) = stream.headers().poll().await {
            let headers = HashMap::<String, String>::try_from(headers).unwrap();
            if let Some(status) = headers.get(":status") {
                assert_eq!(status, "200");
            }
            for (key, value) in headers.into_iter() {
                println!("{}: {}", key, value);
            }
        }

        while let Some(data) = stream.data().poll().await {
            let data_str: &str = (&data).try_into().unwrap();
            println!("{}", data_str);
        }

        while let Some(_) = stream.trailers().poll().await {}
        while let Some(_) = stream.metadata().poll().await {}

        let completion = stream.completion().poll().await;
        assert_eq!(completion, Some(Completion::Complete));
    }

    #[test]
    async fn stream_lifecycle() {
        let engine = EngineBuilder::default()
            .with_log(|data| {
                print!("{}", String::try_from(data).unwrap());
            })
            .build(LogLevel::Error)
            .await;

        make_request(engine.new_stream(false)).await;

        engine.terminate().await;
    }

    #[test]
    async fn loop_trigger_memory_leak() {
        let engine = EngineBuilder::default()
            .with_log(|data| {
                print!("{}", String::try_from(data).unwrap());
            })
            .build(LogLevel::Error)
            .await;

        let mut requests = Vec::with_capacity(100);
        for _ in 0..100 {
            let stream = engine.new_stream(false);
            requests.push(tokio::spawn(make_request(stream)));
        }
        for request in requests.into_iter() {
            request.await.unwrap();
        }

        engine.terminate().await;
    }
}
