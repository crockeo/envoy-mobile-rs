use std::result;

#[derive(Debug)]
pub enum EnvoyError {
    InvalidHandle,
    CouldNotInit,
    FailedToSend(&'static str),
}

pub type EnvoyResult<T> = result::Result<T, EnvoyError>;
