use std::collections::HashMap;

use futures::executor;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyDict;

#[pyclass]
struct Engine(Option<crate::Engine>);

#[pymethods]
impl Engine {
    #[new(kwargs = "**")]
    fn new(log_level: crate::LogLevel, kwargs: Option<&PyDict>) -> PyResult<Self> {
        let mut builder = crate::EngineBuilder::default();
        if let Some(kwargs) = kwargs {
            if let Some(connect_timeout_seconds) = kwargs.get_item("connect_timeout_seconds") {
                builder = builder
                    .with_connect_timeout_seconds(connect_timeout_seconds.extract::<usize>()?);
            }
            if let Some(dns_refresh_rate_seconds) = kwargs.get_item("dns_refresh_rate_seconds") {
                builder = builder
                    .with_dns_refresh_rate_seconds(dns_refresh_rate_seconds.extract::<usize>()?);
            }

            if let Some(dns_fail_interval_seconds) =
                kwargs.get_item("dns_fail_base_interval_seconds")
            {
                let (dns_fail_base_interval_seconds, dns_fail_max_interval_seconds) =
                    dns_fail_interval_seconds.extract::<(usize, usize)>()?;

                builder = builder.with_dns_fail_interval(
                    dns_fail_base_interval_seconds,
                    dns_fail_max_interval_seconds,
                );
            }

            if let Some(dns_query_timeout_seconds) = kwargs.get_item("dns_query_timeout_seconds") {
                builder = builder
                    .with_dns_query_timeout_seconds(dns_query_timeout_seconds.extract::<usize>()?);
            }

            if let Some(enable_interface_binding) = kwargs.get_item("enable_interface_binding") {
                builder = builder
                    .with_enable_interface_binding(enable_interface_binding.extract::<bool>()?);
            }

            if let Some(h2_connection_keepalive_idle_interval_seconds) =
                kwargs.get_item("h2_connection_keepalive_idle_interval_seconds")
            {
                builder = builder.with_h2_connection_keepalive_idle_interval_seconds(
                    h2_connection_keepalive_idle_interval_seconds.extract::<usize>()?,
                );
            }

            if let Some(h2_connection_keepalive_timeout) =
                kwargs.get_item("h2_connection_keepalive_timeout")
            {
                builder = builder.with_h2_connection_keepalive_timeout(
                    h2_connection_keepalive_timeout.extract::<usize>()?,
                );
            }

            // TODO: manually convert IpAddr
            // if let Some(stats_domain) = kwargs.get_item("stats_domain") {
            //     builder = builder
            //         .with_stats_domain(stats_domain.extract::<IpAddr>()?);
            // }

            if let Some(stats_flush_interval_seconds) =
                kwargs.get_item("stats_flush_interval_seconds")
            {
                builder = builder.with_stats_flush_interval_seconds(
                    stats_flush_interval_seconds.extract::<usize>()?,
                );
            }

            // TODO: manually convert IpAddr
            // if let Some(statsd_host) = kwargs.get_item("statsd_host") {
            //     builder = builder
            //         .with_statsd_host(statsd_host.extract::<IpAddr>()?);
            // }

            if let Some(statsd_port) = kwargs.get_item("statsd_port") {
                builder = builder.with_statsd_port(statsd_port.extract::<u16>()?);
            }

            if let Some(stream_idle_timeout_seconds) =
                kwargs.get_item("stream_idle_timeout_seconds")
            {
                builder = builder.with_stream_idle_timeout_seconds(
                    stream_idle_timeout_seconds.extract::<usize>()?,
                );
            }

            if let Some(per_try_idle_timeout_seconds) =
                kwargs.get_item("per_try_idle_timeout_seconds")
            {
                builder = builder.with_per_try_idle_timeout_seconds(
                    per_try_idle_timeout_seconds.extract::<usize>()?,
                );
            }
        }

        let engine = executor::block_on(builder.build(log_level));
        Ok(Self(Some(engine)))
    }

    pub fn new_stream(&self, explicit_flow_control: bool) -> Stream {
	todo!()
    }

    pub fn record_counter_inc(
        &self,
        elements: &str,
        tags: HashMap<String, String>,
        count: usize,
    ) -> PyResult<()> {
        self.apply("record_counter_inc", move |engine| {
            engine.record_counter_inc(elements, crate::StatsTags::from(tags), count)
        })
    }

    pub fn record_gauge_set(
        &self,
        elements: &str,
        tags: HashMap<String, String>,
        value: usize,
    ) -> PyResult<()> {
        self.apply("record_gauge_set", move |engine| {
            engine.record_gauge_set(elements, crate::StatsTags::from(tags), value)
        })
    }

    pub fn record_gauge_add(
        &self,
        elements: &str,
        tags: HashMap<String, String>,
        amount: usize,
    ) -> PyResult<()> {
        self.apply("record_gauge_add", move |engine| {
            engine.record_gauge_add(elements, crate::StatsTags::from(tags), amount)
        })
    }

    pub fn record_gauge_sub(
        &self,
        elements: &str,
        tags: HashMap<String, String>,
        amount: usize,
    ) -> PyResult<()> {
        self.apply("record_gauge_sub", move |engine| {
            engine.record_gauge_sub(elements, crate::StatsTags::from(tags), amount)
        })
    }

    pub fn record_histogram_value(
        &self,
        elements: &str,
        tags: HashMap<String, String>,
        amount: usize,
        unit_measure: crate::HistogramStatUnit,
    ) -> PyResult<()> {
        self.apply("record_histogram_value", move |engine| {
            engine.record_histogram_value(
                elements,
                crate::StatsTags::from(tags),
                amount,
                unit_measure,
            )
        })
    }

    pub fn flush_stats(&self) -> PyResult<()> {
        self.apply("flush_stats", move |engine| engine.flush_stats())
    }

    pub fn dump_stats(&self) -> PyResult<Vec<u8>> {
        self.apply("dump_stats", move |engine| {
            let data = engine.dump_stats();
            data.into()
        })
    }

    fn terminate(&mut self) -> PyResult<()> {
        if let Some(engine) = self.0.take() {
            Ok(executor::block_on(engine.terminate()))
        } else {
            Err(PyException::new_err(
                "cannot terminate an engine after it has already been terminated",
            ))
        }
    }
}

impl Engine {
    fn apply<T>(&self, name: &str, func: impl FnOnce(&crate::Engine) -> T) -> PyResult<T> {
        if let Some(engine) = &self.0 {
            Ok(func(engine))
        } else {
            Err(PyException::new_err(format!(
                "cannot {} after engine has been terminated",
                name
            )))
        }
    }
}

#[pyclass]
struct Stream();

#[pymethods]
impl Stream {
}

#[pymodule]
fn envoy_mobile(_: Python, module: &PyModule) -> PyResult<()> {
    module.add_class::<crate::LogLevel>()?;
    module.add_class::<Engine>()?;
    Ok(())
}
