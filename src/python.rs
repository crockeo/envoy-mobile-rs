use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Once;
use std::task::{Context, Poll};

use futures::executor;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyFunction;
use tokio::runtime::Runtime as TokioRuntime;
use url::Url;

static mut ENGINE_INSTANCE: Option<crate::Engine> = None;
static ENGINE_INSTANCE_INIT: Once = Once::new();

static mut TOKIO_RUNTIME: Option<TokioRuntime> = None;
static TOKIO_RUNTIME_INIT: Once = Once::new();

fn engine_instance() -> &'static crate::Engine {
    unsafe {
        // TODO: provide an interface to configure the engine instance
        ENGINE_INSTANCE_INIT.call_once(|| {
            ENGINE_INSTANCE = Some(executor::block_on(
                crate::EngineBuilder::default()
                    .with_log(|data| {
                        print!("{}", String::try_from(data).unwrap());
                    })
                    .build(crate::LogLevel::Error),
            ))
        });

        ENGINE_INSTANCE.as_ref().unwrap()
    }
}

fn tokio_runtime() -> &'static TokioRuntime {
    unsafe {
	TOKIO_RUNTIME_INIT.call_once(|| {
	    TOKIO_RUNTIME = Some(TokioRuntime::new().unwrap());
	});

	TOKIO_RUNTIME.as_ref().unwrap()
    }
}

#[pyclass]
#[derive(Clone)]
pub struct Response {
    #[pyo3(get)]
    status_code: u16,
    #[pyo3(get)]
    headers: HashMap<String, String>,
    #[pyo3(get)]
    body: Vec<u8>,
}

