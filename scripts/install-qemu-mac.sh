#!/usr/bin/env bash
set -euo pipefail

if command -v brew >/dev/null 2>&1; then
    brew install qemu
else
    echo "Homebrew is required on macOS: https://brew.sh/"
    exit 1
fi
