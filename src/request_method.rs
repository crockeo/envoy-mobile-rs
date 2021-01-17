pub enum RequestMethod {
    // TODO: when i land, look up reference and populate everything
    GET,
    POST,
    HEAD,
    PUT,
    DELETE,
}

impl ToString for RequestMethod {
    fn to_string(&self) -> String {
        use RequestMethod::*;
        match self {
            GET => "GET",
            POST => "POST",
            HEAD => "HEAD",
            PUT => "PUT",
            DELETE => "DELETE",
        }
        .to_string()
    }
}
