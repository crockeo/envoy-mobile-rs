pub enum Scheme {
    HTTP,
    HTTPS,
}

impl ToString for Scheme {
    fn to_string(&self) -> String {
        use Scheme::*;
        match self {
            HTTP => "http",
            HTTPS => "https",
        }
        .to_string()
    }
}
