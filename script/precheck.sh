#!/usr/bin/env bash
set -e

cargo +nightly fmt
cargo check
cargo clippy
cargo test -- --test-threads=1
python3 script/check_i18n.py
python3 script/check_icons.py
