use std::result;

#[derive(Debug)]
pub enum EnvoyError {
    InvalidHandle,
    CouldNotInit,
    FailedToSend(&'static str),

    /// Denotes that an operation was attempted on a future, stream, or
    AlreadyClosed,
}

pub type EnvoyResult<T> = result::Result<T, EnvoyError>;
