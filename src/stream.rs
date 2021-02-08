use envoy_mobile_sys;

use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem;
use std::ptr;
use std::sync::Mutex;

use crate::bridge_util::{Data, HTTPError, Headers};
use crate::callback_futures::{CallbackFuture, CallbackStream};
use crate::result::{EnvoyError, EnvoyResult};

struct StreamContext {
    // non-terminal callbacks
    on_headers: CallbackStream<EnvoyResult<Headers>>,
    on_data: CallbackStream<EnvoyResult<Data>>,
    on_metadata: CallbackFuture<EnvoyResult<Headers>>,
    on_trailers: CallbackFuture<EnvoyResult<Headers>>,

    // terminal callbacks
    on_error: CallbackFuture<EnvoyResult<HTTPError>>,
    on_complete: CallbackFuture<EnvoyResult<()>>,
    on_cancel: CallbackFuture<EnvoyResult<()>>,
}

// these traits define the capabilities of a specialized Stream instance. multiple stages may be
// able to perform an action, so we separate the capability from the witness of the capability.
pub trait CanSendHeaders {}

pub trait CanSendData {}
impl CanSendData for dyn CanSendHeaders {}

pub trait CanSendTrailers {}
impl CanSendTrailers for dyn CanSendData {}

// each of these structs act as a "witness" to the capability of being able to perform some action.
// the stream below is specialized over these structs to ensure only valid actions can be taken on
// the struct
pub struct SendingHeaders {}
impl CanSendHeaders for SendingHeaders {}

pub struct SendingData {}
impl CanSendData for SendingData {}

pub struct SendingTrailers {}
impl CanSendTrailers for SendingTrailers {}

pub struct Complete {}

pub struct Stream<T> {
    context_ptr: *mut StreamContext,
    handle: envoy_mobile_sys::envoy_stream_t,
    request_sent: Mutex<bool>,
    witness: PhantomData<T>,
}

fn stream_with_capability<Existing, New>(stream: Stream<Existing>) -> Stream<New> {
    Stream::<New> {
        context_ptr: stream.context_ptr,
        handle: stream.handle,
        request_sent: stream.request_sent,
        witness: PhantomData,
    }
}

impl Stream<SendingHeaders> {
    pub fn new(handle: envoy_mobile_sys::envoy_stream_t) -> EnvoyResult<Self> {
        let context = StreamContext {
            on_headers: CallbackStream::new(),
            on_data: CallbackStream::new(),
            on_metadata: CallbackFuture::new(),
            on_trailers: CallbackFuture::new(),

            on_error: CallbackFuture::new(),
            on_complete: CallbackFuture::new(),
            on_cancel: CallbackFuture::new(),
        };
        let context_ptr = Box::into_raw(Box::new(context));

        let envoy_stream_callbacks = envoy_mobile_sys::envoy_http_callbacks {
            on_headers: Some(dispatch_on_headers),
            on_data: Some(dispatch_on_data),
            on_metadata: Some(dispatch_on_metadata),
            on_trailers: Some(dispatch_on_trailers),
            on_error: Some(dispatch_on_error),
            on_complete: Some(dispatch_on_complete),
            on_cancel: Some(dispatch_on_cancel),
            context: context_ptr as *mut c_void,
        };

        let status;
        unsafe {
            status = envoy_mobile_sys::start_stream(handle, envoy_stream_callbacks);
        }
        if status == 1 {
            return Err(EnvoyError::CouldNotInit);
        }

        Ok(Stream::<SendingHeaders> {
            context_ptr,
            handle,
            request_sent: Mutex::new(false),
            witness: PhantomData,
        })
    }
}

impl<T: CanSendHeaders> Stream<T> {
    fn send_headers_impl<U>(self, headers: Headers, end_stream: bool) -> EnvoyResult<Stream<U>> {
        let status;
        unsafe {
            status =
                envoy_mobile_sys::send_headers(self.handle, headers.as_envoy_headers(), end_stream);
        }
        if status == 1 {
            return Err(EnvoyError::FailedToSend("headers"));
        }
        *self.request_sent.lock().unwrap() = true;
	Ok(mem::transmute9
        Ok(stream_with_capability(self))
    }

    pub fn send_headers(self, headers: Headers) -> EnvoyResult<Stream<SendingHeaders>> {
        self.send_headers_impl::<SendingHeaders>(headers, false)
    }

    pub fn send_headers_and_close(self, headers: Headers) -> EnvoyResult<Stream<Complete>> {
        self.send_headers_impl::<Complete>(headers, true)
    }
}

impl<T: CanSendData> Stream<T> {
    pub fn send_data(self, data: Data, end_stream: bool) -> EnvoyResult<Stream<SendingData>> {
        let status;
        unsafe {
            status = envoy_mobile_sys::send_data(self.handle, data.as_envoy_data(), end_stream);
        }
        if status == 1 {
            return Err(EnvoyError::FailedToSend("data"));
        }
        Ok(stream_with_capability(self))
    }

