use std::alloc;
use std::collections::{BTreeMap, HashMap};
use std::ffi;
use std::marker::PhantomData;
use std::mem;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::ptr;
use std::str::Utf8Error;
use std::string::FromUtf8Error;

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

pub struct EngineBuilder<Context> {
    engine_callbacks: EngineCallbacks<Context>,
    logger: Logger,
    event_tracker: EventTracker,
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

impl<Context> EngineBuilder<Context> {
    pub fn new(callback_context: Context) -> Self {
        Self {
            engine_callbacks: EngineCallbacks::new(callback_context),
            logger: Logger::default(),
            event_tracker: EventTracker::default(),
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

    pub fn with_on_engine_running(mut self, on_engine_running: OnEngineRunning<Context>) -> Self {
        self.engine_callbacks.on_engine_running = Some(on_engine_running);
        self
    }

    pub fn with_on_exit(mut self, on_exit: OnExit<Context>) -> Self {
        self.engine_callbacks.on_exit = Some(on_exit);
        self
    }

    pub fn with_log(mut self, log: LoggerLog) -> Self {
        self.logger.log = Some(log);
        self
    }

    pub fn with_track(mut self, track: EventTrackerTrack) -> Self {
        self.event_tracker.track = Some(track);
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

    pub fn build(self, log_level: LogLevel) -> Engine {
        let envoy_callbacks = self.engine_callbacks.into_envoy_engine_callbacks();
        let envoy_logger = self.logger.into_envoy_logger();
        let envoy_event_tracker = self.event_tracker.into_envoy_event_tracker();
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
            // TODO: actually use the above config once i have config loading set up
            sys::run_engine(handle, config_cstr.as_ptr(), log_level.into_raw());
        }
        Engine(handle)
    }
}

pub struct Engine(isize);

impl Engine {
    pub fn new_stream<Context>(&'_ self, context: Context) -> StreamPrototype<'_, Context> {
        let handle;
        unsafe {
            handle = sys::init_stream(self.0);
        }
        StreamPrototype::new(handle, context)
    }

    pub fn record_counter_inc<S: AsRef<str>>(&self, elements: S, tags: StatsTags, count: usize) {
        let bytes = Vec::from_iter(elements.as_ref().bytes());
        let elements_cstr = ffi::CStr::from_bytes_with_nul(&bytes).unwrap();

        let envoy_stats_tags = tags.into_envoy_map();
        unsafe {
            sys::record_counter_inc(
                self.0,
                elements_cstr.as_ptr(),
                envoy_stats_tags,
                count.try_into().unwrap(),
            );
        }
    }

    pub fn record_gauge_set<S: AsRef<str>>(&self, elements: S, tags: StatsTags, value: usize) {
        let bytes = Vec::from_iter(elements.as_ref().bytes());
        let elements_cstr = ffi::CStr::from_bytes_with_nul(&bytes).unwrap();

        let envoy_stats_tags = tags.into_envoy_map();
        unsafe {
            sys::record_gauge_set(
                self.0,
                elements_cstr.as_ptr(),
                envoy_stats_tags,
                value.try_into().unwrap(),
            );
        }
    }

    pub fn record_gauge_add<S: AsRef<str>>(&self, elements: S, tags: StatsTags, amount: usize) {
        let bytes = Vec::from_iter(elements.as_ref().bytes());
        let elements_cstr = ffi::CStr::from_bytes_with_nul(&bytes).unwrap();

        let envoy_stats_tags = tags.into_envoy_map();
        unsafe {
            sys::record_gauge_add(
                self.0,
                elements_cstr.as_ptr(),
                envoy_stats_tags,
                amount.try_into().unwrap(),
            );
        }
    }

    pub fn record_gauge_sub<S: AsRef<str>>(&self, elements: S, tags: StatsTags, amount: usize) {
        let bytes = Vec::from_iter(elements.as_ref().bytes());
        let elements_cstr = ffi::CStr::from_bytes_with_nul(&bytes).unwrap();

        let envoy_stats_tags = tags.into_envoy_map();
        unsafe {
            sys::record_gauge_sub(
                self.0,
                elements_cstr.as_ptr(),
                envoy_stats_tags,
                amount.try_into().unwrap(),
            );
        }
    }

    pub fn record_histogram_value<S: AsRef<str>>(
        &self,
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
                self.0,
                elements_cstr.as_ptr(),
                envoy_stats_tags,
                amount.try_into().unwrap(),
                envoy_histogram_stat_unit,
            );
        }
    }

    pub fn flush_stats(&self) {
        unsafe {
            sys::flush_stats(self.0);
        }
    }

    pub fn dump_stats(&self) -> Data {
        let envoy_data;
        unsafe {
            envoy_data = sys::envoy_nodata;
            sys::dump_stats(
                self.0,
                &envoy_data as *const sys::envoy_data as *mut sys::envoy_data,
            );
            Data::from_envoy_data(envoy_data)
        }
    }

    pub fn terminate(self) {
        unsafe {
            sys::drain_connections(self.0);
            sys::terminate_engine(self.0);
        }
    }
}

pub struct StreamPrototype<'a, Context> {
    handle: isize,
    http_callbacks: HTTPCallbacks<Context>,
    _lifetime: PhantomData<&'a ()>,
}

impl<'a, Context> StreamPrototype<'a, Context> {
    fn new(handle: isize, context: Context) -> Self {
        Self {
            handle,
            http_callbacks: HTTPCallbacks::new(context),
            _lifetime: PhantomData,
        }
    }

