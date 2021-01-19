//! Provides structured wrappers around the low-level [Headers] type. This module allows users to
//! avoid populating & parsing reserved header keys.

use crate::bridge_util::Headers;
use crate::request_method::RequestMethod;
use crate::scheme::Scheme;

pub enum HeadersError {
    ReservedKey,
}

pub struct RequestHeaders(Headers);

impl RequestHeaders {
    pub fn new() -> Self {
        Self(Headers::new())
    }

    pub fn add<T: AsRef<str>, U: AsRef<str>>(self, key: T, value: U) -> Result<Self, HeadersError> {
        let key = key.as_ref();
        let value = value.as_ref();

        if is_key_reserved(key) {
            return Err(HeadersError::ReservedKey);
        }

        Ok(RequestHeaders(self.0.add(key, value)))
    }

    pub fn add_method(self, method: RequestMethod) -> Self {
        RequestHeaders(self.0.add(":method", method.to_string()))
    }

    pub fn add_scheme(self, scheme: Scheme) -> Self {
        RequestHeaders(self.0.add(":scheme", scheme.to_string()))
    }

    pub fn add_authority<T: AsRef<str>>(self, authority: T) -> Self {
        RequestHeaders(self.0.add(":authority", authority))
    }

    pub fn add_path<T: AsRef<str>>(self, path: T) -> Self {
        RequestHeaders(self.0.add(":path", path))
    }

    pub fn as_headers(self) -> Headers {
        self.0
    }
}

fn is_key_reserved<T: AsRef<str>>(key: T) -> bool {
    // TODO: download and use lazy_static!
    // TODO: populate more reserved keywords
    let key = key.as_ref();
    let reserved_keys = vec![":method", ":scheme", ":authority", ":path"];
    for reserved_key in reserved_keys.into_iter() {
        if key == reserved_key {
            return true;
        }
    }

    false
}
