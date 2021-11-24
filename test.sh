#!/usr/bin/env bash

maturin develop \
	--rustc-extra-args=-lenvoy_mobile
python3 test.py
