pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl ToString for LogLevel {
    fn to_string(&self) -> String {
        use LogLevel::*;
        match self {
            Debug => "debug",
            Info => "info",
            Warning => "warn",
            Error => "error",
        }
        .to_string()
    }
}
