all:
	scripts/build-image.sh
	bash scripts/prepare-usb-disk.sh build/esp/EFI/BOOT/BOOTX64.EFI disk6
test:
	scripts/build-image.sh
	scripts/test-qemu.sh

	