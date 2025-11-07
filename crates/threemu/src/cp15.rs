//! ARM9 CP15 Coprocessor Emulation
//!
//! This module provides emulation for ARM9 CP15 (System Control Coprocessor) instructions.
//! CP15 controls various system features including:
//! - TCM (Tightly Coupled Memory) configuration
//! - MMU (Memory Management Unit) control
//! - Cache control
//! - System control register
//!
//! Currently implemented:
//! - TCM region configuration (c9, c1, 0/1)
//! - Control register TCM enable bits (c1, c0, 0)
//!
//! # References
//! - [ARM946E-S Technical Reference Manual](https://developer.arm.com/documentation/ddi0201/latest/)
//! - [GBATEK ARM CP15 Documentation](https://problemkaputt.de/gbatek.htm#armcp15systemcontrolcoprocessor)

use tracing::{debug, warn};
use unicorn_engine::{RegisterARM, Unicorn};

/// CP15 coprocessor instruction mask
const CP15_MASK: u32 = 0x0F000000;

/// CP15 coprocessor instruction value
const CP15_VALUE: u32 = 0x0E000000;

/// CP15 coprocessor register mask
const CP15_REG_MASK: u32 = 0x00000F00;

/// CP15 coprocessor register value
const CP15_REG_VALUE: u32 = 0x00000F00;

/// ARM instruction size in bytes
const ARM_INSN_SIZE: u64 = 4;

/// Handles CP15 coprocessor instructions for ARM9
///
/// This function is called from a code hook and processes CP15 instructions.
/// It returns true if a CP15 instruction was handled, false otherwise.
///
/// # Supported Instructions
///
/// - `MCR p15, 0, Rd, c9, c1, 0` - Configure DTCM region
/// - `MCR p15, 0, Rd, c9, c1, 1` - Configure ITCM region
/// - `MCR p15, 0, Rd, c1, c0, 0` - Control register (TCM enable bits)
///
/// All other CP15 instructions are logged as warnings and skipped.
pub fn handle_cp15_instruction<D>(uc: &mut Unicorn<D>, addr: u64, insn: u32) -> bool {
    // Check if it's a CP15 instruction
    let is_cp15 = (insn & CP15_MASK) == CP15_VALUE && (insn & CP15_REG_MASK) == CP15_REG_VALUE;

    if !is_cp15 {
        return false;
    }

    // Decode CP15 instruction fields
    let is_mcr = (insn & (1 << 20)) == 0; // Bit 20 = 0 for MCR, 1 for MRC
    let crn = (insn >> 16) & 0xF; // CRn (coprocessor register)
    let crm = insn & 0xF; // CRm (coprocessor register modifier)
    let opc2 = (insn >> 5) & 0x7; // opcode 2
    let rd = (insn >> 12) & 0xF; // ARM register (source for MCR, dest for MRC)

    // Handle different CP15 registers
    if is_mcr && crn == 9 && crm == 1 && (opc2 == 0 || opc2 == 1) {
        // TCM Region Configuration: MCR p15, 0, Rd, c9, c1, {0,1}
        handle_tcm_region_config(uc, addr, rd, opc2);
    } else if is_mcr && crn == 1 && crm == 0 && opc2 == 0 {
        // Control Register: MCR p15, 0, Rd, c1, c0, 0
        handle_control_register(uc, rd);
    } else {
        // Unsupported CP15 instruction - log and skip
        let op = if is_mcr { "MCR" } else { "MRC" };
        warn!(
            "Unsupported CP15 instruction at {:#X}: {} p15, 0, r{}, c{}, c{}, {} (skipping)",
            addr, op, rd, crn, crm, opc2
        );
    }

    // Skip the CP15 instruction by advancing PC
    let _ = uc.reg_write(RegisterARM::PC, addr + ARM_INSN_SIZE);

    true
}

