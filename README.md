# envoy-mobile-rs

Provides a wrapper and async interface to
[envoy-mobile](https://github.com/envoyproxy/envoy-mobile)
written in [Rust](https://www.rust-lang.org/).

Also provides an envoy-mobile wrapper
in a [requests](https://pypi.org/project/requests)-like interface
that's super fast!

<details>
<summary>Benchmarking results</summary>
<pre>
<code>
----------------------------------------------------------------------------------------------------------- benchmark: 9 tests ----------------------------------------------------------------------------------------------------------
Name (time in us)                                   Min                     Max                    Mean                StdDev                  Median                   IQR            Outliers         OPS            Rounds  Iterations
-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
test_performance[1-envoy_requests]             611.8330 (1.0)          736.1250 (1.0)          672.3000 (1.0)         55.2797 (1.0)          644.9590 (1.0)         93.4483 (2.44)          2;0  1,487.4312 (1.0)           5           1
test_performance[1-requests_session]           940.3340 (1.54)       1,956.9590 (2.66)       1,012.0325 (1.51)        63.8324 (1.15)         999.0420 (1.55)        38.3535 (1.0)         65;53    988.1105 (0.66)        752           1
test_performance[1-requests]                 1,234.7500 (2.02)       1,690.5830 (2.30)       1,346.2345 (2.00)        91.5738 (1.66)       1,319.4585 (2.05)        70.5210 (1.84)        21;13    742.8126 (0.50)        132           1
test_performance[10-envoy_requests]          3,658.2090 (5.98)       4,297.1670 (5.84)       3,869.7184 (5.76)       113.8674 (2.06)       3,855.1245 (5.98)       148.6660 (3.88)         56;2    258.4167 (0.17)        186           1
test_performance[10-requests_session]        8,512.3750 (13.91)      9,462.7090 (12.85)      8,981.5992 (13.36)      179.4580 (3.25)       8,986.7500 (13.93)      218.0840 (5.69)         31;3    111.3387 (0.07)         97           1
test_performance[10-requests]               10,352.0000 (16.92)     17,292.0840 (23.49)     11,078.7854 (16.48)      823.2228 (14.89)     10,923.0830 (16.94)      482.7815 (12.59)         3;3     90.2626 (0.06)         79           1
test_performance[100-envoy_requests]        35,108.0830 (57.38)     41,907.4580 (56.93)     37,060.9867 (55.13)    1,539.1634 (27.84)     36,778.0620 (57.02)    2,040.4170 (53.20)         5;1     26.9826 (0.02)         22           1
test_performance[100-requests_session]      90,539.5830 (147.98)    98,531.9160 (133.85)    92,629.1819 (137.78)   2,508.8304 (45.38)     91,733.9170 (142.23)   1,446.1975 (37.71)         2;2     10.7957 (0.01)         11           1
test_performance[100-requests]             101,515.3750 (165.92)   112,601.4170 (152.97)   105,248.9669 (156.55)   3,501.9664 (63.35)    104,494.2085 (162.02)   2,073.6260 (54.07)         4;2      9.5013 (0.01)         10           1
-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
</code>
</pre>
</details>

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

MIT Open Source License. See the [LICENSE](/LICENSE) document for terms.

Based on [envoy-mobile](https://github.com/envoyproxy/envoy-mobile)
licensed under the MIT Open Source License.
