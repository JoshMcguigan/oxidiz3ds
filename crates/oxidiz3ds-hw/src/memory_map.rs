//! # References
//! - <https://www.3dbrew.org/wiki/Memory_layout>

/// FCRAM (Fast Cycle RAM) - Main system memory shared between ARM9 and ARM11
///
/// Reference: <https://www.3dbrew.org/wiki/Memory_layout#FCRAM>
pub mod fcram {
    /// FCRAM base address
    pub const BASE: u32 = 0x20000000;
    /// FCRAM size (128 MB)
    pub const SIZE: usize = 128 * 1024 * 1024;
}

/// AXI WRAM - Shared WRAM between ARM9 and ARM11
///
/// Reference: <https://www.3dbrew.org/wiki/Memory_layout#AXI_WRAM>
pub mod axi_wram {
    /// AXI WRAM base address
    pub const BASE: u32 = 0x1FF80000;
    /// AXI WRAM size (512 KB)
    pub const SIZE: usize = 512 * 1024;
}

/// VRAM (Video RAM) - Shared between ARM9 and ARM11
///
/// Reference: <https://www.3dbrew.org/wiki/Memory_layout#VRAM>
pub mod vram {
    /// VRAM base address
    pub const BASE: u32 = 0x18000000;
    /// VRAM size (6 MB)
    pub const SIZE: usize = 6 * 1024 * 1024;
}

/// ARM9-specific memory regions
pub mod arm9 {
    /// ARM9 ITCM (Instruction Tightly Coupled Memory)
    ///
    /// Reference: <https://www.3dbrew.org/wiki/Memory_layout#ARM9>
    pub mod itcm {
        /// ITCM base address
        pub const BASE: u32 = 0x08000000;
        /// ITCM size (2 MB)
        pub const SIZE: usize = 2 * 1024 * 1024;
    }

    /// ARM9 Private WRAM (separate from shared AXI WRAM)
    ///
    /// Reference: <https://www.3dbrew.org/wiki/Memory_layout#ARM9>
    pub mod private_wram {
        /// Private WRAM base address
        pub const BASE: u32 = 0x01FF8000;
        /// Private WRAM size (32 KB)
        pub const SIZE: usize = 32 * 1024;
    }

    /// ARM9 Bootrom region
    pub mod bootrom {
        /// Bootrom base address
        pub const BASE: u32 = 0xFFFF0000;
        /// Bootrom size (64 KB)
        pub const SIZE: usize = 64 * 1024;
    }
}

/// MMIO (Memory-Mapped I/O) region boundaries
pub mod mmio {
    /// Primary MMIO region (ARM9) - before VRAM
    pub mod region1 {
        /// Start of MMIO region 1
        pub const BASE: u32 = 0x10000000;
        /// End of MMIO region 1 (exclusive)
        pub const END: u32 = 0x18000000;
    }

    /// Secondary MMIO region (ARM9) - after VRAM
    pub mod region2 {
        /// Start of MMIO region 2
        pub const BASE: u32 = 0x18600000;
        /// End of MMIO region 2 (exclusive)
        pub const END: u32 = 0x1FF80000;
    }

    /// ARM11 MMIO split point
    pub const ARM11_MMIO_SPLIT: u32 = 0x17E11000;
}
