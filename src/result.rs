use std::result;

#[derive(Debug)]
pub enum Error {
    InvalidHandle,
    CouldNotInit,
}

pub type Result<T> = result::Result<T, Error>;
