mod bridge;
mod channel;
mod event;
#[cfg(python)]
mod python;
mod sys;

pub use bridge::*;

#[cfg(python)]
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

    async fn make_request(mut stream: Stream) {
        stream.send_headers(
            Headers::new_request_headers(
                bridge::Method::Get,
                bridge::Scheme::Http,
                "localhost:8080",
                "/",
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

        while let Some(_) = stream.trailers().poll().await {}
        while let Some(_) = stream.metadata().poll().await {}

        let completion = stream.completion().poll().await;
        assert_eq!(completion, Some(Completion::Complete));
    }

    #[test]
    async fn stream_lifecycle() {
        let engine = EngineBuilder::default()
            .with_log(|data| {
                print!("{}", String::try_from(data).unwrap());
            })
            .build(LogLevel::Error)
            .await;

        make_request(engine.new_stream(false)).await;

        engine.terminate().await;
    }

    #[test]
    async fn loop_trigger_memory_leak() {
        let engine = EngineBuilder::default()
            .with_log(|data| {
                print!("{}", String::try_from(data).unwrap());
            })
            .build(LogLevel::Error)
            .await;

        let mut requests = Vec::with_capacity(100);
        for _ in 0..100 {
            let stream = engine.new_stream(false);
            requests.push(tokio::spawn(make_request(stream)));
        }
        for request in requests.into_iter() {
            request.await.unwrap();
        }

        engine.terminate().await;
    }
}
