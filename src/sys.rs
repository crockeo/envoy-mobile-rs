#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

#[link(name = "envoy_mobile")]
extern "C" {
    pub static config_template: *const ::std::os::raw::c_char;
}

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
