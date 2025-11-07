#![no_std]

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

/// Magic addresses for test pass/fail signaling
/// Tests jump to these addresses to indicate success or failure.
/// The emulator detects when PC reaches these unmapped addresses.
/// Using addresses in unmapped memory space (0xF000_0000 region)
pub const TEST_PASS_ADDR: u32 = 0xF0000000;
pub const TEST_FAIL_ADDR: u32 = 0xF0000004;

/// Signal test passed by jumping to magic address
#[unsafe(naked)]
pub extern "C" fn test_pass() -> ! {
    core::arch::naked_asm!(
        "ldr r0, ={addr}",
        "bx r0",
        addr = const TEST_PASS_ADDR,
    )
}

/// Signal test failed by jumping to magic address
#[unsafe(naked)]
pub extern "C" fn test_fail() -> ! {
    core::arch::naked_asm!(
        "ldr r0, ={addr}",
        "bx r0",
        addr = const TEST_FAIL_ADDR,
    )
}

// Symbols from linker script
unsafe extern "C" {
    static mut __sbss: u32;
    static mut __ebss: u32;
    static mut __sdata: u32;
    static mut __edata: u32;
    static __sidata: u32;
    static _stack_start: u32;
}

/// ARM11 runtime entry point
///
/// This function performs:
/// 1. Stack pointer initialization
/// 2. BSS section zeroing
/// 3. Data section initialization (copy from load address to runtime address)
/// 4. VFP (floating point) initialization
/// 5. Call to user's main function
///
/// Test binaries should define a `main` function with signature `fn() -> !`
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        // 1. Set stack pointer
        "ldr sp, =_stack_start",
        // 2. Zero BSS section
        "ldr r0, =__sbss",
        "ldr r1, =__ebss",
        "mov r2, #0",
        "0:",
        "cmp r0, r1",
        "bge 1f",
        "str r2, [r0], #4",
        "b 0b",
        "1:",
        // 3. Copy .data section (if LMA != VMA)
        "ldr r0, =__sdata",
        "ldr r1, =__edata",
        "ldr r2, =__sidata",
        "cmp r0, r2",
        "beq 3f", // Skip if same address (pure RAM execution)
        "2:",
        "cmp r0, r1",
        "bge 3f",
        "ldr r3, [r2], #4",
        "str r3, [r0], #4",
        "b 2b",
        "3:",
        // 4. Enable VFP (ARM11 VFPv2)
        // Enable CP10/CP11 access in CPACR
        "mrc p15, 0, r0, c1, c0, 2",
        "orr r0, r0, #(0xF << 20)", // Full access for CP10/CP11
        "mcr p15, 0, r0, c1, c0, 2",
        "isb",
        // Enable VFP via FPEXC
        "mov r0, #(1 << 30)", // FPEXC.EN bit
        "vmsr fpexc, r0",
        // 5. Call main
        "bl main",
        // 6. Loop forever if main returns (shouldn't happen)
        "b .",
    )
}