    pub fn with_on_headers(mut self, on_headers: OnHeaders<Context>) -> Self {
        self.http_callbacks.on_headers = Some(on_headers);
        self
    }

    pub fn with_on_data(mut self, on_data: OnData<Context>) -> Self {
        self.http_callbacks.on_data = Some(on_data);
        self
    }

    pub fn with_on_metadata(mut self, on_metadata: OnMetadata<Context>) -> Self {
        self.http_callbacks.on_metadata = Some(on_metadata);
        self
    }

    pub fn with_on_trailers(mut self, on_trailers: OnTrailers<Context>) -> Self {
        self.http_callbacks.on_trailers = Some(on_trailers);
        self
    }

    pub fn with_on_error(mut self, on_error: OnError<Context>) -> Self {
        self.http_callbacks.on_error = Some(on_error);
        self
    }

    pub fn with_on_complete(mut self, on_complete: OnComplete<Context>) -> Self {
        self.http_callbacks.on_complete = Some(on_complete);
        self
    }

    pub fn with_on_cancel(mut self, on_cancel: OnCancel<Context>) -> Self {
        self.http_callbacks.on_cancel = Some(on_cancel);
        self
    }

    pub fn with_on_send_window_available(
        mut self,
        on_send_window_available: OnSendWindowAvailable<Context>,
    ) -> Self {
        self.http_callbacks.on_send_window_available = Some(on_send_window_available);
        self
    }

    pub fn start(self, explicit_flow_control: bool) -> Stream<'a> {
        let envoy_http_callbacks = self.http_callbacks.into_envoy_http_callbacks();
        unsafe {
            sys::start_stream(self.handle, envoy_http_callbacks, explicit_flow_control);
        }
        Stream {
            handle: self.handle,
            _lifetime: self._lifetime,
        }
    }
}

pub struct Stream<'a> {
    handle: isize,
    _lifetime: PhantomData<&'a ()>,
}

impl<'a> Stream<'a> {
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
}

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

#[derive(Debug, PartialEq)]
pub enum ErrorCode {
    UndefinedError,
    StreamReset,
    ConnectionFailure,
    BufferLimitExceeded,
    RequestTimeout,
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

impl<S: AsRef<str>> From<S> for Data {
    fn from(string: S) -> Self {
        let string = string.as_ref().to_owned();
        Self(Vec::from_iter(string.into_bytes()))
    }
}

impl From<Data> for Vec<u8> {
    fn from(data: Data) -> Vec<u8> {
	data.0
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
        String::from_utf8(data.0)
    }
}

impl Data {
    unsafe fn from_envoy_data_no_release(envoy_data: &sys::envoy_data) -> Self {
        let length = envoy_data.length.try_into().unwrap();
        let layout = alloc::Layout::array::<u8>(length).unwrap();
        let bytes = alloc::alloc(layout);
        ptr::copy(envoy_data.bytes, bytes, length);
        Self(Vec::from_raw_parts(bytes, length, length))
    }

    unsafe fn from_envoy_data(envoy_data: sys::envoy_data) -> Self {
        let data = Self::from_envoy_data_no_release(&envoy_data);
        sys::release_envoy_data(envoy_data);
        data
    }

