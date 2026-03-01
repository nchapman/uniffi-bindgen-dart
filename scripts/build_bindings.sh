#!/usr/bin/env bash
set -euo pipefail

# Scaffold placeholder: verify CLI is callable.
cargo run -p ubdg_bindgen --bin uniffi-bindgen-dart -- --help >/dev/null

echo "Binding generation script scaffolded. Add fixture generation commands here."
