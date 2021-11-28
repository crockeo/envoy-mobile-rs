#!/usr/bin/env bash

set -euf -o pipefail

cargo clean  # TODO: figure out how to get rid of this
maturin develop \
	--rustc-extra-args=-lenvoy_mobile
pytest benchmark.py -s