    pub fn send_metadata(self, metadata: Headers) -> EnvoyResult<Stream<SendingData>> {
        let status;
        unsafe {
            status = envoy_mobile_sys::send_metadata(self.handle, metadata.as_envoy_headers());
        }
        if status == 1 {
            return Err(EnvoyError::FailedToSend("metadata"));
        }
        Ok(stream_with_capability(self))
    }
}

impl<T: CanSendTrailers> Stream<T> {
    pub fn send_trailers(self, trailers: Headers) -> EnvoyResult<Stream<Complete>> {
        let status;
        unsafe {
            status = envoy_mobile_sys::send_trailers(self.handle, trailers.as_envoy_headers());
        }
        if status == 1 {
            return Err(EnvoyError::FailedToSend("trailers"));
        }
        Ok(stream_with_capability(self))
    }
}

impl Stream<Complete> {
    pub fn on_headers(&self) -> &CallbackStream<EnvoyResult<Headers>> {
        unsafe { &(*self.context_ptr).on_headers }
    }

    pub fn on_data(&self) -> &CallbackStream<EnvoyResult<Data>> {
        unsafe { &(*self.context_ptr).on_data }
    }

    pub fn on_metadata(&self) -> &CallbackFuture<EnvoyResult<Headers>> {
        unsafe { &(*self.context_ptr).on_metadata }
    }

    pub fn on_trailers(&self) -> &CallbackFuture<EnvoyResult<Headers>> {
        unsafe { &(*self.context_ptr).on_trailers }
    }

    pub fn on_error(&self) -> &CallbackFuture<EnvoyResult<HTTPError>> {
        unsafe { &(*self.context_ptr).on_error }
    }

    pub fn on_complete(&self) -> &CallbackFuture<EnvoyResult<()>> {
        unsafe { &(*self.context_ptr).on_complete }
    }

    pub fn on_cancel(&self) -> &CallbackFuture<EnvoyResult<()>> {
        unsafe { &(*self.context_ptr).on_cancel }
    }
}

unsafe extern "C" fn dispatch_on_headers(
    envoy_headers: envoy_mobile_sys::envoy_headers,
    end_stream: bool,
    context: *mut c_void,
) -> *mut c_void {
    let context = context as *const StreamContext;
    let on_headers = &(*context).on_headers;
    on_headers.put_and_report(Headers::from_envoy_headers(envoy_headers));
    if end_stream {
        on_headers.close_and_report();
    }
    ptr::null_mut::<c_void>()
}

unsafe extern "C" fn dispatch_on_data(
    envoy_data: envoy_mobile_sys::envoy_data,
    end_stream: bool,
    context: *mut c_void,
) -> *mut c_void {
    let context = context as *const StreamContext;
    (*context).on_headers.maybe_close();
    let on_data = &(*context).on_data;
    on_data.put_and_report(Data::from_envoy_data(envoy_data));
    if end_stream {
        on_data.close_and_report();
    }
    ptr::null_mut::<c_void>()
}

unsafe extern "C" fn dispatch_on_metadata(
    envoy_metadata: envoy_mobile_sys::envoy_headers,
    context: *mut c_void,
) -> *mut c_void {
    let context = context as *mut StreamContext;
    let on_metadata = &(*context).on_metadata;
    on_metadata.put_and_report(Headers::from_envoy_headers(envoy_metadata));
    ptr::null_mut::<c_void>()
}

unsafe extern "C" fn dispatch_on_trailers(
    envoy_trailers: envoy_mobile_sys::envoy_headers,
    context: *mut c_void,
) -> *mut c_void {
    let context = context as *mut StreamContext;
    let on_trailers = &(*context).on_trailers;
    on_trailers.put_and_report(Headers::from_envoy_headers(envoy_trailers));
    ptr::null_mut::<c_void>()
}

unsafe extern "C" fn dispatch_on_error(
    envoy_error: envoy_mobile_sys::envoy_error,
    context: *mut c_void,
) -> *mut c_void {
    let context = context as *mut StreamContext;
    let on_error = &(*context).on_error;
    on_error.put_and_report(HTTPError::from_envoy_error(envoy_error));
    ptr::null_mut::<c_void>()
}

unsafe extern "C" fn dispatch_on_complete(context: *mut c_void) -> *mut c_void {
    let context = context as *mut StreamContext;
    let on_complete = &(*context).on_complete;
    on_complete.put_and_report(Ok(()));
    ptr::null_mut::<c_void>()
}

unsafe extern "C" fn dispatch_on_cancel(context: *mut c_void) -> *mut c_void {
    let context = context as *mut StreamContext;
    let on_cancel = &(*context).on_cancel;
    on_cancel.put_and_report(Ok(()));
    ptr::null_mut::<c_void>()
}

// impl<T> Drop for Stream<T> {
//     fn drop(&mut self) {
//         if let Ok(request_sent) = self.request_sent.lock() {
//             if !*request_sent {
//                 // SAFETY: We only deallocate the context ptr when there as not been a request
//                 // sent. Therefore the context will not be used from another thread.
//                 //
//                 // If a request has been sent, then the context will be deallocated from inside one
//                 // of the terminal callbacks: on_error, on_complete, or on_cancel.
//                 unsafe {
//                     let _ = Box::from_raw(self.context_ptr);
//                 }
//             }
//         }

//         // SAFETY: this is trivially safe, as:
//         //   - if handle is invalid, nothing happens
//         //   - if the handle is already reset, nothing happens
//         //   - if the handle is valid and active, it gets reset
//         unsafe {
//             envoy_mobile_sys::reset_stream(self.handle);
//         }
//     }
// }
