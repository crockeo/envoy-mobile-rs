// we have a lot of todo!()s laying around,
// so unreachable code adds a lot of visual noise
#![allow(unreachable_code)]

use std::alloc;
use std::ffi;
use std::marker::PhantomData;
use std::mem;
use std::ptr;
use std::string::FromUtf8Error;

use crate::sys;

// TODO: some notes on high level design here:
//
// - i don't like having to wrap all of the callbacks in ptrs.
//   i know they're pointers normally, but i wish there was a more rustic way
//   to be able to express callbacks.
//   research this and maybe replace
//
// - limit the lifetime of the stream returned by the RunningEngine
//   to be that of the running engine

// TODO: go through and fix locations
// when there's an envoy_status_t returned
// but we don't do anything with it

struct Engine(isize);

impl Engine {
    fn new(callbacks: EngineCallbacks, logger: Logger, event_tracker: EventTracker) -> Self {
        let handle;
        unsafe {
            handle = sys::init_engine(
                callbacks.into_envoy_engine_callbacks(),
                logger.into_envoy_logger(),
                event_tracker.into_envoy_event_tracker(),
            )
            .try_into()
            .unwrap();
        }
        Self(handle)
    }

    fn run<S: AsRef<str>>(self, config: S, log_level: LogLevel) -> RunningEngine {
        let config = ffi::CString::new(config.as_ref()).unwrap();
        let log_level = ffi::CString::new(log_level.as_envoy_log_level()).unwrap();

        unsafe {
            // TODO: actually use the above config once i have config loading set up
            sys::run_engine(self.0, sys::config_template, log_level.into_raw());
        }
        RunningEngine(self.0)
    }
}

struct RunningEngine(isize);

impl RunningEngine {
    // TODO: change the way this is returned to limit
    // the lifetime of the Stream to the lifetime of the RunningEngine
    fn new_stream<'a>(&'a self) -> Stream<'a> {
	let handle;
	unsafe {
	    handle = sys::init_stream(self.0);
	}
	Stream {
	    handle,
	    _lifetime: PhantomData,
	}
    }

    fn terminate(self) {
        unsafe {
            sys::drain_connections(self.0);
            sys::terminate_engine(self.0);
        }
    }
}

struct Stream<'a> {
    handle: isize,
    _lifetime: PhantomData<&'a ()>,
}

impl<'a> Stream<'a> {
    fn start(
        self,
        http_callbacks: HTTPCallbacks,
        explicit_flow_control: bool,
    ) -> OpenStream<'a> {
        // /**
        //  * Open an underlying HTTP stream. Note: Streams must be started before other other interaction can
        //  * can occur.
        //  * @param stream, handle to the stream to be started.
        //  * @param callbacks, the callbacks that will run the stream callbacks.
        //  * @param explicit_flow_control, whether to enable explicit flow control on the response stream.
        //  * @return envoy_stream, with a stream handle and a success status, or a failure status.
        //  */
        // envoy_status_t start_stream(envoy_stream_t stream, envoy_http_callbacks callbacks,
        //                             bool explicit_flow_control);
	let envoy_http_callbacks = http_callbacks.into_envoy_http_callbacks();
	unsafe {
	    sys::start_stream(self.handle, envoy_http_callbacks, explicit_flow_control);
	}
	OpenStream {
	    handle: self.handle,
	    _lifetime: self._lifetime,
	}
    }
}

struct OpenStream<'a> {
    handle: isize,
    _lifetime: PhantomData<&'a ()>,
}

impl<'a> OpenStream<'a> {
    fn send_headers(&mut self, headers: Headers, done: bool) {
        // /**
        //  * Send headers over an open HTTP stream. This method can be invoked once and needs to be called
        //  * before send_data.
        //  * @param stream, the stream to send headers over.
        //  * @param headers, the headers to send.
        //  * @param end_stream, supplies whether this is headers only.
        //  * @return envoy_status_t, the resulting status of the operation.
        //  */
        // envoy_status_t send_headers(envoy_stream_t stream, envoy_headers headers, bool end_stream);
    }

