#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::ffi::c_void;
use core::fmt::{self, Write};
use core::mem::size_of;
use core::ptr::{addr_of, addr_of_mut};

global_asm!(include_str!("boot.s"));

type EfiHandle = *mut c_void;
type EfiStatus = usize;
type EfiPhysicalAddress = u64;
type EfiVirtualAddress = u64;
type EfiMemoryType = u32;
type EfiAllocateType = u32;

const EFI_SUCCESS: EfiStatus = 0;
const PAGE_PRESENT: u64 = 1 << 0;
const PAGE_WRITABLE: u64 = 1 << 1;
const PAGE_LARGE: u64 = 1 << 7;
const PAGE_SIZE_2M: u64 = 2 * 1024 * 1024;
const ONE_GIB: u64 = 1024 * 1024 * 1024;
const MAX_MAPPED_GIB: usize = 64;
const MEMORY_MAP_CAPACITY: usize = 64 * 1024;

#[repr(C)]
struct EfiTableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    reserved: u32,
}

#[repr(C)]
struct EfiSimpleTextOutputProtocol {
    reset: usize,
    output_string: extern "efiapi" fn(*mut EfiSimpleTextOutputProtocol, *const u16) -> EfiStatus,
}

#[repr(C)]
struct EfiBootServices {
    hdr: EfiTableHeader,
    raise_tpl: usize,
    restore_tpl: usize,
    allocate_pages: extern "efiapi" fn(
        EfiAllocateType,
        EfiMemoryType,
        usize,
        *mut EfiPhysicalAddress,
    ) -> EfiStatus,
    free_pages: usize,
    get_memory_map: extern "efiapi" fn(
        *mut usize,
        *mut EfiMemoryDescriptor,
        *mut usize,
        *mut usize,
        *mut u32,
    ) -> EfiStatus,
    allocate_pool: usize,
    free_pool: usize,
    create_event: usize,
    set_timer: usize,
    wait_for_event: usize,
    signal_event: usize,
    close_event: usize,
    check_event: usize,
    install_protocol_interface: usize,
    reinstall_protocol_interface: usize,
    uninstall_protocol_interface: usize,
    handle_protocol: usize,
    reserved: usize,
    register_protocol_notify: usize,
    locate_handle: usize,
    locate_device_path: usize,
    install_configuration_table: usize,
    load_image: usize,
    start_image: usize,
    exit: usize,
    unload_image: usize,
    exit_boot_services: extern "efiapi" fn(EfiHandle, usize) -> EfiStatus,
}

