/// CPU frequency specifications
pub mod cpu {
    /// ARM9 CPU frequency (134 MHz)
    pub const ARM9_HZ: u64 = 134_000_000;

    /// ARM11 CPU frequency (268 MHz)
    ///
    /// Note: Can be overclocked to 804 MHz on New 3DS
    pub const ARM11_HZ: u64 = 268_000_000;

    /// ARM11 CPU frequency on New 3DS when overclocked (804 MHz)
    pub const ARM11_HZ_NEW3DS: u64 = 804_000_000;
}

/// Display specifications
pub mod display {
    /// Top screen width in pixels
    pub const TOP_WIDTH: u32 = 400;

    /// Top screen height in pixels
    pub const TOP_HEIGHT: u32 = 240;

    /// Bottom screen width in pixels
    pub const BOTTOM_WIDTH: u32 = 320;

    /// Bottom screen height in pixels
    pub const BOTTOM_HEIGHT: u32 = 240;

    /// Display refresh rate (60 Hz)
    pub const REFRESH_RATE_HZ: u32 = 60;

    /// Framebuffer bytes per pixel for RGBA8 format
    pub const BYTES_PER_PIXEL_RGBA8: usize = 4;

    /// Framebuffer bytes per pixel for RGB8 format
    pub const BYTES_PER_PIXEL_RGB8: usize = 3;

    /// Framebuffer bytes per pixel for RGB565/RGB5A1/RGBA4 formats
    pub const BYTES_PER_PIXEL_16BIT: usize = 2;
}
