#!/usr/bin/env bash
# BM-05 (spec 10.3): compile time of a synthetic machine with 200 states and
# 600 edges, target "under 3 s of expansion + checking on a standard CI
# machine".
#
# Generates the machine (tools/bm05), writes it into a throwaway crate under
# target/bm05 that depends on estado-core and estado-macros by path, warms the
# dependencies up, then times `cargo check` of that crate only (median of
# --runs runs, incremental compilation off) and exits 1 when it takes longer
# than --fail-above seconds.
#
# Usage: scripts/bm05.sh [--runs N] [--fail-above SECONDS]
#   defaults: 3 runs, fail above 3.0 s (the spec target). CI passes a more
#   generous threshold, see .github/workflows/ci.yml.
set -euo pipefail

cd "$(dirname "$0")/.."
exec cargo run --quiet --manifest-path tools/bm05/Cargo.toml -- run "$@"