#[repr(C)]
struct EfiSystemTable {
    hdr: EfiTableHeader,
    firmware_vendor: *const u16,
    firmware_revision: u32,
    console_in_handle: EfiHandle,
    con_in: *mut c_void,
    console_out_handle: EfiHandle,
    con_out: *mut EfiSimpleTextOutputProtocol,
    standard_error_handle: EfiHandle,
    std_err: *mut EfiSimpleTextOutputProtocol,
    runtime_services: *mut c_void,
    boot_services: *mut EfiBootServices,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct EfiMemoryDescriptor {
    typ: u32,
    pad: u32,
    physical_start: EfiPhysicalAddress,
    virtual_start: EfiVirtualAddress,
    number_of_pages: u64,
    attribute: u64,
}

#[repr(C, align(4096))]
#[derive(Clone, Copy)]
struct PageTable([u64; 512]);

static mut MEMORY_MAP_BUFFER: [u8; MEMORY_MAP_CAPACITY] = [0; MEMORY_MAP_CAPACITY];
static mut PML4: PageTable = PageTable([0; 512]);
static mut PDP: PageTable = PageTable([0; 512]);
static mut PAGE_DIRECTORIES: [PageTable; MAX_MAPPED_GIB] = [PageTable([0; 512]); MAX_MAPPED_GIB];
static mut RUNTIME_SCRATCH: [u64; 64] = [0; 64];

#[unsafe(no_mangle)]
extern "efiapi" fn rust_efi_main(image_handle: EfiHandle, system_table: *mut EfiSystemTable) -> EfiStatus {
    let mut console = ConsoleWriter::new(system_table);
    let mut debug = DebugCon;

    let _ = writeln!(console, "x86_64 bare metal runtime stage1");
    let _ = writeln!(console, "uefi direct load");
    let _ = writeln!(debug, "x86_64 bare metal runtime stage1");
    let _ = writeln!(debug, "uefi direct load");
    let _ = writeln!(debug, "phase=boot_services");

    let boot_services = unsafe { (*system_table).boot_services };
    let (memory_top, descriptor_count) = match exit_boot_services(image_handle, boot_services) {
        Ok(result) => result,
        Err(status) => {
            let _ = writeln!(console, "ExitBootServices failed: {status:#x}");
            let _ = writeln!(debug, "ExitBootServices failed: {status:#x}");
            return status;
        }
    };

    let _ = writeln!(debug, "phase=after_exit_boot_services");
    disable_interrupts();
    let _ = writeln!(console, "phase=interrupts_disabled");

    let mapped_gib = unsafe { install_identity_mapping(memory_top) };
    let _ = writeln!(console, "phase=paging_live");
    let probe = unsafe { verify_runtime_memory() };
    let _ = writeln!(console, "phase=memory_probe_done");
    let has_rdtscp = rdtscp_supported();
    let tsc0 = read_tsc(has_rdtscp);
    let tsc1 = read_tsc(has_rdtscp);

    let _ = writeln!(console, "exit boot services ok");
    let _ = writeln!(console, "memory descriptors={descriptor_count}");
    let _ = writeln!(console, "memory top=0x{memory_top:016x}");
    let _ = writeln!(console, "mapped={mapped_gib}GiB using 2MiB pages");
    let _ = writeln!(console, "probe=0x{probe:016x}");
    let _ = writeln!(console, "rdtscp_supported={has_rdtscp}");
    let _ = writeln!(console, "tsc delta={}", tsc1 - tsc0);
    
    warmup();
    runtime_loop(mapped_gib, memory_top, has_rdtscp)
}

fn exit_boot_services(
    image_handle: EfiHandle,
    boot_services: *mut EfiBootServices,
) -> Result<(u64, usize), EfiStatus> {
    let mut last_error = EFI_SUCCESS;
    let mut debug = DebugCon;

    for _ in 0..2 {
        let mut map_size = MEMORY_MAP_CAPACITY;
        let mut map_key = 0usize;
        let mut descriptor_size = 0usize;
        let mut descriptor_version = 0u32;

        let _ = writeln!(debug, "phase=get_memory_map");

        let status = unsafe {
            ((*boot_services).get_memory_map)(
                &mut map_size,
                addr_of_mut!(MEMORY_MAP_BUFFER).cast::<EfiMemoryDescriptor>(),
                &mut map_key,
                &mut descriptor_size,
                &mut descriptor_version,
            )
        };
        if status != EFI_SUCCESS {
            let _ = writeln!(debug, "get_memory_map status={status:#x}");
            last_error = status;
            continue;
        }

        let memory_top = unsafe { highest_mapped_address(map_size, descriptor_size) };
        let descriptor_count = map_size / descriptor_size.max(size_of::<EfiMemoryDescriptor>());
        let _ = writeln!(
            debug,
            "memory_map ok size={map_size} desc_size={descriptor_size} desc_ver={descriptor_version}"
        );

        let exit_status = unsafe { ((*boot_services).exit_boot_services)(image_handle, map_key) };
        if exit_status == EFI_SUCCESS {
            let _ = writeln!(debug, "exit_boot_services status=0x0");
            return Ok((memory_top, descriptor_count));
        }

        let _ = writeln!(debug, "exit_boot_services status={exit_status:#x}");
        last_error = exit_status;
    }

    Err(last_error)
}

unsafe fn highest_mapped_address(map_size: usize, descriptor_size: usize) -> u64 {
    let mut highest = 0u64;
    let buffer = addr_of!(MEMORY_MAP_BUFFER).cast::<u8>();
    let count = map_size / descriptor_size.max(size_of::<EfiMemoryDescriptor>());

    for i in 0..count {
        let descriptor = unsafe { &*buffer.add(i * descriptor_size).cast::<EfiMemoryDescriptor>() };
        let end = descriptor.physical_start + descriptor.number_of_pages * 4096;
        if end > highest {
            highest = end;
        }
    }

    highest
}

unsafe fn install_identity_mapping(memory_top: u64) -> usize {
    let requested = memory_top.div_ceil(ONE_GIB) as usize;
    let mapped_gib = requested.clamp(1, MAX_MAPPED_GIB);

    unsafe {
        PML4.0 = [0; 512];
        PDP.0 = [0; 512];
        PAGE_DIRECTORIES = [PageTable([0; 512]); MAX_MAPPED_GIB];
    }

    unsafe {
        PML4.0[0] = addr_of!(PDP) as u64 | PAGE_PRESENT | PAGE_WRITABLE;
    }

    for gib in 0..mapped_gib {
        unsafe {
            PDP.0[gib] = addr_of!(PAGE_DIRECTORIES[gib]) as u64 | PAGE_PRESENT | PAGE_WRITABLE;
        }

        for entry in 0..512 {
            let physical = ((gib as u64 * 512) + entry as u64) * PAGE_SIZE_2M;
            unsafe {
                PAGE_DIRECTORIES[gib].0[entry] =
                    physical | PAGE_PRESENT | PAGE_WRITABLE | PAGE_LARGE;
            }
        }
    }

    unsafe {
        asm!(
            "mov cr3, {0}",
            in(reg) addr_of!(PML4) as u64,
            options(nostack, preserves_flags)
        );
    }

    mapped_gib
}

unsafe fn verify_runtime_memory() -> u64 {
    let base = rdtsc();
    let scratch = addr_of_mut!(RUNTIME_SCRATCH).cast::<u64>();

    for i in 0..64 {
        unsafe {
            scratch.add(i).write_volatile(base.wrapping_add(i as u64));
        }
    }

    let mut checksum = 0u64;
    for i in 0..64 {
        checksum ^= unsafe { scratch.add(i).read_volatile() };
    }
    checksum
}

fn warmup() {
    for _ in 0..100_000 {
        core::hint::spin_loop();
    }
}

fn runtime_loop(mapped_gib: usize, memory_top: u64, has_rdtscp: bool) -> ! {
    let mut debug = DebugCon;
    let mut last: u64;
    let mut min = u64::MAX;
    let mut max = 0u64;
    let mut iterations = 0u64;

    let _ = writeln!(debug, "phase=loop_live");

    loop {
        let start = read_tsc(has_rdtscp);
        busy_work();
        let end = read_tsc(has_rdtscp);
        let delta = end.wrapping_sub(start);

        last = delta;
        min = min.min(delta);
        max = max.max(delta);
        iterations = iterations.wrapping_add(1);

        if iterations % 10_000 == 0 {
            let _ = writeln!(
                debug,
                "loop={iterations} last={last} min={min} max={max} mapped={mapped_gib}GiB top=0x{memory_top:016x}"
            );
        }
    }
}

fn busy_work() {
    for _ in 0..1024 {
        core::hint::spin_loop();
    }
}

fn disable_interrupts() {
    unsafe {
        asm!("cli", options(nomem, nostack, preserves_flags));
    }
}

pub fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags));
    }
    ((hi as u64) << 32) | lo as u64
}

