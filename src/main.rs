#![allow(dead_code)]

mod bridge_util;
mod callback_futures;
mod engine;
mod headers;
mod log_level;
mod request_method;
mod result;
mod scheme;
mod stream;

use futures::{select, StreamExt};
use tokio;

use engine::EngineBuilder;
use headers::RequestHeaders;
use log_level::LogLevel;
use request_method::RequestMethod;
use result::EnvoyResult;
use scheme::Scheme;

#[tokio::main]
async fn main() -> EnvoyResult<()> {
    let engine = EngineBuilder::new(LogLevel::Error).build()?;
    engine.on_engine_running().await;

    let mut stream = engine.stream()?;

    stream.send_headers(
        RequestHeaders::new()
            .add_method(RequestMethod::GET)
            .add_scheme(Scheme::HTTPS)
            .add_authority("www.google.com")
            .add_path("/")
            .as_headers(),
        true,
    )?;

    while let Some(headers) = stream.on_headers().next().await {
        for (key, value) in headers?.into_iter() {
            println!("{}: {}", key, value);
        }
    }

    while let Some(data) = stream.on_data().next().await {
        if let Ok(s) = data?.as_str() {
            println!("{}", s);
        }
    }

    select! {
        _ = stream.on_error() => {
            println!("error");
        },
        _ = stream.on_complete() => {
            println!("complete");
        },
        _ = stream.on_cancel() => {
            println!("cancel");
        },
    }

    engine.terminate();

    Ok(())
}
