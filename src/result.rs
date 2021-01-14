use std::result;

#[derive(Debug)]
pub enum Error {
    InvalidHandle,
    CouldNotInit,
    FailedToSend(&'static str),
}

pub type Result<T> = result::Result<T, Error>;
