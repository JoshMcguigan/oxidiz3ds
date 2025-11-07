//! # References
//! - <https://www.3dbrew.org/wiki/GPU/External_Registers>
//! - <https://www.3dbrew.org/wiki/LCD>

/// GPU MMIO region base address (ARM11 only)
pub const BASE: u32 = 0x10400000;

/// GPU MMIO region end address (exclusive)
pub const END: u32 = 0x10500000;

/// GPU register offsets (relative to `BASE`)
pub mod registers {
    /// Top screen left framebuffer address register
    ///
    /// Reference: <https://www.3dbrew.org/wiki/GPU/External_Registers#Framebuffers>
    pub const FRAMEBUFFER_TOP_LEFT: u32 = 0x468;

    /// Top screen framebuffer pixel format register
    pub const FRAMEBUFFER_TOP_FORMAT: u32 = 0x470;

    /// Top screen framebuffer stride (bytes per row) register
    pub const FRAMEBUFFER_TOP_STRIDE: u32 = 0x490;

    /// Top screen right framebuffer address register (for 3D mode)
    pub const FRAMEBUFFER_TOP_RIGHT: u32 = 0x494;

    /// Bottom screen framebuffer address register
    pub const FRAMEBUFFER_BOTTOM_LEFT: u32 = 0x568;

    /// Bottom screen framebuffer pixel format register
    pub const FRAMEBUFFER_BOTTOM_FORMAT: u32 = 0x570;

    /// Bottom screen framebuffer stride register
    pub const FRAMEBUFFER_BOTTOM_STRIDE: u32 = 0x590;
}

/// Pixel format values for framebuffer format registers
///
/// These correspond to bits 0-2 of the format register.
///
/// Reference: <https://www.3dbrew.org/wiki/GPU/External_Registers#Framebuffer_format>
pub mod pixel_format {
    /// RGBA8 (32 bits per pixel)
    pub const RGBA8: u32 = 0;
    /// RGB8 (24 bits per pixel)
    pub const RGB8: u32 = 1;
    /// RGB565 (16 bits per pixel)
    pub const RGB565: u32 = 2;
    /// RGB5A1 (16 bits per pixel)
    pub const RGB5A1: u32 = 3;
    /// RGBA4 (16 bits per pixel)
    pub const RGBA4: u32 = 4;
}
