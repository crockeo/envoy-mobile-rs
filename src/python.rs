use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Once;
use std::task::{Context, Poll};

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyFunction;
use tokio::runtime::Runtime as TokioRuntime;
use url::Url;

use crate::bridge;

static mut TOKIO_RUNTIME: Option<TokioRuntime> = None;
static TOKIO_RUNTIME_INIT: Once = Once::new();

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
    status: usize,
    #[pyo3(get)]
    headers: HashMap<String, String>,
    #[pyo3(get)]
    data: Vec<u8>,
}

impl Default for Response {
    fn default() -> Self {
        Self {
            status: 0,
            headers: HashMap::new(),
            data: Vec::new(),
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
    error: Error,
}

#[pyclass(name = "Error")]
#[derive(Clone)]
struct Error {
    error_name: String,
    message: String,
    attempt_count: i32,
}

impl From<bridge::Error> for Error {
    fn from(error: bridge::Error) -> Self {
        let error_name: &str = error.error_code.into();
        Self {
            error_name: error_name.to_owned(),
            message: error.message,
            attempt_count: error.attempt_count,
        }
    }
}

#[pyfunction]
pub fn request(
    method: &str,
    url: &str,
    data: Option<Vec<u8>>,
    headers: Option<HashMap<String, String>>,
) -> PyResult<Response> {
    let tokio_runtime = tokio_runtime();
    tokio_runtime.block_on(request_impl(method, url, data, headers))
}

#[pyfunction]
pub fn async_request(
    method: String,
    url: String,
    data: Option<Vec<u8>>,
    headers: Option<HashMap<String, String>>,
    callback: &PyFunction,
) -> PyResult<()> {
    let callback: PyObject = callback.into();

    let request_promise = request_impl(method, url, data, headers);
    let py_future = PyFuture {
        wake_func: callback,
        inner: RefCell::new(request_promise),
    };
    tokio_runtime().spawn(py_future);

    Ok(())
}

async fn request_impl(
    method: impl AsRef<str>,
    url: impl AsRef<str>,
    data: Option<Vec<u8>>,
    headers: Option<HashMap<String, String>>,
) -> PyResult<Response> {
    let method = normalize_method(method.as_ref())?;
    let (data, mut additional_headers) = normalize_data(data)?;
    let headers = normalize_headers(headers, additional_headers)?;

    let response = crate::Request::new(method, url)
        .map_err(|e| PyException::new_err(e))?
        .with_headers(headers)
        .with_data(data)
        .send()
        .await
        .map_err(|e| PyException::new_err(e))?;

    Ok(Response {
        status: response.status,
        headers: response.headers,
        data: response.data,
    })
}

fn normalize_method(method: &str) -> PyResult<bridge::Method> {
    bridge::Method::try_from(method)
        .map_err(|_| PyValueError::new_err(format!("invalid method '{}'", method)))
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

    Ok((data, headers))
}

fn normalize_headers(
    headers: Option<HashMap<String, String>>,
    additional_headers: HashMap<String, String>,
) -> PyResult<HashMap<String, String>> {
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