    fn send_data(&mut self, data: Data, done: bool) {
        // /**
        //  * Send data over an open HTTP stream. This method can be invoked multiple times.
        //  * @param stream, the stream to send data over.
        //  * @param data, the data to send.
        //  * @param end_stream, supplies whether this is the last data in the stream.
        //  * @return envoy_status_t, the resulting status of the operation.
        //  */
        // envoy_status_t send_data(envoy_stream_t stream, envoy_data data, bool end_stream);
    }

    fn send_metadata(&mut self, metadata: Headers) {
        // /**
        //  * Send metadata over an HTTP stream. This method can be invoked multiple times.
        //  * @param stream, the stream to send metadata over.
        //  * @param metadata, the metadata to send.
        //  * @return envoy_status_t, the resulting status of the operation.
        //  */
        // envoy_status_t send_metadata(envoy_stream_t stream, envoy_headers metadata);
    }

    fn send_trailers(&mut self, trailers: Headers) {
        // /**
        //  * Send trailers over an open HTTP stream. This method can only be invoked once per stream.
        //  * Note that this method implicitly ends the stream.
        //  * @param stream, the stream to send trailers over.
        //  * @param trailers, the trailers to send.
        //  * @return envoy_status_t, the resulting status of the operation.
        //  */
        // envoy_status_t send_trailers(envoy_stream_t stream, envoy_headers trailers);
    }

    fn reset_stream(self) {
        // /**
        //  * Detach all callbacks from a stream and send an interrupt upstream if supported by transport.
        //  * @param stream, the stream to evict.
        //  * @return envoy_status_t, the resulting status of the operation.
        //  */
        // envoy_status_t reset_stream(envoy_stream_t stream);
    }

    fn read_data(&mut self, bytes_to_read: usize) {
        // /**
        //  * Notify the stream that the caller is ready to receive more data from the response stream. Only
        //  * used in explicit flow control mode.
        //  * @param bytes_to_read, the quantity of data the caller is prepared to process.
        //  */
        // envoy_status_t read_data(envoy_stream_t stream, size_t bytes_to_read);
    }
}

enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Critical,
    Off,
}

impl LogLevel {
    fn as_envoy_log_level(&self) -> &'static str {
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
            1 => Status::Failure,
            _ => panic!(),
        }
    }

    fn to_envoy_status(self) -> sys::envoy_status_t {
        match self {
            Status::Success => 0,
            Status::Failure => 1,
        }
    }
}

enum HistogramStatUnit {
    Unspecified,
    Bytes,
    Microseconds,
    Milliseconds,
}

impl HistogramStatUnit {
    fn from_envoy_histogram_stat_unit(
        envoy_histogram_stat_unit: sys::envoy_histogram_stat_unit_t,
    ) -> Self {
        match envoy_histogram_stat_unit {
            0 => HistogramStatUnit::Unspecified,
            1 => HistogramStatUnit::Bytes,
            2 => HistogramStatUnit::Microseconds,
            3 => HistogramStatUnit::Milliseconds,
            _ => panic!(),
        }
    }

    fn to_envoy_histogram_stat_unit(self) -> sys::envoy_histogram_stat_unit_t {
        match self {
            HistogramStatUnit::Unspecified => 0,
            HistogramStatUnit::Bytes => 1,
            HistogramStatUnit::Microseconds => 2,
            HistogramStatUnit::Milliseconds => 3,
        }
    }
}

enum ErrorCode {
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

    fn to_envoy_error_code(self) -> sys::envoy_error_code_t {
        match self {
            ErrorCode::UndefinedError => 0,
            ErrorCode::StreamReset => 1,
            ErrorCode::ConnectionFailure => 2,
            ErrorCode::BufferLimitExceeded => 3,
            ErrorCode::RequestTimeout => 4,
        }
    }
}

