use libc;

use std::collections::HashMap;
use std::ffi::c_void;
use std::mem;
use std::ptr;
use std::slice;
use std::str::{self, Utf8Error};

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

    pub fn from_envoy_headers(envoy_headers: envoy_mobile_sys::envoy_headers) -> Self {
        let envoy_headers_slice;
        unsafe {
            envoy_headers_slice =
                slice::from_raw_parts(envoy_headers.headers, envoy_headers.length as usize);
        }

        let mut headers = Headers(HashMap::new());
        for envoy_header in envoy_headers_slice {
            // TODO: take the time to bubble this error up nicely; i don't know how much we can
            // trust envoy-mobile to do UTF8 validation
            headers = headers
                .add_data(
                    Data::from_envoy_data(envoy_header.key),
                    Data::from_envoy_data(envoy_header.value),
                )
                .unwrap();
        }

        unsafe {
            // normally one might use envoy_mobile_sys::release_envoy_headers, but we already
            // release each of the envoy_data entries while iterating through, so we want to avoid
            // a double free
            libc::free(envoy_headers.headers as *mut c_void);
        }

        headers
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

pub struct HTTPError {}

impl HTTPError {
    pub fn from_envoy_error(envoy_error: envoy_mobile_sys::envoy_error) -> Self {
        todo!()
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
