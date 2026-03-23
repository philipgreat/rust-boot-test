#!/usr/bin/env zsh
set -euo pipefail

# 用法:
#   ./write_uefi_disk_safe.sh /path/to/your.efi [diskN]
#
# 例子:
#   ./write_uefi_disk_safe.sh ./target/x86_64-unknown-uefi/release/app.efi disk6

EFI_FILE="${1:-}"
TARGET_DISK="${2:-disk6}"
VOL_NAME="BAREMETAL"

# 允许的容量范围（GiB）
MIN_GIB=100
MAX_GIB=140

if [[ -z "$EFI_FILE" ]]; then
  echo "Usage: $0 /path/to/your.efi [diskN]"
  exit 1
fi

if [[ ! -f "$EFI_FILE" ]]; then
  echo "EFI file not found: $EFI_FILE"
  exit 1
fi

if [[ ! "$TARGET_DISK" =~ ^disk[0-9]+$ ]]; then
  echo "Invalid disk name: $TARGET_DISK"
  echo "Expected format: disk6"
  exit 1
fi

if ! command -v diskutil >/dev/null 2>&1; then
  echo "diskutil not found. This script must run on macOS."
  exit 1
fi

echo "Inspecting /dev/$TARGET_DISK ..."
INFO="$(diskutil info "/dev/$TARGET_DISK" 2>/dev/null || true)"

if [[ -z "$INFO" ]]; then
  echo "Cannot read disk info for /dev/$TARGET_DISK"
  exit 1
fi

# 提取关键信息
DEVICE_NODE="$(echo "$INFO" | awk -F': *' '/Device Node/ {print $2; exit}')"
WHOLE_DISK="$(echo "$INFO" | awk -F': *' '/Whole/ {print $2; exit}')"
PROTOCOL="$(echo "$INFO" | awk -F': *' '/Protocol/ {print $2; exit}')"
LOCATION="$(echo "$INFO" | awk -F': *' '/Device Location/ {print $2; exit}')"
INTERNAL="$(echo "$INFO" | awk -F': *' '/Internal/ {print $2; exit}')"
REMOVABLE="$(echo "$INFO" | awk -F': *' '/Removable Media/ {print $2; exit}')"
TOTAL_SIZE_LINE="$(echo "$INFO" | awk -F': *' '/Disk Size/ {print $2; exit}')"
MEDIA_NAME="$(echo "$INFO" | awk -F': *' '/Media Name/ {print $2; exit}')"

# 从 "128.0 GB (128035676160 Bytes)" 里提取字节数
SIZE_BYTES="$(echo "$TOTAL_SIZE_LINE" | sed -n 's/.*(\([0-9][0-9]*\) Bytes).*/\1/p')"

if [[ -z "$SIZE_BYTES" ]]; then
  echo "Failed to parse disk size from:"
  echo "  $TOTAL_SIZE_LINE"
  exit 1
fi

SIZE_GIB=$(( SIZE_BYTES / 1024 / 1024 / 1024 ))

echo
echo "========== Disk Verification =========="
echo "Device Node    : ${DEVICE_NODE:-unknown}"
echo "Media Name     : ${MEDIA_NAME:-unknown}"
echo "Whole Disk     : ${WHOLE_DISK:-unknown}"
echo "Protocol       : ${PROTOCOL:-unknown}"
echo "Location       : ${LOCATION:-unknown}"
echo "Internal       : ${INTERNAL:-unknown}"
echo "Removable      : ${REMOVABLE:-unknown}"
echo "Disk Size      : ${TOTAL_SIZE_LINE:-unknown}"
echo "Approx GiB     : ${SIZE_GIB} GiB"
echo "======================================="
echo

# 多重校验
if [[ "$DEVICE_NODE" != "/dev/$TARGET_DISK" ]]; then
  echo "Refuse: device node mismatch."
  exit 1
fi

WHOLE_DISK_LC="$(printf '%s' "${WHOLE_DISK:-unknown}" | tr '[:upper:]' '[:lower:]')"
INTERNAL_LC="$(printf '%s' "${INTERNAL:-unknown}" | tr '[:upper:]' '[:lower:]')"
PROTOCOL_LC="$(printf '%s' "${PROTOCOL:-unknown}" | tr '[:upper:]' '[:lower:]')"
LOCATION_LC="$(printf '%s' "${LOCATION:-unknown}" | tr '[:upper:]' '[:lower:]')"

