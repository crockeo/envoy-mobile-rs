use envoy_mobile_sys;

use std::ffi::c_void;
use std::ptr;
use std::str::Utf8Error;
use std::sync::{Arc, Mutex};

use crate::bridge_util::{Data, HTTPError, Headers};
use crate::result::{Error, Result};

type OnHeaders<T> = fn(&Arc<T>, Headers, bool);
type OnData<T> = fn(&Arc<T>, Data, bool);
type OnMetadata<T> = fn(&Arc<T>, Headers);
type OnTrailers<T> = fn(&Arc<T>, Headers);
type OnError<T> = fn(&Arc<T>, HTTPError);
type OnComplete<T> = fn(&Arc<T>);
type OnCancel<T> = fn(&Arc<T>);

struct StreamCallbacks<T> {
    on_headers: Option<OnHeaders<T>>,
    on_data: Option<OnData<T>>,
    on_metadata: Option<OnMetadata<T>>,
    on_trailers: Option<OnTrailers<T>>,
    on_error: Option<OnError<T>>,
    on_complete: Option<OnComplete<T>>,
    on_cancel: Option<OnCancel<T>>,
}

impl<T> StreamCallbacks<T> {
    fn new() -> Self {
        Self {
            on_headers: None,
            on_data: None,
            on_metadata: None,
            on_trailers: None,
            on_error: None,
            on_complete: None,
            on_cancel: None,
        }
    }
}

pub struct StreamBuilder<T> {
    context: Arc<T>,
    handle: envoy_mobile_sys::envoy_engine_t,
    stream_callbacks: StreamCallbacks<T>,
}

impl<T: Sync> StreamBuilder<T> {
    pub fn new(context: Arc<T>, handle: envoy_mobile_sys::envoy_stream_t) -> Self {
        Self {
            context,
            handle,
            stream_callbacks: StreamCallbacks::new(),
        }
    }

    pub fn build(self) -> Result<Stream<T>> {
        Stream::new(self.context, self.handle, self.stream_callbacks)
    }

    pub fn set_on_headers(mut self, on_headers: OnHeaders<T>) -> Self {
        self.stream_callbacks.on_headers = Some(on_headers);
        self
    }

    pub fn set_on_data(mut self, on_data: OnData<T>) -> Self {
        self.stream_callbacks.on_data = Some(on_data);
        self
    }

    pub fn set_on_metadata(mut self, on_metadata: OnMetadata<T>) -> Self {
        self.stream_callbacks.on_metadata = Some(on_metadata);
        self
    }

    pub fn set_on_trailers(mut self, on_trailers: OnTrailers<T>) -> Self {
        self.stream_callbacks.on_trailers = Some(on_trailers);
        self
    }

    pub fn set_on_error(mut self, on_error: OnError<T>) -> Self {
        self.stream_callbacks.on_error = Some(on_error);
        self
    }

    pub fn set_on_complete(mut self, on_complete: OnComplete<T>) -> Self {
        self.stream_callbacks.on_complete = Some(on_complete);
        self
    }

    pub fn set_on_cancel(mut self, on_cancel: OnCancel<T>) -> Self {
        self.stream_callbacks.on_cancel = Some(on_cancel);
        self
    }
}

struct StreamContextWrapper<T> {
    context: Arc<T>,
    stream_callbacks: StreamCallbacks<T>,
}

// TODO: restructure this API so that only valid actions can be taken on a Stream; e.g. once you've
// send data ensure the user can't try to send headers again
pub struct Stream<T> {
    context_wrapper_ptr: *mut StreamContextWrapper<T>,
    handle: envoy_mobile_sys::envoy_stream_t,
    request_sent: Mutex<bool>,
}

impl<T: Sync> Stream<T> {
    fn new(
        context: Arc<T>,
        handle: envoy_mobile_sys::envoy_stream_t,
        stream_callbacks: StreamCallbacks<T>,
    ) -> Result<Self> {
        let context_wrapper = StreamContextWrapper {
            context,
            stream_callbacks,
        };
        let context_wrapper_ptr = Box::into_raw(Box::new(context_wrapper));

        let envoy_stream_callbacks = envoy_mobile_sys::envoy_http_callbacks {
            on_headers: Some(Stream::<T>::on_headers),
            on_data: Some(Stream::<T>::on_data),
            on_metadata: Some(Stream::<T>::on_metadata),
            on_trailers: Some(Stream::<T>::on_trailers),
            on_error: Some(Stream::<T>::on_error),
            on_complete: Some(Stream::<T>::on_complete),
            on_cancel: Some(Stream::<T>::on_cancel),
            context: context_wrapper_ptr as *mut c_void,
        };

        let status;
        unsafe {
            status = envoy_mobile_sys::start_stream(handle, envoy_stream_callbacks);
        }
        if status == 1 {
            return Err(Error::CouldNotInit);
        }

        Ok(Self {
            context_wrapper_ptr,
            handle,
            request_sent: Mutex::new(false),
        })
    }

    pub fn send_headers(&mut self, headers: Headers, end_stream: bool) -> Result<&mut Self> {
        let status;
        unsafe {
            status =
                envoy_mobile_sys::send_headers(self.handle, headers.as_envoy_headers(), end_stream);
        }
        if status == 1 {
            return Err(Error::FailedToSend("headers"));
        }
        *self.request_sent.lock().unwrap() = true;
        Ok(self)
    }

