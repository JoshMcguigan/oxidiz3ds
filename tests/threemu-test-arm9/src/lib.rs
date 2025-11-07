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

/// ARM9 runtime entry point
///
/// This function performs:
/// 1. Stack pointer initialization
/// 2. BSS section zeroing
/// 3. Data section initialization (copy from load address to runtime address)
/// 4. Call to user's main function
///
/// Note: ARM9 (ARM946E-S) is ARMv5TE and does not have VFP/FPU.
/// This code is written for Thumb mode (thumbv5te-none-eabi target).
///
/// Test binaries should define a `main` function with signature `fn() -> !`
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        // 1. Set stack pointer (Thumb: load into r0 then mov to sp)
        "ldr r0, =_stack_start",
        "mov sp, r0",
        // 2. Zero BSS section
        "ldr r0, =__sbss",
        "ldr r1, =__ebss",
        "movs r2, #0",
        "0:",
        "cmp r0, r1",
        "bge 1f",
        "stmia r0!, {{r2}}", // Store and increment (Thumb-compatible)
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
        "ldmia r2!, {{r3}}", // Load and increment (Thumb-compatible)
        "stmia r0!, {{r3}}", // Store and increment
        "b 2b",
        "3:",
        // 4. Call main
        "bl main",
        // 5. Loop forever if main returns (shouldn't happen)
        "b .",
    )
}
