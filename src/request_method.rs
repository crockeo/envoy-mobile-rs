#[derive(Debug)]
pub enum RequestMethod {
    // copied from Mozilla HTTP request method list:
    // https://developer.mozilla.org/en-US/docs/Web/HTTP/Methods
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
    PATCH,
}

impl ToString for RequestMethod {
    fn to_string(&self) -> String {
        format!("{:?}", self)
    }
}
