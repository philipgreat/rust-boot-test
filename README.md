# rust-boot-test

## Goal

Stage 1 now targets an `x86_64` bare-metal runtime loaded directly by UEFI firmware:

- no OS
- no GRUB
- ring 0 only
- single BSP core
- no scheduler
- no interrupts

Boot flow:

`UEFI firmware -> EFI entry -> ExitBootServices -> paging setup -> TSC loop`

## Stage 1 Runtime

Current implementation in [`src/main.rs`](/Users/Philip/githome/rust-boot-test/src/main.rs):

- UEFI direct entry
- explicit stack handoff in [`src/boot.s`](/Users/Philip/githome/rust-boot-test/src/boot.s)
- largest available UEFI text mode for big console output
- console input echo before entering the runtime stage
- `x86_64` long mode runtime
- interrupts disabled after boot services exit
- minimal identity paging with `2MiB` huge pages
- direct mapping of up to `64GiB` physical memory
- `rdtsc()` / `rdtscp()`
- warmup + busy-spin runtime loop
- `debugcon` output for headless QEMU verification

## Build

```bash
./scripts/build-image.sh
```

This produces:

- [`build/esp/EFI/BOOT/BOOTX64.EFI`](/Users/Philip/githome/rust-boot-test/build/esp/EFI/BOOT/BOOTX64.EFI)

## Run

Graphical QEMU:

```bash
./scripts/test-qemu.sh
```

Headless verification:

```bash
HEADLESS=1 ./scripts/test-qemu.sh
```

Headless logs are written to:

- [`build/debugcon.log`](/Users/Philip/githome/rust-boot-test/build/debugcon.log)

Interactive boot:

- the firmware console switches to the largest available text mode
- keyboard input is echoed to the console
- pressing `Enter` continues into the runtime
- if no input arrives, runtime starts automatically after a short poll window

## Dependencies

- Rust nightly
- `rust-src`
- QEMU with OVMF/edk2 firmware

macOS:

```bash
./scripts/install-qemu-mac.sh
```

Ubuntu:

```bash
./scripts/install-qemu-ubuntu.sh
```
