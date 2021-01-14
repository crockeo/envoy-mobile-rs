use envoy_mobile_sys;

use std::ffi::{c_void, CStr, CString};
use std::sync::Arc;

use super::log_level::LogLevel;
use super::result::{Error, Result};
use super::stream::StreamBuilder;

pub struct EngineCallbacks<T> {
    on_engine_running: Option<fn(&Arc<T>)>,
    on_exit: Option<fn(&Arc<T>)>,
}

impl<T> EngineCallbacks<T> {
    fn new() -> Self {
        Self {
            on_engine_running: None,
            on_exit: None,
        }
    }
}

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

impl<T: Default + Sync> EngineBuilder<T> {
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

    pub fn add_on_engine_running(mut self, on_engine_running: fn(&Arc<T>)) -> Self {
        self.engine_callbacks.on_engine_running = Some(on_engine_running);
        self
    }

    pub fn add_on_exit(mut self, on_exit: fn(&Arc<T>)) -> Self {
        self.engine_callbacks.on_exit = Some(on_exit);
        self
    }
}

struct EngineContextWrapper<T> {
    context: Arc<T>,
    engine_callbacks: EngineCallbacks<T>,
}

pub struct Engine<T> {
    context_wrapper_ptr: *mut EngineContextWrapper<T>,
    handle: envoy_mobile_sys::envoy_engine_t,
}

impl<T: Sync> Engine<T> {
    fn new(
        config: String,
        log_level: LogLevel,
        context: Arc<T>,
        engine_callbacks: EngineCallbacks<T>,
    ) -> Result<Self> {
        let context_wrapper = EngineContextWrapper {
            context,
            engine_callbacks,
        };
        let context_wrapper_ptr = Box::into_raw(Box::new(context_wrapper));

        // SAFETY: This is trivially correct, so long as we trust envoy-mobile. Generally speaking,
        // init_engine is nothing to worry about. Pay more attention to the call to run_engine.
        let handle;
        unsafe {
            handle = envoy_mobile_sys::init_engine();
        }
        if handle == 0 {
            return Err(Error::InvalidHandle);
        }

        let envoy_engine_callbacks = envoy_mobile_sys::envoy_engine_callbacks {
            on_engine_running: Some(Engine::<T>::dispatch_on_engine_running),
            on_exit: Some(Engine::<T>::dispatch_on_exit),
            context: context_wrapper_ptr as *mut c_void,
        };
        let config_c_str = CString::new(config).unwrap();
        let log_level_c_str = CString::new(log_level.to_string()).unwrap();

        // SAFETY: This call is safe so long as the data going in is formatted correctly. A couple
        // of common errors:
        //   - The context portion of the envoy_engine_callbacks doesn't live long enough, causing
        //     SIGSEGVs
        //   - The config YAML is formatted improperly, and that causes a yaml-cpp error.
        //   - Either of the strings aren't formatted correctly, causing buffer overflows.
        let status;
        unsafe {
            status = envoy_mobile_sys::run_engine(
                handle,
                envoy_engine_callbacks,
                config_c_str.as_bytes_with_nul().as_ptr() as *const i8,
                log_level_c_str.as_bytes_with_nul().as_ptr() as *const i8,
            );
        }
        if status == 1 {
            return Err(Error::CouldNotInit);
        }

        Ok(Self {
            context_wrapper_ptr,
            handle,
        })
    }

    pub fn stream_builder<U: Sync>(&self, context: Arc<U>) -> StreamBuilder<U> {
        // SAFETY: this is trivially safe; guaranteed not to fail.
        let stream_handle;
        unsafe {
            stream_handle = envoy_mobile_sys::init_stream(self.handle);
        }
        StreamBuilder::new(context, stream_handle)
    }

    pub fn terminate(self) {
        unsafe {
            envoy_mobile_sys::terminate_engine(self.handle);
        }
    }

    unsafe extern "C" fn dispatch_on_engine_running(context: *mut c_void) {
        let context = context as *const EngineContextWrapper<T>;
        if let Some(on_engine_running) = &(*context).engine_callbacks.on_engine_running {
            on_engine_running(&(*context).context);
        }
    }

    unsafe extern "C" fn dispatch_on_exit(context: *mut c_void) {
        let context = context as *const EngineContextWrapper<T>;
        if let Some(on_exit) = &(*context).engine_callbacks.on_exit {
            on_exit(&(*context).context);
        }
    }
}

impl<T> Drop for Engine<T> {
    fn drop(&mut self) {
        unsafe {
            let _ = Box::from_raw(self.context_wrapper_ptr);
        }
    }
}
