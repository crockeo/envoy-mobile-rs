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
--------------------------------------------------------------------------------------------------------- benchmark: 9 tests --------------------------------------------------------------------------------------------------------
Name (time in us)                                  Min                    Max                   Mean                StdDev                 Median                   IQR            Outliers         OPS            Rounds  Iterations
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
test_performance[1-envoy_requests]            562.2080 (1.0)         727.9170 (1.0)         610.9918 (1.0)         67.1059 (2.66)        586.9170 (1.0)         62.9275 (3.61)          1;0  1,636.6832 (1.0)           5           1
test_performance[1-requests_session]          795.2080 (1.41)      1,244.2500 (1.71)        840.5290 (1.38)        25.1976 (1.0)         840.1460 (1.43)        17.4170 (1.0)        149;61  1,189.7270 (0.73)        830           1
test_performance[1-requests]                1,060.8750 (1.89)      1,511.3330 (2.08)      1,135.1148 (1.86)        66.9845 (2.66)      1,126.5000 (1.92)        90.4685 (5.19)         19;2    880.9681 (0.54)        115           1
test_performance[10-envoy_requests]         3,410.3750 (6.07)      6,924.0410 (9.51)      3,699.5009 (6.05)       285.8807 (11.35)     3,651.8960 (6.22)       152.7080 (8.77)          6;5    270.3067 (0.17)        180           1
test_performance[10-requests_session]       7,001.8330 (12.45)     9,815.6670 (13.48)     7,334.7912 (12.00)      342.2402 (13.58)     7,252.4790 (12.36)      263.4580 (15.13)        11;9    136.3365 (0.08)        114           1
test_performance[10-requests]               9,121.3330 (16.22)    15,756.3750 (21.65)     9,833.6634 (16.09)      794.2537 (31.52)     9,693.2085 (16.52)      559.1250 (32.10)         4;4    101.6915 (0.06)         90           1
test_performance[100-envoy_requests]       32,927.8750 (58.57)    39,767.4170 (54.63)    34,432.2936 (56.35)    1,335.1637 (52.99)    34,198.5630 (58.27)      626.9590 (36.00)         2;3     29.0425 (0.02)         22           1
test_performance[100-requests_session]     81,038.8330 (144.14)   88,714.2910 (121.87)   84,053.4005 (137.57)   2,609.0043 (103.54)   83,830.4160 (142.83)   4,055.3640 (232.84)        4;0     11.8972 (0.01)         13           1
test_performance[100-requests]             89,363.0420 (158.95)   97,099.9590 (133.39)   92,491.8980 (151.38)   3,089.9031 (122.63)   91,736.1670 (156.30)   5,751.9267 (330.25)        4;0     10.8118 (0.01)         11           1
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------</code>
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
