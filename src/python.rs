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
    let method = bridge::Method::try_from(method.as_ref())
        .map_err(|_| PyValueError::new_err(format!("invalid method '{}'", method.as_ref())))?;

    let mut request = crate::Request::new(method, url).map_err(|e| PyException::new_err(e))?;
    if let Some(headers) = headers {
        request = request.with_headers(headers);
    }
    if let Some(data) = data {
        request = request.with_data(data);
    }

    let response = request.send().await.map_err(|e| PyException::new_err(e))?;
    Ok(Response {
        status: response.status,
        headers: response.headers,
        data: response.data,
    })
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