if [[ "$WHOLE_DISK_LC" != "yes" ]]; then
  echo "Refuse: target is not a whole disk."
  exit 1
fi

# 只拒绝“明确是 internal”的盘；unknown 不拒绝
if [[ "$INTERNAL_LC" == "yes" ]]; then
  echo "Refuse: target disk is internal."
  exit 1
fi

# 要求位置为 external
if [[ "$LOCATION_LC" != "external" ]]; then
  echo "Refuse: target disk is not external. location=$LOCATION_LC"
  exit 1
fi

# external physical 的近似判断：
# 物理盘通常 Whole: Yes 且不是 disk image
if [[ "$TARGET_DISK" == "disk4" || "$TARGET_DISK" == "disk5" ]]; then
  echo "Refuse: looks like a disk image, not a physical external disk."
  exit 1
fi

# 协议可按需放宽，常见是 USB / USB-Interface
case "$PROTOCOL_LC" in
  usb|usb-interface|unknown)
    ;;
  *)
    echo "Refuse: protocol is not USB-like: $PROTOCOL"
    exit 1
    ;;
esac

if (( SIZE_GIB < MIN_GIB || SIZE_GIB > MAX_GIB )); then
  echo "Refuse: disk size ${SIZE_GIB} GiB is outside expected range ${MIN_GIB}-${MAX_GIB} GiB."
  exit 1
fi

echo "Verification passed."
echo
echo "Target EFI file : $EFI_FILE"
echo "Target disk     : /dev/$TARGET_DISK"
echo "Volume name     : $VOL_NAME"
echo
echo "WARNING: This will ERASE the whole disk /dev/$TARGET_DISK"
read -r -p "Type exactly: ERASE-$TARGET_DISK to continue: " CONFIRM

if [[ "$CONFIRM" != "ERASE-$TARGET_DISK" ]]; then
  echo "Aborted."
  exit 1
fi

echo
echo "[1/6] Unmounting disk..."
diskutil unmountDisk force "/dev/$TARGET_DISK" || true

echo "[2/6] Erasing disk and creating GPT + FAT32..."
diskutil eraseDisk FAT32 "$VOL_NAME" GPT "/dev/$TARGET_DISK"

MOUNT_POINT="/Volumes/$VOL_NAME"

echo "[3/6] Waiting for mount point..."
for _ in {1..15}; do
  if [[ -d "$MOUNT_POINT" ]]; then
    break
  fi
  sleep 1
done

if [[ ! -d "$MOUNT_POINT" ]]; then
  echo "Mount point not found: $MOUNT_POINT"
  exit 1
fi

echo "[4/6] Creating UEFI boot path..."
mkdir -p "$MOUNT_POINT/EFI/BOOT"

echo "[5/6] Copying EFI binary..."
cp "$EFI_FILE" "$MOUNT_POINT/EFI/BOOT/BOOTX64.EFI"
sync

echo "[6/6] Verifying copied file..."
if [[ ! -f "$MOUNT_POINT/EFI/BOOT/BOOTX64.EFI" ]]; then
  echo "Verification failed: BOOTX64.EFI not found after copy."
  exit 1
fi

SRC_SIZE=$(stat -f%z "$EFI_FILE")
DST_SIZE=$(stat -f%z "$MOUNT_POINT/EFI/BOOT/BOOTX64.EFI")

echo "Source size : $SRC_SIZE bytes"
echo "Dest size   : $DST_SIZE bytes"

if [[ "$SRC_SIZE" != "$DST_SIZE" ]]; then
  echo "Verification failed: copied file size mismatch."
  exit 1
fi

echo
echo "Directory layout:"
find "$MOUNT_POINT/EFI" -maxdepth 3 -print

echo
echo "Success. Boot file installed at:"
echo "  $MOUNT_POINT/EFI/BOOT/BOOTX64.EFI"

echo
echo "Ejecting disk..."
diskutil eject "/dev/$TARGET_DISK" || true

echo
echo "All done."
echo "You can now plug the disk into the target machine and boot in UEFI mode."
