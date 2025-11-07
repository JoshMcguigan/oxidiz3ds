//! Memory setup utilities for 3DS emulation.
//!
//! This module provides functions for setting up memory maps for both ARM9 and ARM11
//! processors, as well as loading FIRM sections into memory.

use crate::firm::FirmSectionHeader;
use crate::mmio;
use oxidiz3ds_hw::{memory_map, mmio as hw_mmio};
use tracing::debug;
use unicorn_engine::{Unicorn, unicorn_const::Prot};

// Memory constants from hardware definitions
pub const FCRAM_BASE: u32 = memory_map::fcram::BASE;
pub const FCRAM_SIZE: usize = memory_map::fcram::SIZE;
pub const AXI_WRAM_BASE: u32 = memory_map::axi_wram::BASE;
pub const AXI_WRAM_SIZE: usize = memory_map::axi_wram::SIZE;
pub const VRAM_BASE: u32 = memory_map::vram::BASE;
pub const VRAM_SIZE: usize = memory_map::vram::SIZE;
pub const ARM9_ITCM_BASE: u32 = memory_map::arm9::itcm::BASE;
pub const ARM9_ITCM_SIZE: usize = memory_map::arm9::itcm::SIZE;
pub const ARM9_PRIVATE_WRAM_BASE: u32 = memory_map::arm9::private_wram::BASE;
pub const ARM9_PRIVATE_WRAM_SIZE: usize = memory_map::arm9::private_wram::SIZE;

// MMIO region constants
const MMIO_REGION1_BASE: u32 = memory_map::mmio::region1::BASE;
const MMIO_REGION1_END: u32 = memory_map::mmio::region1::END;
const MMIO_REGION2_BASE: u32 = memory_map::mmio::region2::BASE;
const MMIO_REGION2_END: u32 = memory_map::mmio::region2::END;
const SDMMC_MMIO_BASE: u32 = hw_mmio::sdmmc::BASE;
const SDMMC_MMIO_END: u32 = hw_mmio::sdmmc::END;
const GPU_MMIO_BASE: u32 = hw_mmio::gpu::BASE;
const GPU_MMIO_END: u32 = hw_mmio::gpu::END;
const ARM11_MMIO_SPLIT: u32 = memory_map::mmio::ARM11_MMIO_SPLIT;

