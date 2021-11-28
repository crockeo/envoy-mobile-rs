/*!
Provides both a high-level and a low-level interface to envoy-mobile.
The high-level interface lives on the top-level of the library:

```rust
let response = Response::new(Method::GET, "http://localhost:8080")?
    .send()
    .await?;

println!("{} {:?} {:?}", response.status, response.headers. response.data);
```

And the low-level interface lives under [bridge]:

```rust
let engine = EngineBuilder::default()
    .build(LogLevel::Error)
    .await;

let mut stream = engine.new_stream(false);
stream.send_headers(
    Headers::new_request_headers(
        Method::Get,
        Scheme::Http,
        "localhost:8080",
        "/",
    ),
    true,
);

while let Some(headers) = stream.headers().poll().await { /* ... */ }
while let Some(data) = stream.data().poll().await { /* ... */ }
while let Some(_) = stream.trailers().poll().await { /* ... */ }
while let Some(_) = stream.metadata().poll().await { /* ... */ }
stream.completion().poll().await;

engine.terminate().await;
```
*/

pub mod bridge;
mod channel;
mod event;
mod python;
mod sys;

use std::collections::HashMap;
use std::sync::{Arc, Once};

use futures::executor;
use url::Url;

pub use bridge::Method;

pub type Error = &'static str;
pub type EnvoyResult<T> = Result<T, Error>;

pub struct Request {
    headers: HashMap<String, String>,
    data: Vec<bridge::Data>,
    content_length: usize,
}

impl Request {
    pub fn new(method: Method, url: impl AsRef<str>) -> EnvoyResult<Self> {
        let (scheme, authority, path) = parse_url(url.as_ref())?;

	let mut headers = HashMap::<String, String>::new();
	for (key, value) in vec![
	    (":method", method.into_envoy_method()),
	    (":scheme", scheme.into_envoy_scheme()),
	    (":authority", authority.as_ref()),
	    (":path", path.as_ref()),
	].into_iter() {
	    headers.insert(key.to_string(), value.to_string());
	}

        Ok(Self {
	    headers,
            data: Vec::default(),
            content_length: 0,
        })
    }

    pub fn with_headers(mut self, headers: impl Into<bridge::Map>) -> Self {
        let headers = HashMap::<String, String>::try_from(headers.into()).expect("failed to parse provided headers");
        for (key, value) in headers.into_iter() {
            self.headers.insert(key, value);
        }
        self
    }

    pub fn with_data(mut self, data: impl Into<bridge::Data>) -> Self {
        let data = data.into();
        self.content_length += data.len();
        self.data.push(data);
        self
    }

    pub async fn send(mut self) -> EnvoyResult<Response> {
        let mut stream = engine_instance().new_stream(false);

        if self.content_length > 0 {
            self.headers.insert(
                "content-length".to_string(),
                self.content_length.to_string(),
            );
        }

        let data_len = self.data.len();
        let has_data = data_len > 0;
	stream.send_headers(self.headers, !has_data);
        for (i, data) in self.data.into_iter().enumerate() {
            let last_data = i == data_len - 1;
            stream.send_data(data, last_data);
        }

        let mut headers_agg = Vec::new();
        let mut data_agg = Vec::new();
        while let Some(headers) = stream.headers().poll().await {
            headers_agg.push(headers);
        }
        while let Some(data) = stream.data().poll().await {
            data_agg.push(data);
        }
        while let Some(metadata) = stream.metadata().poll().await {
            headers_agg.push(metadata);
        }
        while let Some(trailers) = stream.trailers().poll().await {
            headers_agg.push(trailers);
        }

        match stream
            .completion()
            .poll()
            .await
	    .ok_or_else(|| "stream must complete")?
        {
	    bridge::Completion::Cancel => return Err("stream cancelled"),
	    bridge::Completion::Error(_) => return Err("stream errored"),
	    bridge::Completion::Complete => {},
        }

        let mut flat_headers = HashMap::<String, String>::new();
        for headers in headers_agg.into_iter() {
            let headers = HashMap::<String, String>::try_from(headers)
                .map_err(|_| "failed to parse headers")?;
            for (key, value) in headers.into_iter() {
                flat_headers.insert(key, value);
            }
        }

        let status = flat_headers
            .remove(":status")
            .ok_or_else(|| "response headers must have :status")?
            .parse::<usize>()
            .map_err(|_| "response :status must be numeric")?;

        let data_agg_size = data_agg.iter().map(|data| data.len()).sum();
        let mut flat_data = Vec::with_capacity(data_agg_size);
        for data in data_agg.into_iter() {
            flat_data.append(&mut data.into());
        }

        Ok(Response {
            status,
            headers: flat_headers,
            data: flat_data,
        })
    }
}

fn parse_url(url: &str) -> EnvoyResult<(bridge::Scheme, String, String)> {
    let url = Url::parse(url).map_err(|_| "invalid URL")?;

    let scheme = bridge::Scheme::try_from(url.scheme()).map_err(|_| "invalid scheme")?;

    let authority = url.host_str().ok_or_else(|| "URL must contain authority")?;
    let authority = if let Some(port) = url.port() {
        format!("{}:{}", authority, port)
    } else {
        authority.to_string()
    };

    let path = if let Some(query) = url.query() {
        format!("{}?{}", url.path(), query)
    } else {
        url.path().to_string()
    };

    Ok((scheme, authority, path))
}

pub struct Response {
    pub status: usize,
    pub headers: HashMap<String, String>,
    pub data: Vec<u8>,
}

static mut ENGINE_INSTANCE: Option<Arc<bridge::Engine>> = None;
static ENGINE_INSTANCE_INIT: Once = Once::new();

fn engine_instance() -> Arc<bridge::Engine> {
    unsafe {
        // TODO: provide an interface to configure the engine instance
        ENGINE_INSTANCE_INIT.call_once(|| {
            ENGINE_INSTANCE = Some(executor::block_on(
                bridge::EngineBuilder::default()
                    .with_log(|data| {
                        print!("{}", String::try_from(data).unwrap());
                    })
                    .build(bridge::LogLevel::Error),
            ))
        });

        ENGINE_INSTANCE.clone().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tokio::test;

    #[test]
    async fn request_lifecycle() -> EnvoyResult<()> {
        let response = Request::new(Method::Get, "http://localhost:8080")?
            .send()
            .await?;

        assert_eq!(response.status, 200);
        Ok(())
    }
}
