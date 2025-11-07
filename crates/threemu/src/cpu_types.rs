//! CPU-related type definitions
//!
//! This module contains types related to CPU emulation that are used
//! throughout the emulator.

/// ARM general-purpose and special registers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmRegister {
    R0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13, // Stack Pointer (SP)
    R14, // Link Register (LR)
    R15, // Program Counter (PC)
    CPSR,
}
