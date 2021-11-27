#!/usr/bin/env bash

set -euf -o pipefail

cargo test not_a_test_just_to_build 2>&1 > /dev/null
codesign -s - -f --entitlements ~/ent.plist target/debug/deps/envoy_mobile-9fe14092f8657e23
sudo leaks --groupByType --atExit -- target/debug/deps/envoy_mobile-9fe14092f8657e23 loop_trigger_memory_leak --nocapture
