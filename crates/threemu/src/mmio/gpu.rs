//! GPU MMIO register handling for 3DS emulation.
//!
//! This module implements the GPU register interface for the ARM11 processor.
//! The GPU is mapped at 0x10400000-0x10500000 and handles framebuffer configuration.
//!
//! # References
//! - [GPU External Registers](https://www.3dbrew.org/wiki/GPU/External_Registers)
//! - [LCD Registers](https://www.3dbrew.org/wiki/LCD)
//!
//! # Framebuffer Format
//! The 3DS framebuffers have an unusual orientation: pixels are stored left-to-right
//! (as if the screen is rotated 90° clockwise). This means for a 400×240 screen, the
//! framebuffer is actually stored as 240 columns of 400 pixels each.

use oxidiz3ds_hw::mmio::gpu::registers as hw_regs;
use tracing::{debug, instrument, trace, warn};
use unicorn_engine::Unicorn;

/// Pixel format for framebuffers.
///
/// These correspond to the values in bits 0-2 of the format register.
/// Reference: https://www.3dbrew.org/wiki/GPU/External_Registers#Framebuffer_format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PixelFormat {
    /// 32-bit RGBA (8 bits per component)
    Rgba8 = 0,
    /// 24-bit RGB (8 bits per component)
    Rgb8 = 1,
    /// 16-bit RGB (5-6-5)
    Rgb565 = 2,
    /// 16-bit RGB with 1-bit alpha (5-5-5-1)
    Rgb5A1 = 3,
    /// 16-bit RGBA (4 bits per component)
    Rgba4 = 4,
    /// Unknown or unsupported format
    Unknown = 0xFF,
}

impl From<u32> for PixelFormat {
    fn from(value: u32) -> Self {
        match value & 0x7 {
            0 => PixelFormat::Rgba8,
            1 => PixelFormat::Rgb8,
            2 => PixelFormat::Rgb565,
            3 => PixelFormat::Rgb5A1,
            4 => PixelFormat::Rgba4,
            _ => PixelFormat::Unknown,
        }
    }
}

/// GPU state tracking framebuffer configuration
#[derive(Debug)]
pub struct GpuState {
    // Top screen (can have two framebuffers for 3D)
    pub top_left_addr: u32,
    pub top_right_addr: u32,
    pub top_format: PixelFormat,
    pub top_stride: u32,

    // Bottom screen
    pub bottom_addr: u32,
    pub bottom_format: PixelFormat,
    pub bottom_stride: u32,
}

impl GpuState {
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            top_left_addr: 0,
            top_right_addr: 0,
            top_format: PixelFormat::Unknown,
            top_stride: 0,
            bottom_addr: 0,
            bottom_format: PixelFormat::Unknown,
            bottom_stride: 0,
        }
    }

    /// Handle a write to a GPU register
    pub fn write(&mut self, offset: u32, _size: usize, value: u32) {
        trace!(
            "GPU register write: offset={:#X}, value={:#X}",
            offset, value
        );

        match offset {
            hw_regs::FRAMEBUFFER_TOP_LEFT => {
                self.top_left_addr = value;
                debug!("Top screen left framebuffer: {:#X}", self.top_left_addr);
            }
            hw_regs::FRAMEBUFFER_TOP_RIGHT => {
                self.top_right_addr = value;
                debug!("Top screen right framebuffer: {:#X}", self.top_right_addr);
            }
            hw_regs::FRAMEBUFFER_TOP_FORMAT => {
                self.top_format = PixelFormat::from(value);
                debug!("Top screen format: {:?}", self.top_format);
            }
            hw_regs::FRAMEBUFFER_TOP_STRIDE => {
                self.top_stride = value;
                debug!("Top screen stride: {:#X}", self.top_stride);
            }
            hw_regs::FRAMEBUFFER_BOTTOM_LEFT => {
                self.bottom_addr = value;
                debug!("Bottom screen framebuffer: {:#X}", self.bottom_addr);
            }
            hw_regs::FRAMEBUFFER_BOTTOM_FORMAT => {
                self.bottom_format = PixelFormat::from(value);
                debug!("Bottom screen format: {:?}", self.bottom_format);
            }
            hw_regs::FRAMEBUFFER_BOTTOM_STRIDE => {
                self.bottom_stride = value;
                debug!("Bottom screen stride: {:#X}", self.bottom_stride);
            }
            _ => {
                // Unknown register - log at warn level
                warn!(
                    "Unknown GPU register write: offset={:#X}, value={:#X}",
                    offset, value
                );
            }
        }
    }

    /// Handle a read from a GPU register
    pub fn read(&self, offset: u32, _size: usize) -> u32 {
        trace!("GPU register read: offset={:#X}", offset);

        match offset {
            hw_regs::FRAMEBUFFER_TOP_LEFT => self.top_left_addr,
            hw_regs::FRAMEBUFFER_TOP_RIGHT => self.top_right_addr,
            hw_regs::FRAMEBUFFER_TOP_FORMAT => self.top_format as u32,
            hw_regs::FRAMEBUFFER_TOP_STRIDE => self.top_stride,
            hw_regs::FRAMEBUFFER_BOTTOM_LEFT => self.bottom_addr,
            hw_regs::FRAMEBUFFER_BOTTOM_FORMAT => self.bottom_format as u32,
            hw_regs::FRAMEBUFFER_BOTTOM_STRIDE => self.bottom_stride,
            _ => {
                warn!("Unknown GPU register read: offset={:#X}", offset);
                0
            }
        }
    }
}

// ============================================================================
// Unicorn MMIO Adapters
// ============================================================================

/// MMIO read handler function (for use with Unicorn)
///
/// This is a thin adapter that converts Unicorn's u64 addresses to the u32
/// offsets expected by the GPU handler.
#[instrument(level = "trace", skip(uc))]
pub fn read_handler(uc: &mut Unicorn<'_, super::EmulatorState>, addr: u64, size: usize) -> u64 {
    uc.get_data_mut().gpu.read(addr as u32, size) as u64
}

/// MMIO write handler function (for use with Unicorn)
///
/// This is a thin adapter that converts Unicorn's u64 addresses and values to the u32
/// types expected by the GPU handler.
#[instrument(level = "trace", skip(uc))]
pub fn write_handler(
    uc: &mut Unicorn<'_, super::EmulatorState>,
    addr: u64,
    size: usize,
    value: u64,
) {
    uc.get_data_mut().gpu.write(addr as u32, size, value as u32);
}
