#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="$ROOT_DIR/build"
ESP_DIR="$BUILD_DIR/esp"
EFI_BOOT_DIR="$ESP_DIR/EFI/BOOT"
EFI_IMAGE="$EFI_BOOT_DIR/BOOTX64.EFI"

mkdir -p "$EFI_BOOT_DIR"

cargo build \
    -Z build-std=core,compiler_builtins \
    --target x86_64-unknown-uefi

cp "$ROOT_DIR/target/x86_64-unknown-uefi/debug/rust-boot-test.efi" "$EFI_IMAGE"

echo "created $EFI_IMAGE"
