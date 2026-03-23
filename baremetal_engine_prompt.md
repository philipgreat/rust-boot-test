# x86_64 Bare Metal Matching Engine (Stage 1) — Codex Prompt

## 🎯 Project Goal

Build a minimal bare-metal runtime environment for x86_64 motherboard:

- Only one program exists
- No OS, no user/kernel separation
- Runs at Ring 0
- Uses CPU at maximum performance
- Boot → run immediately
- Focus ONLY on core runtime (not peripherals)

---

## 🧱 Stage 1 Scope (STRICT)

### MUST IMPLEMENT

1. CPU mode: x86_64 Long Mode
2. Boot flow: UEFI direct load (NO GRUB)
3. Memory:
   - Minimal page table
   - Map large physical memory region
4. Stack initialization
5. TSC timer (rdtsc / rdtscp)
6. Single core execution (BSP only)
7. Infinite main loop (busy-spin)
8. No context switch, no scheduler

---

### MUST NOT INCLUDE

- Multi-core bring-up
- Interrupt handling logic
- Device drivers
- Syscalls / user mode
- Filesystem
- Complex memory manager
- Networking

---

## 🧠 System Model

This is NOT an application.

This is a freestanding bootable runtime image:

UEFI Firmware → EFI Entry → Init → ExitBootServices → Runtime → Main Loop

---

## ⚙️ Requirements

### CPU Mode
- Must run in x86_64 long mode
- Flat segmentation

### Memory
- Paging required
- Prefer 2MB huge pages
- Identity or direct mapping

### Stack
- Must be initialized before main

### TSC Timer

Provide:
fn rdtsc() -> u64;
fn rdtscp() -> u64;

### Execution Model

init → warmup → loop:
    rdtsc → work → rdtsc → stats

### Interrupts
- Disable interrupts
- Minimal IDT recommended

---

## 🧩 Suggested Implementation (Rust)

- #![no_std]
- #![no_main]
- UEFI application

Crates:
- uefi
- uefi-services

---

## 🧪 Acceptance Criteria

1. Boots directly (no GRUB)
2. Runs in 64-bit mode
3. Infinite loop executes
4. TSC works
5. Memory accessible
6. No scheduler interference

---

## 💡 Philosophy

Minimal execution substrate for ultra-low-latency engine.
