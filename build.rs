use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rustc-link-search=.");
    println!("cargo:rustc-link-lib=envoy_mobile");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg("-I./envoy-mobile")
        .generate()
        .unwrap();

    bindings.write_to_file(out_dir.join("bindings.rs")).unwrap();
}
