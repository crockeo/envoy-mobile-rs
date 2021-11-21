use std::alloc;
use std::ffi;
use std::marker::PhantomData;
use std::mem;
use std::ptr;
use std::string::FromUtf8Error;

use crate::sys;

// some things i don't like and want to work on
//
// - callbacks need to be wrapped in boxes all the time
//   look to see if there's a good way to represent them statically
//
// - represent states of a stream in types
//   instead of no-opping
//
// - check that envoy_status_t is good
//   and error if it's not
//
// - extend the lifetime of data passed across program boundaries
//   a la string views
//
// - config loading and parametrization with the engine builder

pub struct EngineBuilder {
    engine_callbacks: EngineCallbacks,
    logger: Logger,
    event_tracker: EventTracker,
}

impl EngineBuilder {
    pub fn new() -> Self {
        Self {
            engine_callbacks: EngineCallbacks::default(),
            logger: Logger::default(),
            event_tracker: EventTracker::default(),
        }
    }

    pub fn with_on_engine_running(mut self, on_engine_running: OnEngineRunning) -> Self {
        self.engine_callbacks.on_engine_running = Some(on_engine_running);
        self
    }

    pub fn with_on_exit(mut self, on_exit: OnExit) -> Self {
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

    pub fn build<S: AsRef<str>>(self, config: S, log_level: LogLevel) -> Engine {
        let envoy_callbacks = self.engine_callbacks.into_envoy_engine_callbacks();
        let envoy_logger = self.logger.into_envoy_logger();
        let envoy_event_tracker = self.event_tracker.into_envoy_event_tracker();
        let handle;
        unsafe {
            handle = sys::init_engine(envoy_callbacks, envoy_logger, envoy_event_tracker);
        }

        let config = ffi::CString::new(config.as_ref()).unwrap();
        let log_level = ffi::CString::new(log_level.into_envoy_log_level()).unwrap();
        unsafe {
            // TODO: actually use the above config once i have config loading set up
            sys::run_engine(handle, sys::config_template, log_level.into_raw());
        }
        Engine(handle)
    }
}

pub struct Engine(isize);

impl Engine {
    pub fn new_stream<'a>(&'a self) -> StreamPrototype<'a> {
        let handle;
        unsafe {
            handle = sys::init_stream(self.0);
        }
        StreamPrototype::new(handle)
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
        let mut envoy_data;
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

pub struct StreamPrototype<'a> {
    handle: isize,
    http_callbacks: HTTPCallbacks,
    _lifetime: PhantomData<&'a ()>,
}

impl<'a> StreamPrototype<'a> {
    fn new(handle: isize) -> Self {
        Self {
            handle,
            http_callbacks: HTTPCallbacks::default(),
            _lifetime: PhantomData,
        }
    }

    pub fn with_on_headers(mut self, on_headers: OnHeaders) -> Self {
        self.http_callbacks.on_headers = Some(on_headers);
        self
    }

    pub fn with_on_data(mut self, on_data: OnData) -> Self {
        self.http_callbacks.on_data = Some(on_data);
        self
    }

    pub fn with_on_metadata(mut self, on_metadata: OnMetadata) -> Self {
        self.http_callbacks.on_metadata = Some(on_metadata);
        self
    }

    pub fn with_on_trailers(mut self, on_trailers: OnTrailers) -> Self {
        self.http_callbacks.on_trailers = Some(on_trailers);
        self
    }

    pub fn with_on_error(mut self, on_error: OnError) -> Self {
        self.http_callbacks.on_error = Some(on_error);
        self
    }

    pub fn with_on_complete(mut self, on_complete: OnComplete) -> Self {
        self.http_callbacks.on_complete = Some(on_complete);
        self
    }

    pub fn with_on_cancel(mut self, on_cancel: OnCancel) -> Self {
        self.http_callbacks.on_cancel = Some(on_cancel);
        self
    }