/// Set up memory map for ARM9
pub fn setup_arm9_memory(
    emu: &mut Unicorn<mmio::EmulatorState>,
    fcram: &mut [u8],
    axi_wram: &mut [u8],
    vram: &mut [u8],
    arm9_private_wram: &mut [u8],
) {
    // Shared memory regions
    debug!(
        "  Mapping shared FCRAM at {:#X} ({}MB)",
        FCRAM_BASE,
        FCRAM_SIZE / (1024 * 1024)
    );
    unsafe {
        emu.mem_map_ptr(
            FCRAM_BASE as u64,
            FCRAM_SIZE as u64,
            Prot::ALL,
            fcram.as_mut_ptr() as _,
        )
        .expect("failed to map FCRAM");
    }

    debug!(
        "  Mapping shared AXI WRAM at {:#X} ({}KB)",
        AXI_WRAM_BASE,
        AXI_WRAM_SIZE / 1024
    );
    unsafe {
        emu.mem_map_ptr(
            AXI_WRAM_BASE as u64,
            AXI_WRAM_SIZE as u64,
            Prot::ALL,
            axi_wram.as_mut_ptr() as _,
        )
        .expect("failed to map AXI WRAM");
    }

    debug!(
        "  Mapping shared VRAM at {:#X} ({}MB)",
        VRAM_BASE,
        VRAM_SIZE / (1024 * 1024)
    );
    unsafe {
        emu.mem_map_ptr(
            VRAM_BASE as u64,
            VRAM_SIZE as u64,
            Prot::ALL,
            vram.as_mut_ptr() as _,
        )
        .expect("failed to map VRAM");
    }

    // ARM9-specific internal memory
    debug!(
        "  Mapping ARM9 internal memory at {:#X} ({}MB)",
        ARM9_ITCM_BASE,
        ARM9_ITCM_SIZE / (1024 * 1024)
    );
    emu.mem_map(ARM9_ITCM_BASE as u64, ARM9_ITCM_SIZE as u64, Prot::ALL)
        .expect("failed to map ARM9 internal memory");

    // ARM9-specific private WRAM
    debug!(
        "  Mapping ARM9 private WRAM at {:#X} ({}KB)",
        ARM9_PRIVATE_WRAM_BASE,
        ARM9_PRIVATE_WRAM_SIZE / 1024
    );
    unsafe {
        emu.mem_map_ptr(
            ARM9_PRIVATE_WRAM_BASE as u64,
            ARM9_PRIVATE_WRAM_SIZE as u64,
            Prot::ALL,
            arm9_private_wram.as_mut_ptr() as _,
        )
        .expect("failed to map ARM9 private WRAM");
    }

    // Generic MMIO regions (split around VRAM and SDMMC)
    debug!(
        "  Mapping generic MMIO region {:#X} - {:#X}",
        MMIO_REGION1_BASE, SDMMC_MMIO_BASE
    );
    emu.mmio_map(
        MMIO_REGION1_BASE as u64,
        (SDMMC_MMIO_BASE - MMIO_REGION1_BASE) as u64,
        Some(mmio::generic::read_handler),
        Some(mmio::generic::write_handler),
    )
    .expect("failed to map generic MMIO region");

    debug!(
        "  Mapping SDMMC MMIO region {:#X} - {:#X}",
        SDMMC_MMIO_BASE, SDMMC_MMIO_END
    );
    emu.mmio_map(
        SDMMC_MMIO_BASE as u64,
        (SDMMC_MMIO_END - SDMMC_MMIO_BASE) as u64,
        Some(mmio::sdmmc::read_handler),
        Some(mmio::sdmmc::write_handler),
    )
    .expect("failed to map SDMMC MMIO region");

    debug!(
        "  Intentionally leaving {:#X} - {:#X} unmapped (unused region)",
        SDMMC_MMIO_END,
        SDMMC_MMIO_END + 0x1000
    );

    debug!(
        "  Mapping generic MMIO region {:#X} - {:#X}",
        SDMMC_MMIO_END + 0x1000,
        MMIO_REGION1_END
    );
    emu.mmio_map(
        (SDMMC_MMIO_END + 0x1000) as u64,
        (MMIO_REGION1_END - (SDMMC_MMIO_END + 0x1000)) as u64,
        Some(mmio::generic::read_handler),
        Some(mmio::generic::write_handler),
    )
    .expect("failed to map generic MMIO region");

    debug!(
        "  Mapping generic MMIO region {:#X} - {:#X}",
        MMIO_REGION2_BASE, MMIO_REGION2_END
    );
    emu.mmio_map(
        MMIO_REGION2_BASE as u64,
        (MMIO_REGION2_END - MMIO_REGION2_BASE) as u64,
        Some(mmio::generic::read_handler),
        Some(mmio::generic::write_handler),
    )
    .expect("failed to map MMIO region");
}

