mod bridge;
mod channel;
mod event;
mod python;
mod sys;

pub use bridge::*;
pub use python::*;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tokio::test;

    use super::*;

    #[test]
    async fn engine_lifecycle() {
        let engine = EngineBuilder::default()
            .with_log(|data| {
                print!("{}", String::try_from(data).unwrap());
            })
            .build(LogLevel::Debug)
            .await;
        engine.terminate().await;
    }

    #[test]
    async fn stream_lifecycle() {
        let engine = EngineBuilder::default()
            .with_log(|data| {
                print!("{}", String::try_from(data).unwrap());
            })
            .build(LogLevel::Error)
            .await;

        let mut stream = engine.new_stream(false);
        stream.send_headers(
            Headers::new_request_headers(
                bridge::Method::Get,
                bridge::Scheme::Https,
                "api.lyft.com",
                "/ping",
            ),
            true,
        );

        while let Some(headers) = stream.headers().poll().await {
            let headers = HashMap::<String, String>::try_from(headers).unwrap();
            if let Some(status) = headers.get(":status") {
                assert_eq!(status, "200");
            }
            for (key, value) in headers.into_iter() {
                println!("{}: {}", key, value);
            }
        }

        while let Some(data) = stream.data().poll().await {
            let data_str: &str = (&data).try_into().unwrap();
            println!("{}", data_str);
        }

        let completion = stream.completion().poll().await;
        assert_eq!(completion, Some(Completion::Complete));

        engine.terminate().await;
    }
}
