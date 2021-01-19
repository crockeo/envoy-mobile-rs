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
            .add_authority("www.delta.com")
            .add_path("/")
            .as_headers(),
        true,
    )?;

    // TODO: do this after implementing Iterator on both of these
    // for headers in stream.on_headers() {
    //     let headers = headers.await;
    //     for (key, value) in headers.into_iter() {
    //         println!("{}: {}", key, value);
    //     }
    // }

    // for data in stream.on_data() {
    //     let data = data.await;
    //     if let Ok(s) = data.as_str() {
    //         println!("{}", s);
    //     }
    // }

    // TODO: do this after implementing:
    //   - FutureExt
    //   - Iterator
    //   - futures::Future
    // select! {
    //     (_) = stream.on_error().fuse() => {},
    //     () = stream.on_complete().fuse() => {},
    //     () = stream.on_cancel().fuse() => {},
    // }

    engine.terminate();

    Ok(())
}
