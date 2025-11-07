//! Memory-Mapped I/O (MMIO) handling for 3DS emulation.
//!
//! This module provides handlers for different MMIO regions on the 3DS hardware.
//! Each region has specific functionality (GPU, generic I/O, etc.) and may be
//! accessible to different processors (ARM9, ARM11, or both).
//!
//! # Memory Map
//! According to [3DBrew IO Registers](https://www.3dbrew.org/wiki/IO_Registers):
//! - `0x10000000-0x10400000`: Generic MMIO (both ARM9 and ARM11)
//! - `0x10400000-0x10500000`: GPU registers (ARM11 only)
//! - `0x10500000-0x18000000`: Additional MMIO regions
//! - `0x18000000-0x18600000`: VRAM (6MB, both ARM9 and ARM11)
//! - `0x18600000-0x1FF80000`: More MMIO regions

use std::path::PathBuf;

pub mod generic;
pub mod gpu;
pub mod sdmmc;

// Re-export types for convenience
pub use gpu::{GpuState, PixelFormat};
pub use sdmmc::SdmmcState;

/// Shared emulator state accessible from MMIO callbacks and main loop
#[derive(Debug)]
pub struct EmulatorState {
    pub gpu: GpuState,
    pub sdmmc: SdmmcState,
}

impl EmulatorState {
    pub fn new(sd_card_path: Option<PathBuf>) -> Self {
        Self {
            gpu: GpuState::new(),
            sdmmc: SdmmcState::new(sd_card_path),
        }
    }
}