enum Network {
    Generic,
    WLAN,
    WWAN,
}

impl Network {
    fn from_envoy_network(envoy_network: sys::envoy_network_t) -> Self {
        match envoy_network {
            0 => Network::Generic,
            1 => Network::WLAN,
            2 => Network::WWAN,
            _ => panic!(),
        }
    }

    fn to_envoy_network(self) -> sys::envoy_network_t {
        match self {
            Network::Generic => 0,
            Network::WLAN => 1,
            Network::WWAN => 2,
        }
    }
}

#[derive(Debug)]
struct Data(Vec<u8>);

impl TryInto<String> for Data {
    type Error = ();

    fn try_into(self) -> Result<String, Self::Error> {
        Ok(String::from_utf8(self.0).unwrap())
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

        sys::envoy_data {
            length: length.try_into().unwrap(),
            bytes: ptr,
            release: Some(sys::envoy_noop_release),
            context: ptr::null_mut(),
        }
    }

    fn into_string(self) -> Result<String, FromUtf8Error> {
        String::from_utf8(self.0)
    }
}

struct MapEntry {
    key: Vec<u8>,
    value: Vec<u8>,
}

impl MapEntry {
    unsafe fn from_envoy_map_no_release(envoy_map_entry: &sys::envoy_map_entry) -> Self {
        let key_data = Data::from_envoy_data_no_release(&envoy_map_entry.key);
        let value_data = Data::from_envoy_data_no_release(&envoy_map_entry.value);
        Self {
            key: key_data.0,
            value: value_data.0,
        }
    }

    fn into_envoy_map_entry(self) -> sys::envoy_map_entry {
        sys::envoy_map_entry {
            key: Data::into_envoy_data(Data(self.key)),
            value: Data::into_envoy_data(Data(self.value)),
        }
    }
}

struct Map {
    entries: Vec<MapEntry>,
}

impl Map {
    unsafe fn from_envoy_map(envoy_map: sys::envoy_map) -> Self {
        let length = envoy_map.length.try_into().unwrap();
        let mut entries = Vec::with_capacity(length);
        for i in 0..length {
            let entry = &*envoy_map.entries.add(i);
            entries.push(MapEntry::from_envoy_map_no_release(entry));
        }
        sys::release_envoy_map(envoy_map);
        Self { entries }
    }

    fn into_envoy_map(self) -> sys::envoy_map {
        let mut envoy_map_entries = Vec::with_capacity(self.entries.len());
        for entry in self.entries.into_iter() {
            envoy_map_entries.push(entry.into_envoy_map_entry());
        }

        let ptr = envoy_map_entries.as_mut_ptr();
        let length = envoy_map_entries.len();
        sys::envoy_map {
            length: length.try_into().unwrap(),
            entries: ptr,
        }
    }
}

type Headers = Map;
type StatsTags = Map;

struct Error {
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

struct StreamIntel {
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

type OnHeaders = Box<dyn Fn(Headers, bool, StreamIntel)>;
type OnData = Box<dyn Fn(Data, bool, StreamIntel)>;
type OnMetadata = Box<dyn Fn(Headers, StreamIntel)>;
type OnTrailers = Box<dyn Fn(Headers, StreamIntel)>;
type OnError = Box<dyn Fn(Error, StreamIntel)>;
type OnComplete = Box<dyn Fn(StreamIntel)>;
type OnCancel = Box<dyn Fn(StreamIntel)>;
type OnSendWindowAvailable = Box<dyn Fn(StreamIntel)>;

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
    fn with_on_headers(mut self, on_headers: OnHeaders) -> Self {
        self.on_headers = Some(on_headers);
        self
    }

    fn with_on_data(mut self, on_data: OnData) -> Self {
        self.on_data = Some(on_data);
        self
    }

    fn with_on_metadata(mut self, on_metadata: OnMetadata) -> Self {
        self.on_metadata = Some(on_metadata);
        self
    }

