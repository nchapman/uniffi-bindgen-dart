#!/usr/bin/env bash
set -euo pipefail

docker run --rm -v "$PWD":/workspace -w /workspace rust:1.85 bash -lc "./scripts/test_bindings.sh"