    fn into_envoy_data(self) -> sys::envoy_data {
        let mut vec = mem::ManuallyDrop::new(self.0);
        let ptr = vec.as_mut_ptr();
        let length = vec.len();

        // TODO: this is leaking memory!
        // make it so we keep around the pointer to the vector
        // and can therefore release it later
        sys::envoy_data {
            length: length.try_into().unwrap(),
            bytes: ptr,
            release: Some(sys::envoy_noop_release),
            context: ptr::null_mut(),
        }
    }
}

#[derive(Debug)]
pub struct Map(Vec<(Data, Data)>);

impl<S: AsRef<str>> From<Vec<(S, S)>> for Map {
    fn from(pairs: Vec<(S, S)>) -> Self {
        let mut entries = Vec::with_capacity(pairs.len());
        for (key, value) in pairs.into_iter() {
            entries.push((Data::from(key), Data::from(value)));
        }
        Map(entries)
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
    pub fn new_request_headers<S: AsRef<str>>(method: Method, scheme: Scheme, authority: S, path: S) -> Self {
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
        let mut envoy_map_entries = Vec::with_capacity(self.0.len());
        for entry in self.0.into_iter() {
            envoy_map_entries.push(sys::envoy_map_entry {
                key: entry.0.into_envoy_data(),
                value: entry.1.into_envoy_data(),
            });
        }

        let mut envoy_map_entries = mem::ManuallyDrop::new(envoy_map_entries);
        let ptr = envoy_map_entries.as_mut_ptr();
        let length = envoy_map_entries.len();
        sys::envoy_map {
            length: length.try_into().unwrap(),
            entries: ptr,
        }
    }
}

pub type Headers = Map;
pub type StatsTags = Map;

#[derive(Debug, PartialEq)]
pub struct Error {
    error_code: ErrorCode,
    message: Data,
    attempt_count: i32,
}

impl Error {
    unsafe fn from_envoy_error(envoy_error: sys::envoy_error) -> Self {
        let error = Self {
            error_code: ErrorCode::from_envoy_error_code(envoy_error.error_code),
            message: Data::from_envoy_data_no_release(&envoy_error.message),
            attempt_count: envoy_error.attempt_count,
        };
        sys::release_envoy_error(envoy_error);
        error
    }
}

#[derive(Debug)]
pub struct StreamIntel {
    stream_id: i64,
    connection_id: i64,
    attempt_count: u64,
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

pub type OnHeaders<Context> = fn(&Context, Headers, bool, StreamIntel);
pub type OnData<Context> = fn(&Context, Data, bool, StreamIntel);
pub type OnMetadata<Context> = fn(&Context, Headers, StreamIntel);
pub type OnTrailers<Context> = fn(&Context, Headers, StreamIntel);
pub type OnError<Context> = fn(&Context, Error, StreamIntel);
pub type OnComplete<Context> = fn(&Context, StreamIntel);
pub type OnCancel<Context> = fn(&Context, StreamIntel);
pub type OnSendWindowAvailable<Context> = fn(&Context, StreamIntel);

struct HTTPCallbacks<Context> {
    on_headers: Option<OnHeaders<Context>>,
    on_data: Option<OnData<Context>>,
    on_metadata: Option<OnMetadata<Context>>,
    on_trailers: Option<OnTrailers<Context>>,
    on_error: Option<OnError<Context>>,
    on_complete: Option<OnComplete<Context>>,
    on_cancel: Option<OnCancel<Context>>,
    on_send_window_available: Option<OnSendWindowAvailable<Context>>,
    context: Context,
}

impl<Context> HTTPCallbacks<Context> {
    fn new(context: Context) -> Self {
        Self {
            on_headers: None,
            on_data: None,
            on_metadata: None,
            on_trailers: None,
            on_error: None,
            on_complete: None,
            on_cancel: None,
            on_send_window_available: None,
            context,
        }
    }

    fn into_envoy_http_callbacks(self) -> sys::envoy_http_callbacks {
        let http_callbacks = Box::into_raw(Box::new(self));
        sys::envoy_http_callbacks {
            on_headers: Some(HTTPCallbacks::<Context>::c_on_headers),
            on_data: Some(HTTPCallbacks::<Context>::c_on_data),
            on_metadata: Some(HTTPCallbacks::<Context>::c_on_metadata),
            on_trailers: Some(HTTPCallbacks::<Context>::c_on_trailers),
            on_error: Some(HTTPCallbacks::<Context>::c_on_error),
            on_complete: Some(HTTPCallbacks::<Context>::c_on_complete),
            on_cancel: Some(HTTPCallbacks::<Context>::c_on_cancel),
            on_send_window_available: Some(HTTPCallbacks::<Context>::c_on_send_window_available),
            context: http_callbacks as *mut ffi::c_void,
        }
    }

    unsafe extern "C" fn c_on_headers(
        envoy_headers: sys::envoy_headers,
        end_stream: bool,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks<Context>;
        let headers = Headers::from_envoy_map(envoy_headers);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_headers) = &(*http_callbacks).on_headers {
            on_headers(
                &(*http_callbacks).context,
                headers,
                end_stream,
                stream_intel,
            );
        }
        context
    }

