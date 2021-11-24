#![allow(dead_code)]
#![allow(deref_nullptr)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(unaligned_references)]

#[link(name = "envoy_mobile")]
extern "C" {
    pub static config_template: *const ::std::os::raw::c_char;
}

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
