//! # References
//! - <https://www.3dbrew.org/wiki/EMMC_Registers>
//! - <https://dsibrew.org/wiki/SD/MMC/SDIO_Registers>

/// SDMMC MMIO region base address
pub const BASE: u32 = 0x10006000;

/// SDMMC MMIO region end address (exclusive)
pub const END: u32 = 0x10007000;

/// SDMMC register offsets (relative to `BASE`)
pub mod registers {
    /// Command register
    pub const CMD: u32 = 0x000;

    /// Port selection register (0 = SD card, 1 = NAND)
    pub const PORTSEL: u32 = 0x002;

    /// Command argument register (lower 16 bits)
    pub const CMDARG0: u32 = 0x004;

    /// Command argument register (upper 16 bits)
    pub const CMDARG1: u32 = 0x006;

    /// Stop/abort command register
    pub const STOP: u32 = 0x008;

    /// Block count register
    pub const BLKCOUNT: u32 = 0x00a;

    /// Response data registers (8 registers × 2 bytes each)
    pub const RESP0: u32 = 0x00c;
    pub const RESP1: u32 = 0x00e;
    pub const RESP2: u32 = 0x010;
    pub const RESP3: u32 = 0x012;
    pub const RESP4: u32 = 0x014;
    pub const RESP5: u32 = 0x016;
    pub const RESP6: u32 = 0x018;
    pub const RESP7: u32 = 0x01a;

    /// Status register 0 (includes card detection flags)
    pub const STATUS0: u32 = 0x01c;

    /// Status register 1
    pub const STATUS1: u32 = 0x01e;

    /// Interrupt status register
    pub const IRQ_STAT: u32 = 0x020;

    /// Interrupt mask register
    pub const IRQ_MASK: u32 = 0x024;

    /// Clock control register
    pub const CLK_CTL: u32 = 0x028;

    /// Block length register
    pub const BLKLEN: u32 = 0x02a;

    /// Option register
    pub const OPTION: u32 = 0x02c;

    /// FIFO control register
    pub const FIFO_CTL: u32 = 0x034;

    /// Data FIFO register (16-bit access)
    pub const DATA_FIFO: u32 = 0x030;

    /// Data control register
    pub const DATA_CTL: u32 = 0x038;

    /// Software reset register
    pub const SOFT_RST: u32 = 0x100;

    /// SD clock control register
    pub const SD_CLK_CTL: u32 = 0x104;
}

/// SDMMC command bit flags
pub mod cmd_flags {
    /// Response type mask
    pub const RESP_MASK: u16 = 0x0060;
    /// No response
    pub const RESP_NONE: u16 = 0x0000;
    /// R1 response (48-bit)
    pub const RESP_R1: u16 = 0x0020;
    /// R1b response (48-bit with busy)
    pub const RESP_R1B: u16 = 0x0060;
    /// R2 response (136-bit)
    pub const RESP_R2: u16 = 0x0040;
    /// R3 response (48-bit without CRC)
    pub const RESP_R3: u16 = 0x0020;
}

/// SDMMC status register bit flags
pub mod status {
    /// Command response received
    pub const CMD_RESP_END: u32 = 0x0001;
    /// Data transfer end
    pub const DATA_END: u32 = 0x0004;
    /// Card removed
    pub const CARD_REMOVED: u32 = 0x0008;
    /// Card inserted
    pub const CARD_INSERTED: u32 = 0x0010;
    /// Write protect enabled
    pub const WRITE_PROTECT: u32 = 0x0020;
}
