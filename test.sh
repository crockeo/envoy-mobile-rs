#!/usr/bin/env bash

set -euf -o pipefail

cargo clean  # TODO: figure out how to get rid of this
maturin build \
	--release \
	--rustc-extra-args=-lenvoy_mobile \
	--interpreter=python3.8
pip uninstall -y envoy-requests
pip install ./target/wheels/envoy_requests-0.1.0-cp38-cp38-macosx_11_0_arm64.whl
python tests/test.py