pub fn rdtscp_supported() -> bool {
    let max_extended = cpuid(0x8000_0000).0;
    if max_extended < 0x8000_0001 {
        return false;
    }

    let (_, _, _, edx) = cpuid(0x8000_0001);
    edx & (1 << 27) != 0
}

pub fn rdtscp() -> u64 {
    let lo: u32;
    let hi: u32;
    let aux: u32;
    unsafe {
        asm!(
            "rdtscp",
            out("eax") lo,
            out("edx") hi,
            out("ecx") aux,
            options(nomem, nostack)
        );
    }
    let _ = aux;
    ((hi as u64) << 32) | lo as u64
}

fn read_tsc(has_rdtscp: bool) -> u64 {
    if has_rdtscp {
        rdtscp()
    } else {
        rdtsc()
    }
}

fn cpuid(leaf: u32) -> (u32, u32, u32, u32) {
    let eax: u32;
    let ebx: u32;
    let ecx: u32;
    let edx: u32;

    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            inlateout("eax") leaf => eax,
            lateout("ecx") ecx,
            lateout("edx") edx,
            ebx_out = lateout(reg) ebx,
            options(preserves_flags)
        );
    }

    (eax, ebx, ecx, edx)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    let mut debug = DebugCon;
    let _ = writeln!(debug, "panic");
    loop {
        core::hint::spin_loop();
    }
}

struct ConsoleWriter {
    system_table: *mut EfiSystemTable,
}

impl ConsoleWriter {
    const fn new(system_table: *mut EfiSystemTable) -> Self {
        Self { system_table }
    }
}

impl Write for ConsoleWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let con_out = unsafe { (*self.system_table).con_out };
        if con_out.is_null() {
            return Ok(());
        }

        let mut utf16 = [0u16; 128];
        let mut cursor = 0usize;

        for byte in s.bytes() {
            let ch = if byte == b'\n' { b'\r' } else { byte };
            utf16[cursor] = ch as u16;
            cursor += 1;

            if byte == b'\n' {
                utf16[cursor] = b'\n' as u16;
                cursor += 1;
            }

            if cursor >= utf16.len() - 1 {
                utf16[cursor] = 0;
                unsafe {
                    ((*con_out).output_string)(con_out, utf16.as_ptr());
                }
                cursor = 0;
            }
        }

        if cursor != 0 {
            utf16[cursor] = 0;
            unsafe {
                ((*con_out).output_string)(con_out, utf16.as_ptr());
            }
        }

        Ok(())
    }
}

struct DebugCon;

impl DebugCon {
    fn write_byte(&mut self, byte: u8) {
        unsafe {
            asm!(
                "out dx, al",
                in("dx") 0xe9u16,
                in("al") byte,
                options(nomem, nostack, preserves_flags)
            );
        }
    }
}

impl Write for DebugCon {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
        Ok(())
    }
}