    fn with_on_trailers(mut self, on_trailers: OnTrailers) -> Self {
        self.on_trailers = Some(on_trailers);
        self
    }

    fn with_on_error(mut self, on_error: OnError) -> Self {
        self.on_error = Some(on_error);
        self
    }

    fn with_on_complete(mut self, on_complete: OnComplete) -> Self {
        self.on_complete = Some(on_complete);
        self
    }

    fn with_on_cancel(mut self, on_cancel: OnCancel) -> Self {
        self.on_cancel = Some(on_cancel);
        self
    }

    fn with_on_send_window_available(
        mut self,
        on_send_window_available: OnSendWindowAvailable,
    ) -> Self {
        self.on_send_window_available = Some(on_send_window_available);
        self
    }

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
        context
    }

    unsafe extern "C" fn c_on_metadata(
        envoy_metadata: sys::envoy_headers,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        context
    }

    unsafe extern "C" fn c_on_trailers(
        envoy_trailers: sys::envoy_headers,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        context
    }

    unsafe extern "C" fn c_on_error(
        envoy_error: sys::envoy_error,
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
        context
    }

    unsafe extern "C" fn c_on_complete(
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
	context
    }

    unsafe extern "C" fn c_on_cancel(
        envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
	context
    }

    unsafe extern "C" fn c_on_send_window_available(
	envoy_stream_intel: sys::envoy_stream_intel,
        context: *mut ffi::c_void,
    ) -> *mut ffi::c_void {
	context
    }
}

type OnEngineRunning = Box<dyn Fn()>;
type OnExit = Box<dyn Fn()>;

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
    fn with_on_engine_running(mut self, on_engine_running: OnEngineRunning) -> Self {
        self.on_engine_running = Some(on_engine_running);
        self
    }

    fn with_on_exit(mut self, on_exit: OnExit) -> Self {
        self.on_exit = Some(on_exit);
        self
    }

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

type LoggerLog = Box<dyn Fn(Data)>;

struct Logger {
    log: Option<LoggerLog>,
}

impl Default for Logger {
    fn default() -> Self {
        Self { log: None }
    }
}