    pub fn send_data(&mut self, data: Data, end_stream: bool) -> Result<&mut Self> {
        let status;
        unsafe {
            status = envoy_mobile_sys::send_data(self.handle, data.as_envoy_data(), end_stream);
        }
        if status == 1 {
            return Err(Error::FailedToSend("data"));
        }
        Ok(self)
    }

    pub fn send_metadata(&mut self, metadata: Headers) -> Result<&mut Self> {
        let status;
        unsafe {
            status = envoy_mobile_sys::send_metadata(self.handle, metadata.as_envoy_headers());
        }
        if status == 1 {
            return Err(Error::FailedToSend("metadata"));
        }
        Ok(self)
    }

    pub fn send_trailers(&mut self, trailers: Headers) -> Result<&mut Self> {
        let status;
        unsafe {
            status = envoy_mobile_sys::send_trailers(self.handle, trailers.as_envoy_headers());
        }
        if status == 1 {
            return Err(Error::FailedToSend("trailers"));
        }
        Ok(self)
    }

    unsafe extern "C" fn on_headers(
        envoy_headers: envoy_mobile_sys::envoy_headers,
        end_stream: bool,
        context: *mut c_void,
    ) -> *mut c_void {
        let context = context as *const StreamContextWrapper<T>;
        if let Some(on_headers) = &(*context).stream_callbacks.on_headers {
            on_headers(
                &(*context).context,
                Headers::from_envoy_headers(envoy_headers),
                end_stream,
            );
        }
        ptr::null_mut::<c_void>()
    }

    unsafe extern "C" fn on_data(
        envoy_data: envoy_mobile_sys::envoy_data,
        end_stream: bool,
        context: *mut c_void,
    ) -> *mut c_void {
        let context = context as *const StreamContextWrapper<T>;
        if let Some(on_data) = &(*context).stream_callbacks.on_data {
            on_data(
                &(*context).context,
                Data::from_envoy_data(envoy_data),
                end_stream,
            );
        }
        ptr::null_mut::<c_void>()
    }

    unsafe extern "C" fn on_metadata(
        envoy_metadata: envoy_mobile_sys::envoy_headers,
        context: *mut c_void,
    ) -> *mut c_void {
        let context = context as *const StreamContextWrapper<T>;
        if let Some(on_metadata) = &(*context).stream_callbacks.on_metadata {
            on_metadata(
                &(*context).context,
                Headers::from_envoy_headers(envoy_metadata),
            );
        }
        ptr::null_mut::<c_void>()
    }

    unsafe extern "C" fn on_trailers(
        envoy_trailers: envoy_mobile_sys::envoy_headers,
        context: *mut c_void,
    ) -> *mut c_void {
        let context = context as *const StreamContextWrapper<T>;
        if let Some(on_trailers) = &(*context).stream_callbacks.on_trailers {
            on_trailers(
                &(*context).context,
                Headers::from_envoy_headers(envoy_trailers),
            );
        }
        ptr::null_mut::<c_void>()
    }

    // TODO: try to elicit an error where none of these are called and we leak the context. i'm
    // not sure if it's possible but i want to try
    unsafe extern "C" fn on_error(
        envoy_error: envoy_mobile_sys::envoy_error,
        context: *mut c_void,
    ) -> *mut c_void {
        let context = context as *mut StreamContextWrapper<T>;
        if let Some(on_error) = &(*context).stream_callbacks.on_error {
            on_error(
                &(*context).context,
                HTTPError::from_envoy_error(envoy_error),
            );
        }
        let _ = Box::from_raw(context);
        ptr::null_mut::<c_void>()
    }

    unsafe extern "C" fn on_complete(context: *mut c_void) -> *mut c_void {
        let context = context as *mut StreamContextWrapper<T>;
        if let Some(on_complete) = &(*context).stream_callbacks.on_complete {
            on_complete(&(*context).context);
        }
        let _ = Box::from_raw(context);
        ptr::null_mut::<c_void>()
    }

    unsafe extern "C" fn on_cancel(context: *mut c_void) -> *mut c_void {
        let context = context as *mut StreamContextWrapper<T>;
        if let Some(on_cancel) = &(*context).stream_callbacks.on_cancel {
            on_cancel(&(*context).context);
        }
        let _ = Box::from_raw(context);
        ptr::null_mut::<c_void>()
    }
}

impl<T> Drop for Stream<T> {
    fn drop(&mut self) {
        if let Ok(request_sent) = self.request_sent.lock() {
            if !*request_sent {
                // SAFETY: We only deallocate the context ptr when there as not been a request
                // sent. Therefore the context will not be used from another thread.
                //
                // If a request has been sent, then the context will be deallocated from inside one
                // of the terminal callbacks: on_error, on_complete, or on_cancel.
                unsafe {
                    let _ = Box::from_raw(self.context_wrapper_ptr);
                }
            }
        }

        // SAFETY: this is trivially safe, as:
        //   - if handle is invalid, nothing happens
        //   - if the handle is already reset, nothing happens
        //   - if the handle is valid and active, it gets reset
        unsafe {
            envoy_mobile_sys::reset_stream(self.handle);
        }
    }
}