    pub fn with_on_send_window_available(
        mut self,
        on_send_window_available: OnSendWindowAvailable,
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

    fn to_envoy_status(self) -> sys::envoy_status_t {
        match self {
            Status::Success => 0,
            Status::Failure => 1,
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

#[derive(Debug)]
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
    WLAN,
    WWAN,
}

impl Network {
    fn into_envoy_network(self) -> sys::envoy_network_t {
        match self {
            Network::Generic => 0,
            Network::WLAN => 1,
            Network::WWAN => 2,
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
            entries.push((
                Data::from(key),
                Data::from(value),
            ));
        }
        Map(entries)
    }
}

impl Map {
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

#[derive(Debug)]
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

pub type OnHeaders = Box<dyn Fn(Headers, bool, StreamIntel)>;
pub type OnData = Box<dyn Fn(Data, bool, StreamIntel)>;
pub type OnMetadata = Box<dyn Fn(Headers, StreamIntel)>;
pub type OnTrailers = Box<dyn Fn(Headers, StreamIntel)>;
pub type OnError = Box<dyn Fn(Error, StreamIntel)>;
pub type OnComplete = Box<dyn Fn(StreamIntel)>;
pub type OnCancel = Box<dyn Fn(StreamIntel)>;
pub type OnSendWindowAvailable = Box<dyn Fn(StreamIntel)>;

struct HTTPCallbacks {
    on_headers: Option<OnHeaders>,
    on_data: Option<OnData>,
    on_metadata: Option<OnMetadata>,
    on_trailers: Option<OnTrailers>,
    on_error: Option<OnError>,
    on_complete: Option<OnComplete>,
    on_cancel: Option<OnCancel>,
    on_send_window_available: Option<OnSendWindowAvailable>,
}

impl Default for HTTPCallbacks {
    fn default() -> Self {
        HTTPCallbacks {
            on_headers: None,
            on_data: None,
            on_metadata: None,
            on_trailers: None,
            on_error: None,
            on_complete: None,
            on_cancel: None,
            on_send_window_available: None,
        }
    }
}

impl HTTPCallbacks {
    fn into_envoy_http_callbacks(self) -> sys::envoy_http_callbacks {
        let http_callbacks = Box::into_raw(Box::new(self));
        sys::envoy_http_callbacks {
            on_headers: Some(HTTPCallbacks::c_on_headers),
            on_data: Some(HTTPCallbacks::c_on_data),
            on_metadata: Some(HTTPCallbacks::c_on_metadata),
            on_trailers: Some(HTTPCallbacks::c_on_trailers),
            on_error: Some(HTTPCallbacks::c_on_error),
            on_complete: Some(HTTPCallbacks::c_on_complete),
            on_cancel: Some(HTTPCallbacks::c_on_cancel),
            on_send_window_available: Some(HTTPCallbacks::c_on_send_window_available),
            context: http_callbacks as *mut ffi::c_void,
        }
    }

    unsafe extern "C" fn c_on_headers(
        envoy_headers: sys::envoy_headers,
        end_stream: bool,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks;
        let headers = Headers::from_envoy_map(envoy_headers);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_headers) = &(*http_callbacks).on_headers {
            on_headers(headers, end_stream, stream_intel);
        }
        context
    }

    unsafe extern "C" fn c_on_data(
        envoy_data: sys::envoy_data,
        end_stream: bool,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks;
        let data = Data::from_envoy_data(envoy_data);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_data) = &(*http_callbacks).on_data {
            on_data(data, end_stream, stream_intel);
        }
        context
    }

    unsafe extern "C" fn c_on_metadata(
        envoy_metadata: sys::envoy_headers,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks;
        let metadata = Headers::from_envoy_map(envoy_metadata);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_metadata) = &(*http_callbacks).on_metadata {
            on_metadata(metadata, stream_intel);
        }
        context
    }

    unsafe extern "C" fn c_on_trailers(
        envoy_trailers: sys::envoy_headers,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks;
        let trailers = Headers::from_envoy_map(envoy_trailers);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_trailers) = &(*http_callbacks).on_trailers {
            on_trailers(trailers, stream_intel);
        }
        context
    }

    unsafe extern "C" fn c_on_error(
        envoy_error: sys::envoy_error,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks;
        let error = Error::from_envoy_error(envoy_error);
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_error) = &(*http_callbacks).on_error {
            on_error(error, stream_intel);
        }
        context
    }

    unsafe extern "C" fn c_on_complete(
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks;
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_complete) = &(*http_callbacks).on_complete {
            on_complete(stream_intel);
        }
        context
    }

    unsafe extern "C" fn c_on_cancel(
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks;
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_cancel) = &(*http_callbacks).on_cancel {
            on_cancel(stream_intel);
        }
        context
    }

    unsafe extern "C" fn c_on_send_window_available(
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        let http_callbacks = context as *const HTTPCallbacks;
        let stream_intel = StreamIntel::from_envoy_stream_intel(envoy_stream_intel);
        if let Some(on_send_window_available) = &(*http_callbacks).on_send_window_available {
            on_send_window_available(stream_intel);
        }
        context
    }
}

pub type OnEngineRunning = Box<dyn Fn()>;
pub type OnExit = Box<dyn Fn()>;