/// Handles TCM region configuration (c9, c1, 0/1)
///
/// Configures DTCM (opc2=0) or ITCM (opc2=1) region base address and size.
///
/// # Register Format
///
/// - Bits [31:12]: Base address (4KB aligned)
/// - Bits [11:6]: Reserved (should be zero)
/// - Bits [5:1]: Size encoding (size = 512 << size_bits)
/// - Bit [0]: Region enable (historically used, but c1 control bits take priority)
///
/// # Notes
///
/// On real hardware, you can configure TCM regions while they're disabled via
/// the control register. The region is mapped immediately in our emulator,
/// regardless of the region enable bit, to match this behavior.
fn handle_tcm_region_config<D>(uc: &mut Unicorn<D>, addr: u64, rd: u32, opc2: u32) {
    use unicorn_engine::Prot;

    // Read the register value
    let reg_val = read_arm_register(uc, rd);

    // Parse TCM region register format
    let base_addr = reg_val & 0xFFFFF000; // Bits [31:12]
    let size_bits = (reg_val >> 1) & 0x1F; // Bits [5:1]
    let region_enable = (reg_val & 1) == 1; // Bit [0]

    // Calculate size from encoding: size = 512 << size_bits
    let size = 512u32 << size_bits;
    let tcm_type = if opc2 == 0 { "DTCM" } else { "ITCM" };

    debug!(
        "CP15 {:#X}: Configuring {} at {:#X}, size {}KB, region_enable={} (mapping now, will be enabled via c1)",
        addr,
        tcm_type,
        base_addr,
        size / 1024,
        region_enable
    );

    // Map the memory region regardless of the region enable bit
    // The control register (c1, c0, 0) bits 16/18 control actual TCM access
    // This matches real hardware behavior where you can configure disabled regions
    if let Err(e) = uc.mem_map(base_addr as u64, size as u64, Prot::ALL) {
        debug!(
            "CP15 {:#X}: Failed to map {}: {:?} (may already be mapped)",
            addr, tcm_type, e
        );
    }
}

/// Handles control register writes (c1, c0, 0)
///
/// The control register contains various system control bits. We currently
/// track the TCM enable bits:
///
/// - Bit 16: DTCM enable
/// - Bit 18: ITCM enable
///
/// # Notes
///
/// Since we map TCM regions when they're configured via c9, this handler
/// currently just logs the enable state. In the future, we could track
/// the control register state for more accurate emulation.
fn handle_control_register<D>(uc: &Unicorn<D>, rd: u32) {
    // Read the register value
    let reg_val = read_arm_register(uc, rd);

    let dtcm_enable = (reg_val & 0x10000) != 0; // Bit 16
    let itcm_enable = (reg_val & 0x40000) != 0; // Bit 18

    debug!(
        "CP15: Control Register update - DTCM enable: {}, ITCM enable: {} (supported)",
        dtcm_enable, itcm_enable
    );
}

/// Reads an ARM general-purpose register (R0-R12)
///
/// Returns 0 for invalid register numbers (>12).
fn read_arm_register<D>(uc: &Unicorn<D>, rd: u32) -> u32 {
    match rd {
        0 => uc.reg_read(RegisterARM::R0).unwrap_or(0) as u32,
        1 => uc.reg_read(RegisterARM::R1).unwrap_or(0) as u32,
        2 => uc.reg_read(RegisterARM::R2).unwrap_or(0) as u32,
        3 => uc.reg_read(RegisterARM::R3).unwrap_or(0) as u32,
        4 => uc.reg_read(RegisterARM::R4).unwrap_or(0) as u32,
        5 => uc.reg_read(RegisterARM::R5).unwrap_or(0) as u32,
        6 => uc.reg_read(RegisterARM::R6).unwrap_or(0) as u32,
        7 => uc.reg_read(RegisterARM::R7).unwrap_or(0) as u32,
        8 => uc.reg_read(RegisterARM::R8).unwrap_or(0) as u32,
        9 => uc.reg_read(RegisterARM::R9).unwrap_or(0) as u32,
        10 => uc.reg_read(RegisterARM::R10).unwrap_or(0) as u32,
        11 => uc.reg_read(RegisterARM::R11).unwrap_or(0) as u32,
        12 => uc.reg_read(RegisterARM::R12).unwrap_or(0) as u32,
        _ => 0,
    }
}
