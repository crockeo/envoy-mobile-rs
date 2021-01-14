use std::sync::Arc;

use super::engine::{Engine, EngineCallbacks};
use super::log_level::LogLevel;
use super::result::Result;

pub struct EngineBuilder<T> {
    context: Arc<T>,
    log_level: LogLevel,
    stats_domain: String,
    connect_timeout_seconds: u64,
    dns_refresh_seconds: u64,
    dns_failure_refresh_seconds_base: u64,
    dns_failure_refresh_seconds_max: u64,
    stats_flush_seconds: u64,
    app_version: String,
    app_id: String,
    virtual_clusters: String,
    engine_callbacks: EngineCallbacks<T>,
}

impl<T: Default> EngineBuilder<T> {
    pub fn new(context: Arc<T>, log_level: LogLevel) -> Self {
        Self {
            context,
            log_level,
            stats_domain: "0.0.0.0".to_string(),
            connect_timeout_seconds: 30,
            dns_refresh_seconds: 60,
            dns_failure_refresh_seconds_base: 2,
            dns_failure_refresh_seconds_max: 10,
            stats_flush_seconds: 60,
            app_version: "unspecified".to_string(),
            app_id: "unspecified".to_string(),
            virtual_clusters: "[]".to_string(),
            engine_callbacks: EngineCallbacks::new(),
        }
    }

    pub fn build(self) -> Result<Engine<T>> {
        Engine::new(
            self.build_config(),
            self.log_level,
            self.context,
            self.engine_callbacks,
        )
    }

    fn build_config(&self) -> String {
        // SAFETY: again, this is just trusting envoy-mobile. We have to assume that the c string
        // they provide under `config_template` is a valid point in memory & null-terminated.
        let config_template;
        unsafe {
            config_template = CStr::from_ptr(envoy_mobile_sys::config_template);
        }

        // TODO: make this faster; there's no reason for us to make a new string on every
        // replacement.
        let replacements = vec![
            ("{{ app_id }}", self.app_id.clone()),
            ("{{ app_version }}", self.app_version.clone()),
            (
                "{{ connect_timeout_seconds }}",
                self.connect_timeout_seconds.to_string(),
            ),
            ("{{ device_os }}", "rust".to_string()),
            (
                "{{ dns_failure_refresh_rate_seconds_base }}",
                self.dns_failure_refresh_seconds_base.to_string(),
            ),
            (
                "{{ dns_failure_refresh_rate_seconds_max }}",
                self.dns_failure_refresh_seconds_max.to_string(),
            ),
            (
                "{{ dns_refresh_rate_seconds }}",
                self.dns_refresh_seconds.to_string(),
            ),
            ("{{ native_filter_chain }}", "".to_string()),
            ("{{ platform_filter_chain }}", "".to_string()),
            ("{{ stats_domain }}", self.stats_domain.clone()),
            (
                "{{ stats_flush_interval_seconds }}",
                self.stats_flush_seconds.to_string(),
            ),
            ("{{ virtual_clusters }}", self.virtual_clusters.clone()),
        ];

        // this template doesn't change, and is written in ASCII under:
        // `envoy-mobile/library/common/config_template.cc`
        // so we never expect to have a Utf8Error
        let mut config_template = config_template.to_str().unwrap().to_string();
        for (search_str, replacement) in replacements.into_iter() {
            config_template = config_template.replace(search_str, replacement.as_str());
        }

        config_template
    }

    pub fn add_stats_domain(mut self, stats_domain: &str) -> Self {
        self.stats_domain = stats_domain.to_owned();
        self
    }

    pub fn add_dns_refresh_seconds(mut self, dns_refresh_seconds: u64) -> Self {
        self.dns_refresh_seconds = dns_refresh_seconds;
        self
    }

    pub fn add_dns_failure_refresh_seconds(mut self, base: u64, max: u64) -> Self {
        self.dns_failure_refresh_seconds_base = base;
        self.dns_failure_refresh_seconds_max = max;
        self
    }

    pub fn add_stats_flush_seconds(mut self, stats_flush_seconds: u64) -> Self {
        self.stats_flush_seconds = stats_flush_seconds;
        self
    }

    pub fn add_app_version(mut self, app_version: &str) -> Self {
        self.app_version = app_version.to_owned();
        self
    }

    pub fn add_app_id(mut self, app_id: &str) -> Self {
        self.app_id = app_id.to_owned();
        self
    }

    pub fn add_virtual_clusters(mut self, virutal_clusters: &str) -> Self {
        self.virtual_clusters = virutal_clusters.to_owned();
        self
    }

    pub fn add_on_engine_running<U: 'static + Fn(&Arc<T>)>(mut self, on_engine_running: U) -> Self {
        self.engine_callbacks.on_engine_running = Some(Box::new(on_engine_running));
        self
    }

    pub fn add_on_exit<U: 'static + Fn(&Arc<T>)>(mut self, on_exit: U) -> Self {
        self.engine_callbacks.on_exit = Some(Box::new(on_exit));
        self
    }
}

