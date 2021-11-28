use std::ffi;
use std::ptr;
use std::str::Utf8Error;
use std::string::FromUtf8Error;

use crate::sys;

#[derive(Eq, Clone, Debug, PartialEq)]
pub struct Data(Vec<u8>);

impl Data {
    pub unsafe fn from_envoy_data_no_release(envoy_data: &sys::envoy_data) -> Self {
        let length = envoy_data.length.try_into().unwrap();
        let mut bytes = vec![0; length];
        ptr::copy(envoy_data.bytes, bytes.as_mut_ptr(), length);
        Self(bytes)
    }

    pub unsafe fn from_envoy_data(envoy_data: sys::envoy_data) -> Self {
        let data = Self::from_envoy_data_no_release(&envoy_data);
        sys::release_envoy_data(envoy_data);
        data
    }

    pub fn into_envoy_data(self) -> sys::envoy_data {
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

impl From<String> for Data {
    fn from(string: String) -> Self {
        Self(string.into_bytes())
    }
}

impl From<&str> for Data {
    fn from(string: &str) -> Self {
        Self::from(string.to_owned())
    }
}

impl From<Vec<u8>> for Data {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl From<&[u8]> for Data {
    fn from(bytes: &[u8]) -> Self {
        Self::from(bytes.to_owned())
    }
}

impl TryFrom<Data> for String {
    type Error = FromUtf8Error;

    fn try_from(data: Data) -> Result<String, Self::Error> {
        String::from_utf8(data.into())
    }
}

impl<'a> TryFrom<&'a Data> for &'a str {
    type Error = Utf8Error;

    fn try_from(data: &'a Data) -> Result<&'a str, Self::Error> {
        std::str::from_utf8(data.into())
    }
}

impl From<Data> for Vec<u8> {
    fn from(data: Data) -> Self {
        data.0
    }
}

impl<'a> From<&'a Data> for &'a [u8] {
    fn from(data: &'a Data) -> Self {
        &data.0
    }
}
