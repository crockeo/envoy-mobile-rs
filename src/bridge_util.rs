use libc;

use std::collections::HashMap;
use std::ffi::c_void;
use std::mem;
use std::ptr;
use std::slice;
use std::str::{self, Utf8Error};

use crate::result::EnvoyResult;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Headers(HashMap<String, Vec<String>>);

impl Headers {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn add<T: AsRef<str>, U: AsRef<str>>(mut self, key: T, value: U) -> Self {
        let key = key.as_ref().to_string();
        let value = value.as_ref().to_string();

        self.0.entry(key).or_insert_with(|| Vec::new()).push(value);
        self
    }

    pub fn add_data(self, key: Data, value: Data) -> Result<Self, Utf8Error> {
        Ok(self.add(key.as_str()?, value.as_str()?))
    }

    pub fn from_envoy_headers(envoy_headers: envoy_mobile_sys::envoy_headers) -> EnvoyResult<Self> {
        let envoy_headers_slice;
        unsafe {
            envoy_headers_slice =
                slice::from_raw_parts(envoy_headers.headers, envoy_headers.length as usize);
        }

        let mut headers = Headers(HashMap::new());
        for envoy_header in envoy_headers_slice {
            headers = headers.add_data(
                Data::from_envoy_data(envoy_header.key),
                Data::from_envoy_data(envoy_header.value),
            )?;
        }

        unsafe {
            // normally one might use envoy_mobile_sys::release_envoy_headers, but we already
            // release each of the envoy_data entries while iterating through, so we want to avoid
            // a double free
            libc::free(envoy_headers.headers as *mut c_void);
        }

        Ok(headers)
    }

    pub fn as_envoy_headers(self) -> envoy_mobile_sys::envoy_headers {
        let mut headers_len = 0;
        for (_, headers) in self.0.iter() {
            headers_len += headers.len();
        }

        let headers_ptr;
        unsafe {
            headers_ptr = envoy_mobile_sys::safe_malloc(
                (headers_len * mem::size_of::<envoy_mobile_sys::envoy_header>()) as u64,
            ) as *mut envoy_mobile_sys::envoy_header;
        }

        let headers: &mut [envoy_mobile_sys::envoy_header];
        unsafe {
            headers = std::slice::from_raw_parts_mut(headers_ptr, headers_len);
        }

        let mut i = 0;
        for (key, values) in self.0.into_iter() {
            for value in values.into_iter() {
                headers[i] = envoy_mobile_sys::envoy_header {
                    key: Data::from_bytes(key.clone()).as_envoy_data(),
                    value: Data::from_bytes(value).as_envoy_data(),
                };
                i += 1;
            }
        }

        envoy_mobile_sys::envoy_headers {
            length: headers_len as i32,
            headers: headers_ptr,
        }
    }
}

pub struct HeadersIter<'a> {
    map_iter: std::collections::hash_map::Iter<'a, String, Vec<String>>,
    key: &'a str,
    values: std::slice::Iter<'a, String>,
}

impl<'a> HeadersIter<'a> {
    fn new(map: &'a HashMap<String, Vec<String>>) -> Self {
        Self {
            map_iter: map.into_iter(),
            key: "",
            values: [].iter(),
        }
    }
}

impl<'a> Iterator for HeadersIter<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(value) = self.values.next() {
            Some((&self.key, &value))
        } else if let Some((key, values)) = self.map_iter.next() {
            self.key = key;
            self.values = values.into_iter();
            self.next()
        } else {
            None
        }
    }
}

impl<'a> IntoIterator for &'a Headers {
    type Item = (&'a str, &'a str);
    type IntoIter = HeadersIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        HeadersIter::new(&self.0)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Data(Vec<u8>);

impl Data {
    pub fn from_bytes<T: AsRef<[u8]>>(bytes: T) -> Self {
        let bytes = bytes.as_ref();
        let mut vec = vec![0; bytes.len()];
        vec.copy_from_slice(bytes);
        Data(vec)
    }

    pub fn from_envoy_data(envoy_data: envoy_mobile_sys::envoy_data) -> Self {
        let mut vec = vec![0; envoy_data.length as usize];
        unsafe {
            ptr::copy(
                envoy_data.bytes,
                vec.as_mut_ptr(),
                envoy_data.length as usize,
            );
            if let Some(release) = envoy_data.release {
                release(envoy_data.context);
            }
        }
        Self(vec)
    }

    pub fn as_envoy_data(self) -> envoy_mobile_sys::envoy_data {
        let bytes;
        unsafe {
            bytes = envoy_mobile_sys::safe_malloc(self.0.len() as u64) as *mut u8;
            ptr::copy(self.0.as_ptr(), bytes, self.0.len());
        }

        envoy_mobile_sys::envoy_data {
            length: self.0.len() as u64,
            bytes,
            release: Some(libc::free),
            context: bytes as *mut c_void,
        }
    }

    pub fn as_str(&self) -> Result<&str, Utf8Error> {
        str::from_utf8(self.0.as_slice())
    }
}

#[derive(Debug)]
pub enum HTTPErrorCode {
    Undefined,
    StreamReset,
    ConnectionFailure,
    BufferLimitExceeded,
    RequestTimeout,
}

impl HTTPErrorCode {
    fn from_envoy_error_code(
        error_code: envoy_mobile_sys::envoy_error_code_t,
    ) -> Option<HTTPErrorCode> {
        use HTTPErrorCode::*;
        match error_code {
            envoy_mobile_sys::envoy_error_code_t_ENVOY_UNDEFINED_ERROR => Some(Undefined),
            envoy_mobile_sys::envoy_error_code_t_ENVOY_STREAM_RESET => Some(StreamReset),
            envoy_mobile_sys::envoy_error_code_t_ENVOY_CONNECTION_FAILURE => {
                Some(ConnectionFailure)
            }
            envoy_mobile_sys::envoy_error_code_t_ENVOY_BUFFER_LIMIT_EXCEEDED => {
                Some(BufferLimitExceeded)
            }
            envoy_mobile_sys::envoy_error_code_t_ENVOY_REQUEST_TIMEOUT => Some(RequestTimeout),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct HTTPError {
    error_code: HTTPErrorCode,
    message: String,
    attempt_count: i32,
}

impl HTTPError {
    pub fn from_envoy_error(envoy_error: envoy_mobile_sys::envoy_error) -> EnvoyResult<Self> {
        Ok(HTTPError {
            error_code: HTTPErrorCode::from_envoy_error_code(envoy_error.error_code)
                .expect("invalid http error code received from envoy-mobile"),
            message: Data::from_envoy_data(envoy_error.message)
                .as_str()?
                .to_string(),
            attempt_count: envoy_error.attempt_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_from_envoy_headers_identity() {
        let original_headers = Headers::new()
            .add(":method", "GET")
            .add(":scheme", "https")
            .add(":authority", "www.google.com")
            .add(":path", "/");

        let envoy_headers = original_headers.clone().as_envoy_headers();
        let headers = Headers::from_envoy_headers(envoy_headers);

        assert_eq!(original_headers, headers,);
    }

    #[test]
    fn test_as_from_envoy_data_identity() {
        let original_data = Data::from_bytes("hello world");

        let envoy_data = original_data.clone().as_envoy_data();
        let data = Data::from_envoy_data(envoy_data);

        assert_eq!(original_data, data);
    }
}
