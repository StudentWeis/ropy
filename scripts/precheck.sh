#!/usr/bin/env bash
set -e

rtk cargo +nightly fmt
rtk cargo check
rtk cargo clippy --all-targets --all-features
rtk cargo test -- --test-threads=1
python3 scripts/check/check_i18n.py
python3 scripts/check/check_icons.py
python3 scripts/check/check_themes.py
