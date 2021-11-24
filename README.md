# envoy-mobile-rs

Provides a wrapper and async interface to
[envoy-mobile](https://github.com/envoyproxy/envoy-mobile)
written in [Rust](https://www.rust-lang.org/).

## Building

envoy-mobile-rs requires that you have
`libenvoy_mobile.so` present on your library include path.
Unfortunately upstream envoy mobile does not yet
provide a way to build a .so from the common library.
In the meantime, you can do so by:

1. Cloning [my fork](https://github.com/crockeo/envoy-mobile)
1. Using Bazel to build `//:libenvoy_mobile.so`
1. Copying `libenvoy_mobile.so` from the build directory
   to a well known library include location (e.g. `/usr/local/lib`)
1. Running `cargo build`

I'm going to be opening up a PR upstream to allow builds from the main repo,
at which point I'll update this build process :)

## License

All rights reserved.