impl Default for Response {
    fn default() -> Self {
        Self {
            status_code: 0,
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }
}

create_exception!(envoy_mobile, StreamCancelled, PyException);
create_exception!(envoy_mobile, StreamErrored, PyException);

#[pyclass]
struct StreamErroredInformation {
    #[pyo3(get)]
    partial_response: Response,
    #[pyo3(get)]
    envoy_error: crate::Error,
}

#[pyfunction]
pub fn request(
    method: &str,
    url: &str,
    data: Option<Vec<u8>>,
    headers: Option<HashMap<String, String>>,
) -> PyResult<Response> {
    let engine = engine_instance();
    executor::block_on(request_impl(engine, method, url, data, headers))
}

#[pyfunction]
pub fn async_request(
    method: String,
    url: String,
    data: Option<Vec<u8>>,
    headers: Option<HashMap<String, String>>,
    callback: &PyFunction,
) -> PyResult<()> {
    let engine = engine_instance();
    let callback: PyObject = callback.into();

    let request_promise = request_impl(&engine, method, url, data, headers);
    let py_future = PyFuture {
	wake_func: callback,
	inner: RefCell::new(request_promise),
    };
    tokio_runtime().spawn(py_future);

    Ok(())
}

async fn request_impl(
    engine: &crate::Engine,
    method: impl AsRef<str>,
    url: impl AsRef<str>,
    data: Option<Vec<u8>>,
    headers: Option<HashMap<String, String>>,
) -> PyResult<Response> {
    let mut stream = engine.new_stream(false);

    let method = normalize_method(method.as_ref())?;
    let (scheme, authority, path) = normalize_url(url.as_ref())?;
    let (data, mut additional_headers) = normalize_data(data)?;

    let extra_headers = vec![
        (":method", method.into_envoy_method()),
        (":scheme", scheme.into_envoy_scheme()),
        (":authority", &authority),
        (":path", &path),
    ];
    for (key, value) in extra_headers.into_iter() {
        additional_headers.insert(key.to_string(), value.to_string());
    }

    let headers = normalize_headers(headers, additional_headers)?;

    // TODO: allow chunked sending things over the wire
    // and/or sending trailers and metadata
    stream.send_headers(headers, data.len() == 0);
    if data.len() > 0 {
        stream.send_data(data, true);
    }

    let mut response = Response::default();
    while let Some(headers_block) = stream.headers().poll().await {
        // TODO: check for error here
        let mut headers_block = HashMap::<String, String>::try_from(headers_block).unwrap();
        if let Some(status) = headers_block.remove(":status") {
            // TODO: check for error here
            response.status_code = str::parse::<u16>(&status).unwrap();
        }
        response.headers.extend(headers_block.into_iter());
    }

    while let Some(body_block) = stream.data().poll().await {
        response.body.extend(Vec::<u8>::from(body_block));
    }

    // TODO: populate headers with metadata and trailers?

    // TODO: check for error here

    match stream.completion().poll().await.unwrap() {
        crate::Completion::Cancel => Err(StreamCancelled::new_err("stream cancelled")),

        crate::Completion::Complete => Ok(response),

        crate::Completion::Error(envoy_error) => {
            Err(StreamErrored::new_err(StreamErroredInformation {
                partial_response: response,
                envoy_error,
            }))
        }
    }
}

fn normalize_method(method: &str) -> PyResult<crate::Method> {
    crate::Method::try_from(method)
        .map_err(|_| PyValueError::new_err(format!("invalid method '{}'", method)))
}

fn normalize_url(url: &str) -> PyResult<(crate::Scheme, String, String)> {
    let url =
        Url::parse(url).map_err(|_| PyValueError::new_err(format!("invalid URL '{}'", url)))?;

    let scheme = crate::Scheme::try_from(url.scheme())
        .map_err(|_| PyValueError::new_err(format!("invalid scheme '{}'", url.scheme())))?;

    let authority = url
        .host_str()
        .ok_or_else(|| PyValueError::new_err(format!("URL must contain host '{}'", url)))?;
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

fn normalize_data(data: Option<Vec<u8>>) -> PyResult<(Vec<u8>, HashMap<String, String>)> {
    let mut is_utf8 = false;
    let data = match data {
        None => Vec::<u8>::new(),
        Some(data) => {
            is_utf8 = std::str::from_utf8(&data).is_ok();
            data
        }
    };

    // TODO: do some kind of content sniffing (e.g. accept &PyAny?)
    // in order to populate content-type
    let mut headers = HashMap::<String, String>::new();
    if is_utf8 {
        headers.insert("charset".to_string(), "utf8".to_string());
    }
    if data.len() > 0 {
        headers.insert("content-length".to_string(), format!("{}", data.len()));
    }

    Ok((data, headers))
}

fn normalize_headers(
    headers: Option<HashMap<String, String>>,
    additional_headers: HashMap<String, String>,
) -> PyResult<HashMap<String, String>> {
    // NOTE: yes this is obviously inefficient
    // but it's functionality is obvious
    let mut final_headers = HashMap::new();
    for (key, value) in additional_headers {
        final_headers.insert(key, value);
    }
    if let Some(headers) = headers {
        for (key, value) in headers {
            final_headers.insert(key, value);
        }
    }
    Ok(final_headers)
}

struct PyFuture<T> {
    wake_func: PyObject,
    inner: RefCell<T>,
}

impl<T, U> Future for PyFuture<T>
where
    T: Future<Output = PyResult<U>> + Sized,
    U: IntoPy<PyObject>,
{
    // TODO: think about how to surface async errors better
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut borrow = self.inner.borrow_mut();
        let inner: &mut T = &mut *borrow;

        let pin;
        unsafe {
            pin = Pin::new_unchecked(inner);
        }
        let result = pin.poll(ctx);
        match result {
            Poll::Pending => Poll::Pending,

            Poll::Ready(Ok(result)) => {
                let _ = Python::with_gil(|py| self.wake_func.call(py, (result,), None));
                Poll::Ready(())
            }

            Poll::Ready(Err(e)) => {
                let _ = Python::with_gil(|py| self.wake_func.call(py, (e,), None));
                Poll::Ready(())
            }
        }
    }
}

#[pymodule]
fn envoy_mobile(py: Python, module: &PyModule) -> PyResult<()> {
    module.add_class::<Response>()?;
    module.add_class::<StreamErroredInformation>()?;
    module.add_function(wrap_pyfunction!(request, module)?)?;
    module.add_function(wrap_pyfunction!(async_request, module)?)?;

    module.add("StreamCancelled", py.get_type::<StreamCancelled>())?;
    module.add("StreamErrored", py.get_type::<StreamErrored>())?;

    Ok(())
}
