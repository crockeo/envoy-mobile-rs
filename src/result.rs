//! Provides the [EnvoyError] and [EnvoyResult] types to normalize the heterogeneous errors that can
//! occur in this package into a homogeneous error/result type.

use std::result;
use std::str::Utf8Error;

use crate::bridge_util::HTTPError;

/// Wrapper around all the different kinds of errors that can occur while using this package.
#[derive(Debug)]
pub enum EnvoyError {
    /// Denotes an occurrence when a handle created by envoy-mobile was invalid, whatever "invalid"
    /// means for that particular handle type.
    InvalidHandle,

    /// Denotes an occurrence when envoy-mobile returns a failure code for initializing a
    /// [crate::engine::Engine] or a [crate::stream::Stream].
    CouldNotInit,

    /// Returned when envoy-mobile fails to send some data with a function like
    /// [crate::stream::Stream::send_headers].
    FailedToSend(&'static str),

    /// Used when some [Data](crate::bridge_util::Data], which is assumed to be a [String], was not
    /// decoded successfully.
    DecodeFailure,

    /// Denotes that an operation was attempted on a [crate::callback_futures::CallbackFuture] or
    /// [crate::callback_futures::CallbackStream] that was already closed.
    AlreadyClosed,

    /// Denotes an [crate::bridge_util::HTTPError] occurred while using a [crate::stream::Stream].
    HTTPError(HTTPError),
}

impl From<Utf8Error> for EnvoyError {
    fn from(_: Utf8Error) -> EnvoyError {
        EnvoyError::DecodeFailure
    }
}

/// Type alias to make an [EnvoyError] result easier to use.
pub type EnvoyResult<T> = result::Result<T, EnvoyError>;
