#!/usr/bin/env bash

echo "\
// Copied from envoy-mobile (https://github.com/envoyproxy/envoy-mobile).
// Which is licensed under Apache 2.0. Code copyright
// the Envoy Project Authors." > wrapper.h
cat envoy-mobile/library/common/types/c_types.h >> wrapper.h
cat envoy-mobile/library/common/main_interface.h | grep -v "#include \"library.\\+\"" >> wrapper.h