    unsafe extern "C" fn c_on_data(
        envoy_data: sys::envoy_data,
        end_stream: bool,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks<Context>;
        let data = Data::from_envoy_data(envoy_data);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_data) = &(*http_callbacks).on_data {
            on_data(&(*http_callbacks).context, data, end_stream, stream_intel);
        }
        context
    }

    unsafe extern "C" fn c_on_metadata(
        envoy_metadata: sys::envoy_headers,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks<Context>;
        let metadata = Headers::from_envoy_map(envoy_metadata);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_metadata) = &(*http_callbacks).on_metadata {
            on_metadata(&(*http_callbacks).context, metadata, stream_intel);
        }
        context
    }

    unsafe extern "C" fn c_on_trailers(
        envoy_trailers: sys::envoy_headers,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks<Context>;
        let trailers = Headers::from_envoy_map(envoy_trailers);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_trailers) = &(*http_callbacks).on_trailers {
            on_trailers(&(*http_callbacks).context, trailers, stream_intel);
        }
        context
    }

    unsafe extern "C" fn c_on_error(
        envoy_error: sys::envoy_error,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks<Context>;
        let error = Error::from_envoy_error(envoy_error);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_error) = &(*http_callbacks).on_error {
            on_error(&(*http_callbacks).context, error, stream_intel);
        }
        context
    }

    unsafe extern "C" fn c_on_complete(
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks<Context>;
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_complete) = &(*http_callbacks).on_complete {
            on_complete(&(*http_callbacks).context, stream_intel);
        }
        context
    }

    unsafe extern "C" fn c_on_cancel(
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks<Context>;
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_cancel) = &(*http_callbacks).on_cancel {
            on_cancel(&(*http_callbacks).context, stream_intel);
        }
        context
    }

    unsafe extern "C" fn c_on_send_window_available(
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks<Context>;
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_send_window_available) = &(*http_callbacks).on_send_window_available {
            on_send_window_available(&(*http_callbacks).context, stream_intel);
        }
        context
    }
}

pub type OnEngineRunning<Context> = fn(&Context);
pub type OnExit<Context> = fn(&Context);

struct EngineCallbacks<Context> {
    on_engine_running: Option<OnEngineRunning<Context>>,
    on_exit: Option<OnExit<Context>>,
    context: Context,
}

impl<Context> EngineCallbacks<Context> {
    fn new(context: Context) -> Self {
        Self {
            on_engine_running: None,
            on_exit: None,
            context,
        }
    }

    fn into_envoy_engine_callbacks(self) -> sys::envoy_engine_callbacks {
        let engine_callbacks = Box::into_raw(Box::new(self));
        sys::envoy_engine_callbacks {
            on_engine_running: Some(EngineCallbacks::<Context>::c_on_engine_running),
            on_exit: Some(EngineCallbacks::<Context>::c_on_exit),
            context: engine_callbacks as *mut ffi::c_void,
        }
    }

    unsafe extern "C" fn c_on_engine_running(context: *mut ffi::c_void) {
        let engine_callbacks = context as *mut EngineCallbacks<Context>;
        if let Some(on_engine_running) = &(*engine_callbacks).on_engine_running {
            on_engine_running(&(*engine_callbacks).context);
        }
    }

    unsafe extern "C" fn c_on_exit(context: *mut ffi::c_void) {
        let engine_callbacks = Box::from_raw(context as *mut EngineCallbacks<Context>);
        if let Some(on_exit) = engine_callbacks.on_exit {
            on_exit(&engine_callbacks.context);
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

// TODO: provide a context to this thing
pub type EventTrackerTrack = fn(Map);

struct EventTracker {
    track: Option<EventTrackerTrack>,
}

impl Default for EventTracker {
    fn default() -> Self {
        Self { track: None }
    }
}

impl EventTracker {
    fn into_envoy_event_tracker(self) -> sys::envoy_event_tracker {
        let event_tracker = Box::into_raw(Box::new(self));
        sys::envoy_event_tracker {
            track: Some(EventTracker::c_track),
            context: event_tracker as *mut ffi::c_void,
        }
    }

    unsafe extern "C" fn c_track(envoy_event: sys::envoy_map, context: *const ffi::c_void) {
        let event_tracker = context as *const EventTracker;
        let event = Map::from_envoy_map(envoy_event);
        if let Some(track) = &(*event_tracker).track {
            track(event);
        }
    }
}

pub fn set_preferred_network(network: Network) {
    unsafe {
        sys::set_preferred_network(network.into_envoy_network());
    }
}

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

impl Method {
    fn into_envoy_method(self) -> &'static str {
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

impl Scheme {
    fn into_envoy_scheme(self) -> &'static str {
	match self {
	    Scheme::Http => "http",
	    Scheme::Https => "https",
	}
    }
}
