use std::alloc;
use std::collections::{BTreeMap, HashMap};
use std::ffi;
use std::net::{IpAddr, Ipv4Addr};
use std::ptr;
use std::str::Utf8Error;
use std::string::FromUtf8Error;
use std::sync::Arc;

#[cfg(python)]
use pyo3::prelude::*;

use crate::channel::{Channel, ReadOnlyChannel};
use crate::event::{Event, EventFuture};
use crate::sys;

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

#[derive(Clone)]
pub struct EngineContext {
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

pub type OnEngineRunning = fn(&EngineContext);
pub type OnExit = fn(&EngineContext);
pub type EventTrack = fn(Map);

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

pub struct Engine {
    handle: isize,
    context: EngineContext,
}

impl Engine {
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

    pub fn record_counter_inc<S: AsRef<str>>(self: &Arc<Self>, elements: S, tags: StatsTags, count: usize) {
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

    pub fn record_gauge_set<S: AsRef<str>>(self: &Arc<Self>, elements: S, tags: StatsTags, value: usize) {
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

    pub fn record_gauge_add<S: AsRef<str>>(self: &Arc<Self>, elements: S, tags: StatsTags, amount: usize) {
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

    pub fn record_gauge_sub<S: AsRef<str>>(self: &Arc<Self>, elements: S, tags: StatsTags, amount: usize) {
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

    pub fn flush_stats(self: &Arc<Self>) {
        unsafe {
            sys::flush_stats(self.handle);
        }
    }

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

    pub fn terminate(self: Arc<Self>) -> EventFuture<()> {
        unsafe {
            sys::drain_connections(self.handle);
            sys::terminate_engine(self.handle);
        }
        EventFuture::new((), self.context.engine_terminated.clone())
    }
}

#[derive(Debug, PartialEq)]
pub enum Completion {
    Cancel,
    Complete,
    Error(Error),
}

pub struct StreamContext {
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

pub type OnHeaders = fn(&StreamContext, Headers, bool, StreamIntel);
pub type OnData = fn(&StreamContext, Data, bool, StreamIntel);
pub type OnMetadata = fn(&StreamContext, Headers, StreamIntel);
pub type OnTrailers = fn(&StreamContext, Headers, StreamIntel);
pub type OnError = fn(&StreamContext, Error, StreamIntel);
pub type OnComplete = fn(&StreamContext, StreamIntel);
pub type OnCancel = fn(&StreamContext, StreamIntel);
pub type OnSendWindowAvailable = fn(&StreamContext, StreamIntel);

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
            on_headers: |ctx, headers, _, _| {
                ctx.headers.put(headers);
            },
            on_data: |ctx, data, _, _| {
                ctx.headers.close();
                ctx.data.put(data);
            },
            on_metadata: |ctx, metadata, _| {
                ctx.metadata.put(metadata);
            },
            on_trailers: |ctx, trailers, _| {
                ctx.headers.close();
                ctx.data.close();
                ctx.trailers.put(trailers);
            },
            on_error: |ctx, error, _| {
                ctx.completion.put(Completion::Error(error));
                ctx.close_channels();
            },
            on_complete: |ctx, _| {
                ctx.completion.put(Completion::Complete);
                ctx.close_channels();
            },
            on_cancel: |ctx, _| {
                ctx.completion.put(Completion::Cancel);
                ctx.close_channels();
            },
            on_send_window_available: |ctx, _| {
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
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const StreamCallbacks;
        let headers = Headers::from_envoy_map(envoy_headers);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        ((*http_callbacks).on_headers)(
            &(*http_callbacks).context,
            headers,
            end_stream,
            stream_intel,
        );
        context
    }

    unsafe extern "C" fn c_on_data(
        envoy_data: sys::envoy_data,
        end_stream: bool,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const StreamCallbacks;
        let data = Data::from_envoy_data(envoy_data);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        ((*http_callbacks).on_data)(&(*http_callbacks).context, data, end_stream, stream_intel);
        context
    }

    unsafe extern "C" fn c_on_metadata(
        envoy_metadata: sys::envoy_headers,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const StreamCallbacks;
        let metadata = Headers::from_envoy_map(envoy_metadata);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        ((*http_callbacks).on_metadata)(&(*http_callbacks).context, metadata, stream_intel);
        context
    }

    unsafe extern "C" fn c_on_trailers(
        envoy_trailers: sys::envoy_headers,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const StreamCallbacks;
        let trailers = Headers::from_envoy_map(envoy_trailers);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        ((*http_callbacks).on_trailers)(&(*http_callbacks).context, trailers, stream_intel);
        context
    }

    unsafe extern "C" fn c_on_error(
        envoy_error: sys::envoy_error,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const StreamCallbacks;
        let error = Error::from_envoy_error(envoy_error);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        ((*http_callbacks).on_error)(&(*http_callbacks).context, error, stream_intel);
        context
    }

    unsafe extern "C" fn c_on_complete(
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = Box::from_raw(context as *mut StreamCallbacks);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        ((*http_callbacks).on_complete)(&(*http_callbacks).context, stream_intel);
        context
    }

    unsafe extern "C" fn c_on_cancel(
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = Box::from_raw(context as *mut StreamCallbacks);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        ((*http_callbacks).on_cancel)(&(*http_callbacks).context, stream_intel);
        context
    }

    unsafe extern "C" fn c_on_send_window_available(
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = Box::from_raw(context as *mut StreamCallbacks);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        ((*http_callbacks).on_send_window_available)(&(*http_callbacks).context, stream_intel);
        context
    }
}

pub struct Stream {
    engine: Arc<Engine>,
    handle: isize,
    context: Arc<StreamContext>,
}

impl Stream {
    fn new(engine: Arc<Engine>, handle: isize, context: Arc<StreamContext>) -> Self {
        Self {
	    engine,
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

#[cfg(python)]
#[pyclass]
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

#[cfg(not(python))]
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

#[cfg(python)]
#[pyclass]
#[derive(Clone, Copy)]
pub enum HistogramStatUnit {
    Unspecified,
    Bytes,
    Microseconds,
    Milliseconds,
}

#[cfg(not(python))]
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

#[cfg(python)]
#[pyclass]
#[derive(Copy, Debug, Clone, PartialEq)]
pub enum ErrorCode {
    UndefinedError,
    StreamReset,
    ConnectionFailure,
    BufferLimitExceeded,
    RequestTimeout,
}

#[cfg(not(python))]
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

pub enum Network {
    Generic,
    Wlan,
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

#[derive(Eq, Clone, Debug, PartialEq)]
pub struct Data(Vec<u8>);

impl From<&str> for Data {
    fn from(string: &str) -> Self {
        Data::from(string.to_owned())
    }
}

impl From<String> for Data {
    fn from(string: String) -> Self {
        Self(Vec::from_iter(string.into_bytes()))
    }
}

impl From<Vec<u8>> for Data {
    fn from(data: Vec<u8>) -> Self {
        Data(data)
    }
}

impl<'a> TryFrom<&'a Data> for &'a str {
    type Error = Utf8Error;

    fn try_from(data: &'a Data) -> Result<&'a str, Self::Error> {
        std::str::from_utf8(&data.0)
    }
}

impl TryFrom<Data> for String {
    type Error = FromUtf8Error;

    fn try_from(data: Data) -> Result<String, Self::Error> {
        String::from_utf8(data.into())
    }
}

impl From<Data> for Vec<u8> {
    fn from(data: Data) -> Vec<u8> {
        data.0
    }
}

impl Data {
    unsafe fn from_envoy_data_no_release(envoy_data: &sys::envoy_data) -> Self {
	let length = envoy_data.length.try_into().unwrap();
	let mut bytes = vec![0; length];
	ptr::copy(envoy_data.bytes, bytes.as_mut_ptr(), length);
	Self(bytes)
    }

    unsafe fn from_envoy_data(envoy_data: sys::envoy_data) -> Self {
        let data = Self::from_envoy_data_no_release(&envoy_data);
        sys::release_envoy_data(envoy_data);
        data
    }

    fn into_envoy_data(self) -> sys::envoy_data {
        let vec = Box::new(self.0);
        let data_ptr = vec.as_ptr();
        let length = vec.len();

        sys::envoy_data {
            length: length.try_into().unwrap(),
            bytes: data_ptr,
            release: Some(Data::release_data),
            context: Box::into_raw(vec) as *mut ffi::c_void,
        }
    }

    unsafe extern "C" fn release_data(context: *mut ffi::c_void) {
        let _ = Box::from_raw(context as *mut Vec<u8>);
    }
}

#[derive(Debug)]
pub struct Map(Vec<(Data, Data)>);

impl<S: AsRef<str>> From<Vec<(S, S)>> for Map
where
    Data: From<S>,
{
    fn from(pairs: Vec<(S, S)>) -> Self {
        let mut entries = Vec::with_capacity(pairs.len());
        for (key, value) in pairs.into_iter() {
            entries.push((Data::from(key), Data::from(value)));
        }
        Map(entries)
    }
}

impl From<HashMap<String, String>> for Map {
    fn from(hash_map: HashMap<String, String>) -> Self {
        let mut pairs = Vec::with_capacity(hash_map.len());
        for (key, value) in hash_map.into_iter() {
            pairs.push((Data::from(key), Data::from(value)));
        }
        Map(pairs)
    }
}

impl TryFrom<Map> for Vec<(String, String)> {
    type Error = FromUtf8Error;

    fn try_from(map: Map) -> Result<Vec<(String, String)>, Self::Error> {
        let mut pairs = Vec::with_capacity(map.0.len());
        for (key, value) in map.0.into_iter() {
            pairs.push((key.try_into()?, value.try_into()?));
        }
        Ok(pairs)
    }
}

impl TryFrom<Map> for HashMap<String, String> {
    type Error = FromUtf8Error;

    fn try_from(map: Map) -> Result<HashMap<String, String>, Self::Error> {
        let pairs = Vec::<(String, String)>::try_from(map)?;
        let mut hash_map = HashMap::new();
        for (key, value) in pairs.into_iter() {
            hash_map.insert(key, value);
        }
        Ok(hash_map)
    }
}

impl TryFrom<Map> for BTreeMap<String, String> {
    type Error = FromUtf8Error;

    fn try_from(map: Map) -> Result<BTreeMap<String, String>, Self::Error> {
        let pairs = Vec::<(String, String)>::try_from(map)?;
        let mut btree_map = BTreeMap::new();
        for (key, value) in pairs.into_iter() {
            btree_map.insert(key, value);
        }
        Ok(btree_map)
    }
}

impl Map {
    pub fn new_request_headers(
        method: Method,
        scheme: Scheme,
        authority: impl AsRef<str>,
        path: impl AsRef<str>,
    ) -> Self {
        Map::from(vec![
            (":method", method.into_envoy_method()),
            (":scheme", scheme.into_envoy_scheme()),
            (":authority", authority.as_ref()),
            (":path", path.as_ref()),
        ])
    }

    unsafe fn from_envoy_map(envoy_map: sys::envoy_map) -> Self {
        let length = envoy_map.length.try_into().unwrap();
        let mut entries = Vec::with_capacity(length);
        for i in 0..length {
            let entry = &*envoy_map.entries.add(i);
            entries.push((
                Data::from_envoy_data_no_release(&entry.key),
                Data::from_envoy_data_no_release(&entry.value),
            ));
        }
        sys::release_envoy_map(envoy_map);
        Self(entries)
    }

    fn into_envoy_map(self) -> sys::envoy_map {
        let length = self.0.len();
        let layout = alloc::Layout::array::<sys::envoy_map_entry>(length)
            .expect("failed to construct layout for envoy_map_entry block");
        let memory;
        unsafe {
            memory = alloc::alloc(layout) as *mut sys::envoy_map_entry;
        }

        for (i, entry) in self.0.into_iter().enumerate() {
            let envoy_entry = sys::envoy_map_entry {
                key: entry.0.into_envoy_data(),
                value: entry.1.into_envoy_data(),
            };
            unsafe {
                (*memory.add(i)) = envoy_entry;
            }
        }

        sys::envoy_map {
            length: length.try_into().unwrap(),
            entries: memory,
        }
    }
}

pub type Headers = Map;
pub type StatsTags = Map;

#[cfg(python)]
#[pyclass]
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    #[pyo3(get)]
    error_code: ErrorCode,
    #[pyo3(get)]
    message: String,
    #[pyo3(get)]
    attempt_count: i32,
}

#[cfg(not(python))]
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    error_code: ErrorCode,
    message: String,
    attempt_count: i32,
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

#[derive(Debug)]
pub struct StreamIntel {
    pub stream_id: i64,
    pub connection_id: i64,
    pub attempt_count: u64,
}

impl StreamIntel {
    fn from_envoy_stream_intel(envoy_stream_intel: sys::envoy_stream_intel) -> Self {
        Self {
            stream_id: envoy_stream_intel.stream_id,
            connection_id: envoy_stream_intel.connection_id,
            attempt_count: envoy_stream_intel.attempt_count,
        }
    }
}

// TODO: provide a context to this thing
pub type LoggerLog = fn(Data);

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

pub fn set_preferred_network(network: Network) {
    unsafe {
        sys::set_preferred_network(network.into_envoy_network());
    }
}

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