/// Set up memory map for ARM11
pub fn setup_arm11_memory(
    emu: &mut Unicorn<mmio::EmulatorState>,
    fcram: &mut [u8],
    axi_wram: &mut [u8],
    vram: &mut [u8],
) {
    // Shared memory regions
    debug!(
        "  Mapping shared FCRAM at {:#X} ({}MB)",
        FCRAM_BASE,
        FCRAM_SIZE / (1024 * 1024)
    );
    unsafe {
        emu.mem_map_ptr(
            FCRAM_BASE as u64,
            FCRAM_SIZE as u64,
            Prot::ALL,
            fcram.as_mut_ptr() as _,
        )
        .expect("failed to map FCRAM");
    }

    debug!(
        "  Mapping shared AXI WRAM at {:#X} ({}KB)",
        AXI_WRAM_BASE,
        AXI_WRAM_SIZE / 1024
    );
    unsafe {
        emu.mem_map_ptr(
            AXI_WRAM_BASE as u64,
            AXI_WRAM_SIZE as u64,
            Prot::ALL,
            axi_wram.as_mut_ptr() as _,
        )
        .expect("failed to map AXI WRAM");
    }

    debug!(
        "  Mapping shared VRAM at {:#X} ({}MB)",
        VRAM_BASE,
        VRAM_SIZE / (1024 * 1024)
    );
    unsafe {
        emu.mem_map_ptr(
            VRAM_BASE as u64,
            VRAM_SIZE as u64,
            Prot::ALL,
            vram.as_mut_ptr() as _,
        )
        .expect("failed to map VRAM");
    }

    // MMIO regions with separate handlers
    debug!(
        "  Mapping generic MMIO region {:#X} - {:#X}",
        MMIO_REGION1_BASE, SDMMC_MMIO_BASE
    );
    emu.mmio_map(
        MMIO_REGION1_BASE as u64,
        (SDMMC_MMIO_BASE - MMIO_REGION1_BASE) as u64,
        Some(mmio::generic::read_handler),
        Some(mmio::generic::write_handler),
    )
    .expect("failed to map generic MMIO region");

    debug!(
        "  Mapping SDMMC MMIO region {:#X} - {:#X}",
        SDMMC_MMIO_BASE, SDMMC_MMIO_END
    );
    emu.mmio_map(
        SDMMC_MMIO_BASE as u64,
        (SDMMC_MMIO_END - SDMMC_MMIO_BASE) as u64,
        Some(mmio::sdmmc::read_handler),
        Some(mmio::sdmmc::write_handler),
    )
    .expect("failed to map SDMMC MMIO region");

    debug!(
        "  Intentionally leaving {:#X} - {:#X} unmapped (unused region)",
        SDMMC_MMIO_END,
        SDMMC_MMIO_END + 0x1000
    );

    debug!(
        "  Mapping generic MMIO region {:#X} - {:#X}",
        SDMMC_MMIO_END + 0x1000,
        GPU_MMIO_BASE
    );
    emu.mmio_map(
        (SDMMC_MMIO_END + 0x1000) as u64,
        (GPU_MMIO_BASE - (SDMMC_MMIO_END + 0x1000)) as u64,
        Some(mmio::generic::read_handler),
        Some(mmio::generic::write_handler),
    )
    .expect("failed to map generic MMIO region");

    debug!(
        "  Mapping GPU MMIO region {:#X} - {:#X} (ARM11 only)",
        GPU_MMIO_BASE, GPU_MMIO_END
    );
    emu.mmio_map(
        GPU_MMIO_BASE as u64,
        (GPU_MMIO_END - GPU_MMIO_BASE) as u64,
        Some(mmio::gpu::read_handler),
        Some(mmio::gpu::write_handler),
    )
    .expect("failed to map GPU MMIO region");

    debug!(
        "  Mapping remaining MMIO region {:#X} - {:#X}",
        GPU_MMIO_END, ARM11_MMIO_SPLIT
    );
    emu.mmio_map(
        GPU_MMIO_END as u64,
        (ARM11_MMIO_SPLIT - GPU_MMIO_END) as u64,
        Some(mmio::generic::read_handler),
        Some(mmio::generic::write_handler),
    )
    .expect("failed to map remaining MMIO region");

    debug!(
        "  Mapping final MMIO region {:#X} - {:#X}",
        MMIO_REGION2_BASE, MMIO_REGION2_END
    );
    emu.mmio_map(
        MMIO_REGION2_BASE as u64,
        (MMIO_REGION2_END - MMIO_REGION2_BASE) as u64,
        Some(mmio::generic::read_handler),
        Some(mmio::generic::write_handler),
    )
    .expect("failed to map final MMIO region");
}

/// Check if an address is in ARM9-specific memory
pub fn is_arm9_memory(addr: u32) -> bool {
    // ARM9 internal memory
    (ARM9_ITCM_BASE..(ARM9_ITCM_BASE + ARM9_ITCM_SIZE as u32)).contains(&addr)
}

/// Load FIRM sections into emulator
pub fn load_sections(
    emu: &mut Unicorn<mmio::EmulatorState>,
    sections: &[FirmSectionHeader],
    firm_data: &[u8],
    is_arm9: bool,
) {
    for (i, section) in sections.iter().enumerate() {
        if section.size == 0 {
            continue;
        }

        let addr = section.load_address;

        // Skip ARM9-specific sections if this is ARM11, and vice versa
        if is_arm9_memory(addr) != is_arm9 {
            debug!(
                "  Section {}: addr={:#X}, size={:#X} - skipping (wrong processor)",
                i, addr, section.size
            );
            continue;
        }

        debug!(
            "  Section {}: addr={:#X}, size={:#X}, offset={:#X}",
            i, addr, section.size, section.offset
        );

        // Copy section data - let Unicorn figure out which backing memory it goes to
        let section_start = section.offset as usize;
        let section_end = section_start + section.size as usize;
        let section_data = &firm_data[section_start..section_end];

        emu.mem_write(addr as u64, section_data)
            .expect("failed to write section data");
    }
}
