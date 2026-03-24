#!/usr/bin/env bash
set -e

# Check code formatting, linting, and tests before committing
./scripts/precheck.sh
./scripts/record_build_size.sh
