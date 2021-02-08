use envoy_mobile_sys;

use std::ffi::CStr;

pub fn config(fields: Vec<(&str, &str)>) -> String {
    // SAFETY: this is trivially safe as the config_template is written down as a static c-string inside of a
    // UTF8-encoded .c file
    let config_template;
    unsafe {
	config_template = CStr::from_ptr(envoy_mobile_sys::config_template);
    }
    let config_template = config_template.to_str().unwrap();
    
    let config = String::with_capacity(config_template.len());
    
    // TODO: keep pushing into the config from the config_template unless we're
    // in a template, and then find the right value instead.

    config
}
