// TODO: implement mapping from Data<->envoy_data and Headers<->envoy_headers
pub struct Headers(Vec<(String, Vec<String>)>);

impl Headers {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn add<T: AsRef<str>, U: AsRef<str>>(mut self, new_header: T, new_value: U) -> Self {
        let new_header = new_header.as_ref();
        let new_value = new_value.as_ref().to_string();

        for (header, values) in self.0.iter_mut() {
            if header == new_header {
                values.push(new_value.to_string());
                return self;
            }
        }

        self.0.push((new_header.to_string(), vec![new_value]));
        self
    }
}

pub struct Data(Vec<u8>);

pub struct HTTPError {}

pub fn envoy_headers_as_headers(envoy_headers: envoy_mobile_sys::envoy_headers) -> Headers {
    todo!()
}

pub fn headers_as_envoy_headers(headers: Headers) -> envoy_mobile_sys::envoy_headers {
    todo!()
}

pub fn envoy_data_as_data(envoy_data: envoy_mobile_sys::envoy_data) -> Data {
    todo!()
}

pub fn data_as_envoy_data(data: Data) -> envoy_mobile_sys::envoy_data {
    todo!()
}

pub fn envoy_error_as_error(envoy_error: envoy_mobile_sys::envoy_error) -> HTTPError {
    todo!()
}
