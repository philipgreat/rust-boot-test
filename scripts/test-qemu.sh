#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="$ROOT_DIR/build"
ESP_DIR="$BUILD_DIR/esp"
OVMF_CODE_CANDIDATES=(
    "/usr/local/share/qemu/edk2-x86_64-code.fd"
    "/usr/share/OVMF/OVMF_CODE.fd"
    "/usr/share/edk2/ovmf/OVMF_CODE.fd"
)
OVMF_VARS_CANDIDATES=(
    "/usr/local/share/qemu/edk2-i386-vars.fd"
    "/usr/local/share/qemu/edk2-x86_64-code.fd"
    "/usr/share/OVMF/OVMF_VARS.fd"
    "/usr/share/edk2/ovmf/OVMF_VARS.fd"
)

find_first() {
    for path in "$@"; do
        if [[ -f "$path" ]]; then
            printf '%s\n' "$path"
            return 0
        fi
    done
    return 1
}

OVMF_CODE="$(find_first "${OVMF_CODE_CANDIDATES[@]}")"
OVMF_VARS_SRC="$(find_first "${OVMF_VARS_CANDIDATES[@]}")"
OVMF_VARS="$BUILD_DIR/OVMF_VARS.fd"

if [[ ! -f "$ESP_DIR/EFI/BOOT/BOOTX64.EFI" ]]; then
    echo "missing EFI image, run scripts/build-image.sh first"
    exit 1
fi

if [[ -z "${OVMF_CODE:-}" || -z "${OVMF_VARS_SRC:-}" ]]; then
    echo "missing OVMF firmware files"
    exit 1
fi

cp "$OVMF_VARS_SRC" "$OVMF_VARS"

QEMU_ARGS=(
    -machine q35
    -m 1024
    -drive "if=pflash,format=raw,readonly=on,file=$OVMF_CODE"
    -drive "if=pflash,format=raw,file=$OVMF_VARS"
    -drive "format=raw,file=fat:rw:$ESP_DIR"
    -no-reboot
    -no-shutdown
    -debugcon stdio
)

if [[ "${HEADLESS:-0}" == "1" ]]; then
    exec qemu-system-x86_64 "${QEMU_ARGS[@]}" -display none -serial none -debugcon stdio
fi

exec qemu-system-x86_64 "${QEMU_ARGS[@]}"