impl Logger {
    fn with_log(mut self, log: LoggerLog) -> Self {
        self.log = Some(log);
        self
    }

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

type EventTrackerTrack = Box<dyn Fn(Map)>;

struct EventTracker {
    track: Option<EventTrackerTrack>,
}

impl Default for EventTracker {
    fn default() -> Self {
        Self { track: None }
    }
}

impl EventTracker {
    fn with_track(mut self, track: EventTrackerTrack) -> Self {
        self.track = Some(track);
        self
    }

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

// /**
//  * Update the network interface to the preferred network for opening new streams.
//  * Note that this state is shared by all engines.
//  * @param network, the network to be preferred for new streams.
//  * @return envoy_status_t, the resulting status of the operation.
//  */
// envoy_status_t set_preferred_network(envoy_network_t network);

// /**
//  * Increment a counter with the given elements and by the given count.
//  * @param engine, the engine that owns the counter.
//  * @param elements, the string that identifies the counter to increment.
//  * @param tags, a map of {key, value} pairs of tags.
//  * @param count, the count to increment by.
//  */
// envoy_status_t record_counter_inc(envoy_engine_t, const char* elements, envoy_stats_tags tags,
//                                   uint64_t count);

// /**
//  * Set a gauge of a given string of elements with the given value.
//  * @param engine, the engine that owns the gauge.
//  * @param elements, the string that identifies the gauge to set value with.
//  * @param tags, a map of {key, value} pairs of tags.
//  * @param value, the value to set to the gauge.
//  */
// envoy_status_t record_gauge_set(envoy_engine_t engine, const char* elements, envoy_stats_tags tags,
//                                 uint64_t value);

// /**
//  * Add the gauge with the given string of elements and by the given amount.
//  * @param engine, the engine that owns the gauge.
//  * @param elements, the string that identifies the gauge to add to.
//  * @param tags, a map of {key, value} pairs of tags.
//  * @param amount, the amount to add to the gauge.
//  */
// envoy_status_t record_gauge_add(envoy_engine_t engine, const char* elements, envoy_stats_tags tags,
//                                 uint64_t amount);

// /**
//  * Subtract from the gauge with the given string of elements and by the given amount.
//  * @param engine, the engine that owns the gauge.
//  * @param elements, the string that identifies the gauge to subtract from.
//  * @param tags, a map of {key, value} pairs of tags.
//  * @param amount, amount to subtract from the gauge.
//  */
// envoy_status_t record_gauge_sub(envoy_engine_t engine, const char* elements, envoy_stats_tags tags,
//                                 uint64_t amount);

// /**
//  * Add another recorded amount to the histogram with the given string of elements and unit
//  * measurement.
//  * @param engine, the engine that owns the histogram.
//  * @param elements, the string that identifies the histogram to subtract from.
//  * @param tags, a map of {key, value} pairs of tags.
//  * @param value, amount to record as a new value for the histogram distribution.
//  * @param unit_measure, the unit of measurement (e.g. milliseconds, bytes, etc.)
//  */
// envoy_status_t record_histogram_value(envoy_engine_t engine, const char* elements,
//                                       envoy_stats_tags tags, uint64_t value,
//                                       envoy_histogram_stat_unit_t unit_measure);

// /**
//  * Flush the stats sinks outside of a flushing interval.
//  * Note: flushing before the engine has started will result in a no-op.
//  * Note: stats flushing may not be synchronous.
//  * Therefore, this function may return prior to flushing taking place.
//  */
// void flush_stats(envoy_engine_t engine);

// /**
//  * Collect a snapshot of all active stats.
//  * Note: this function may block for some time while collecting stats.
//  * @param engine, the engine whose stats to dump.
//  * @param data, out parameter to populate with stats data.
//  */
// envoy_status_t dump_stats(envoy_engine_t engine, envoy_data* data);

// /**
//  * Statically register APIs leveraging platform libraries.
//  * Warning: Must be completed before any calls to run_engine().
//  * @param name, identifier of the platform API
//  * @param api, type-erased c struct containing function pointers and context.
//  * @return envoy_status_t, the resulting status of the operation.
//  */
// envoy_status_t register_platform_api(const char* name, void* api);

// #ifdef __cplusplus
// } // functions
// #endif

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
	    let _ = self.condvar.wait_while(guard, |occurred| { !(*occurred) });
	}
    }

    fn make_running_engine() -> (RunningEngine, Arc<Event>) {
	let engine_running = Arc::new(Event::new());
	let engine_terminated = Arc::new(Event::new());

        let callbacks = {
            let engine_running = engine_running.clone();
            let engine_terminated = engine_terminated.clone();

            EngineCallbacks::default()
                .with_on_engine_running(Box::new(move || { engine_running.set(); }))
                .with_on_exit(Box::new(move || { engine_terminated.set(); }))
        };
        let logger = Logger::default().with_log(Box::new(|data| {
            print!("{}", data.into_string().unwrap());
        }));
        let event_tracker = EventTracker::default();

        // TODO: populate config
        let config: &str = "";
        let engine = Engine::new(callbacks, logger, event_tracker).run(config, LogLevel::Info);
	engine_running.wait();

	(engine, engine_terminated)
    }

    #[test]
    fn test_engine_lifecycle() {
	let (engine, engine_terminated) = make_running_engine();
        engine.terminate();
	engine_terminated.wait();
    }

    #[test]
    fn test_stream_lifecycle() {
	let (engine, engine_terminated) = make_running_engine();

	let http_callbacks = HTTPCallbacks::default()
	    .with_on_headers(Box::new(|headers: Map, end_stream: bool, stream_intel: StreamIntel| {
	    }));

	let mut stream = engine.new_stream().start(http_callbacks, false);
	// TODO: do something with the stream

	engine.terminate();
	engine_terminated.wait();
    }
}
