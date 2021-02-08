# envoy-mobile-rs

⚠️ ⚠️ ⚠️

At present, this **does not build** because `bazel` requires a git repo,
which `cargo` strips away for dependencies. I don't have time to fix it just yet,
but bear with me.

⚠️ ⚠️ ⚠️

Rust bindings to [envoy-mobile](https://github.com/envoyproxy/envoy-mobile).

## What?

[envoy](https://github.com/envoyproxy/envoy) is a "cloud-native high-performance
edge/middle/service proxy."

[envoy-mobile](https://github.com/envoyproxy/envoy-mobile) is a wrapper around envoy that:

* Provides a C interface to interop with Envoy.
* Implements a collection of bindings to that interface for Kotlin, Swift, and Python.

envoy-mobile was originally used to bring envoy to mobile devices, but can, in general, bring envoy
in-process instead of alongside as a sidecar.

## Why?

Rust already has a great HTTP ecoystem out there with [hyper](https://github.com/hyperium/hyper)
powering a whole bunch of projects like [warp](https://github.com/seanmonstar/warp). If you're
writing your own HTTP-related project for fun then I suggest you go that path instead.

If you're in the microservice world, there's a sizable chance that you're using
[envoy](https://github.com/envoyproxy/envoy) _at least_ as a sidecar, if not the front proxy as
well. It can be advantageous to have the same HTTP implementation across each process. That's where
envoy-mobile-rs comes in; in such a situation, you may want to embed Envoy in-process to handle
your HTTP.