struct EngineCallbacks {
    on_engine_running: Option<OnEngineRunning>,
    on_exit: Option<OnExit>,
}

impl Default for EngineCallbacks {
    fn default() -> Self {
        Self {
            on_engine_running: None,
            on_exit: None,
        }
    }
}

impl EngineCallbacks {
    fn into_envoy_engine_callbacks(self) -> sys::envoy_engine_callbacks {
        let engine_callbacks = Box::into_raw(Box::new(self));
        return sys::envoy_engine_callbacks {
            on_engine_running: Some(EngineCallbacks::c_on_engine_running),
            on_exit: Some(EngineCallbacks::c_on_exit),
            context: engine_callbacks as *mut ffi::c_void,
        };
    }

    unsafe extern "C" fn c_on_engine_running(context: *mut ffi::c_void) {
        let engine_callbacks = context as *mut EngineCallbacks;
        if let Some(on_engine_running) = &(*engine_callbacks).on_engine_running {
            on_engine_running();
        }
    }

    unsafe extern "C" fn c_on_exit(context: *mut ffi::c_void) {
        let engine_callbacks = Box::from_raw(context as *mut EngineCallbacks);
        if let Some(on_exit) = engine_callbacks.on_exit {
            on_exit();
        }
    }
}

pub type LoggerLog = Box<dyn Fn(Data)>;

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

pub type EventTrackerTrack = Box<dyn Fn(Map)>;

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Condvar;
    use std::sync::Mutex;

    use super::*;

    struct Event {
        occurred: Mutex<bool>,
        condvar: Condvar,
    }

    impl Event {
        fn new() -> Self {
            Self {
                occurred: Mutex::new(false),
                condvar: Condvar::new(),
            }
        }

        fn set(&self) {
            let mut guard = self.occurred.lock().unwrap();
            if !*guard {
                (*guard) = true;
                self.condvar.notify_all();
            }
        }

        fn wait(&self) {
            let guard = self.occurred.lock().unwrap();
            let _ = self.condvar.wait_while(guard, |occurred| !(*occurred));
        }
    }

    fn make_engine() -> (Engine, Arc<Event>) {
        let engine_running = Arc::new(Event::new());
        let engine_terminated = Arc::new(Event::new());

        let engine = {
            let engine_running = engine_running.clone();
            let engine_terminated = engine_terminated.clone();

            EngineBuilder::new()
                .with_on_engine_running(Box::new(move || {
                    engine_running.set();
                }))
                .with_on_exit(Box::new(move || {
                    engine_terminated.set();
                }))
                .with_log(Box::new(|data| {
                    print!("{}", String::try_from(data).unwrap());
                }))
                .build("", LogLevel::Debug)
        };
        engine_running.wait();

        (engine, engine_terminated)
    }

    #[test]
    fn test_data_lifecycle() {
        let data = Data::from("hello world");
        let expected_contents = data.0.clone();

        let converted_data;
        unsafe {
            converted_data = Data::from_envoy_data(data.into_envoy_data());
        }
        let contents = converted_data.0;

        assert_eq!(contents, expected_contents);
    }

    #[test]
    fn test_map_lifecycle() {
        let map = Map::from(vec![("hello", "world")]);
        let expected_contents = map.0.clone();

        let converted_map;
        unsafe {
            converted_map = Map::from_envoy_map(map.into_envoy_map());
        }
        let contents = converted_map.0;

        assert_eq!(contents, expected_contents);
    }

    #[test]
    fn test_engine_lifecycle() {
        let (engine, engine_terminated) = make_engine();
        engine.terminate();
        engine_terminated.wait();
    }

    #[test]
    fn test_stream_lifecycle() {
        let (engine, engine_terminated) = make_engine();

        let stream_complete = Arc::new(Event::new());
        let mut stream = engine
            .new_stream()
            .with_on_error(Box::new(move |error: Error, _| {
                // TODO: fail this test more gracefully :)
                panic!("{:?}", error);
            }))
            .with_on_complete({
                let stream_complete = stream_complete.clone();
                Box::new(move |_| {
                    stream_complete.set();
                })
            })
            .with_on_cancel({
                let stream_complete = stream_complete.clone();
                Box::new(move |_| {
                    stream_complete.set();
                })
            })
            .start(false);
        stream.send_headers(
            vec![
                (":method", "GET"),
                (":scheme", "https"),
                (":authority", "www.google.com"),
                (":path", "/"),
            ],
            true,
        );
        stream_complete.wait();
        // TODO: do something with the stream

        engine.terminate();
        engine_terminated.wait();
    }
}
