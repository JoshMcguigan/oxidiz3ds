//! SDMMC MMIO register handling for 3DS emulation.
//!
//! This module implements the SDMMC (SD/MMC) controller register interface.
//! The SDMMC is mapped at 0x10006000-0x10007000 and handles SD card and NAND access.
//!
//! # References
//! - [EMMC Registers](https://www.3dbrew.org/wiki/EMMC_Registers)
//! - [SD/MMC/SDIO Registers](https://dsibrew.org/wiki/SD/MMC/SDIO_Registers)

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use tracing::{debug, instrument, trace, warn};
use unicorn_engine::Unicorn;

/// SDMMC register offsets (relative to base)
mod reg {
    pub const CMD: u32 = 0x000;
    pub const PORTSEL: u32 = 0x002;
    pub const CMDARG0: u32 = 0x004;
    pub const CMDARG1: u32 = 0x006;
    pub const STOP: u32 = 0x008;
    pub const BLKCOUNT: u32 = 0x00a;
    pub const RESP0: u32 = 0x00c;
    pub const RESP1: u32 = 0x00e;
    pub const RESP2: u32 = 0x010;
    pub const RESP3: u32 = 0x012;
    pub const RESP4: u32 = 0x014;
    pub const RESP5: u32 = 0x016;
    pub const RESP6: u32 = 0x018;
    pub const RESP7: u32 = 0x01a;
    pub const STATUS0: u32 = 0x01c;
    pub const STATUS1: u32 = 0x01e;
    pub const IRQ_MASK0: u32 = 0x020;
    pub const IRQ_MASK1: u32 = 0x022;
    pub const CLKCTL: u32 = 0x024;
    pub const BLKLEN: u32 = 0x026;
    pub const OPT: u32 = 0x028;
    pub const ERROR_DETAIL_STATUS0: u32 = 0x02c;
    pub const ERROR_DETAIL_STATUS1: u32 = 0x02e;
    pub const FIFO: u32 = 0x030;
    pub const DATA_CTL: u32 = 0x0d8;
    pub const RESET: u32 = 0x0e0;
    pub const DATA32_IRQ: u32 = 0x100;
    pub const DATA32_BLK_LEN: u32 = 0x104;
    pub const DATA32_BLK_COUNT: u32 = 0x108;
    pub const DATA32_FIFO: u32 = 0x10c;
}

// Status flag constants
const TMIO_STAT0_CMDRESPEND: u16 = 0x0001;
const TMIO_STAT0_DATAEND: u16 = 0x0004;
const TMIO_STAT0_CARD_INSERTED: u16 = 1 << 5;
const TMIO_STAT0_WRPROTECT: u16 = 1 << 7;

const TMIO_STAT1_RXRDY: u16 = 0x0100;
const TMIO_STAT1_TXRQ: u16 = 0x0200;
const TMIO_STAT1_CMD_BUSY: u16 = 0x4000;

// MMC card states (stored in STATUS1 bits 9-12, also returned in R1 response)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
enum MmcState {
    Idle = 0,
    Ready = 1,
    Identify = 2,
    Standby = 3,
    Transfer = 4,
    Data = 5,
    Receive = 6,
    Program = 7,
}

/// SDMMC state tracking controller registers and internal emulation state
#[derive(Debug)]
pub struct SdmmcState {
    // ========================================================================
    // REGISTER STATE - Direct mappings to hardware registers
    // ========================================================================

    // Command and control registers
    pub cmd: u16,      // 0x000: REG_CMD
    pub portsel: u16,  // 0x002: REG_PORTSEL
    pub cmdarg0: u16,  // 0x004: REG_CMDARG0
    pub cmdarg1: u16,  // 0x006: REG_CMDARG1
    pub stop: u16,     // 0x008: REG_STOP
    pub blkcount: u16, // 0x00A: REG_BLKCOUNT

    // Response data registers (8 registers × 2 bytes each)
    pub resp: [u16; 8], // 0x00C-0x01A: REG_RESP0-7

    // Status and interrupt registers
    pub status0: u16,   // 0x01C: REG_STATUS0
    pub status1: u16,   // 0x01E: REG_STATUS1
    pub irq_mask0: u16, // 0x020: REG_IRQ_MASK0
    pub irq_mask1: u16, // 0x022: REG_IRQ_MASK1

    // Configuration registers
    pub clkctl: u16, // 0x024: REG_CLKCTL
    pub blklen: u16, // 0x026: REG_BLKLEN
    pub opt: u16,    // 0x028: REG_OPT

    // Error status registers
    pub error_detail_status0: u16, // 0x02C: REG_ERROR_DETAIL_STATUS0
    pub error_detail_status1: u16, // 0x02E: REG_ERROR_DETAIL_STATUS1

    // Data transfer registers (16-bit mode)
    pub fifo: u16, // 0x030: REG_FIFO

    // Data control registers
    pub data_ctl: u16, // 0x0D8: REG_DATA_CTL

    // Reset register
    pub reset: u16, // 0x0E0: REG_RESET

    // 32-bit mode registers
    pub data32_irq: u16,       // 0x100: REG_DATA32_IRQ
    pub data32_blk_len: u16,   // 0x104: REG_DATA32_BLK_LEN
    pub data32_blk_count: u16, // 0x108: REG_DATA32_BLK_COUNT
    pub data32_fifo: u32,      // 0x10C: REG_DATA32_FIFO

    // ========================================================================
    // INTERNAL STATE - Emulation bookkeeping (not directly mapped to registers)
    // ========================================================================
    /// Next command should be interpreted as ACMD (set by CMD55)
    app_command_next: bool,

    /// Current data transfer buffer (for FIFO reads/writes)
    transfer_buffer: Vec<u8>,

    /// Current position within transfer_buffer
    transfer_pos: usize,

    /// Number of blocks remaining in multi-block transfer
    transfer_blocks_remaining: u16,

    /// Starting address for current transfer operation
    transfer_start_addr: u32,

    /// SD card backing file handle
    sd_file: Option<std::fs::File>,
}

impl SdmmcState {
    pub fn new(sd_card_path: Option<PathBuf>) -> Self {
        // Open SD card file if path provided
        let sd_file = sd_card_path.and_then(|path| {
            match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
            {
                Ok(file) => {
                    debug!("Opened SD card image: {:?}", path);
                    Some(file)
                }
                Err(e) => {
                    warn!("Failed to open SD card image {:?}: {}", path, e);
                    None
                }
            }
        });

        Self {
            // Register state
            cmd: 0,
            portsel: 0,
            cmdarg0: 0,
            cmdarg1: 0,
            stop: 0,
            blkcount: 0,
            resp: [0; 8],
            status0: 0,
            status1: 0,
            irq_mask0: 0,
            irq_mask1: 0,
            clkctl: 0,
            blklen: 0,
            opt: 0,
            error_detail_status0: 0,
            error_detail_status1: 0,
            fifo: 0,
            data_ctl: 0,
            reset: 0,
            data32_irq: 0,
            data32_blk_len: 0,
            data32_blk_count: 0,
            data32_fifo: 0,

            // Internal state
            app_command_next: false,
            transfer_buffer: Vec::new(),
            transfer_pos: 0,
            transfer_blocks_remaining: 0,
            transfer_start_addr: 0,
            sd_file,
        }
    }

    /// Handle a write to an SDMMC register
    pub fn write(&mut self, offset: u32, _size: usize, value: u32) {
        trace!(
            "SDMMC register write: offset={:#X}, value={:#X}",
            offset, value
        );

        match offset {
            reg::CMD => {
                self.cmd = value as u16;
                let cmd = (value & 0x3F) as u8;
                let arg = self.get_argument();

                debug!(
                    "SDMMC command: {:#X} (CMD{}, arg={:#X})",
                    self.cmd, cmd, arg
                );

                // Set CMD_BUSY to indicate command is being processed
                self.status1 |= TMIO_STAT1_CMD_BUSY;

                // Execute command (will clear CMD_BUSY when done)
                if self.app_command_next {
                    self.app_command_next = false;
                    self.execute_acmd(cmd, arg);
                } else {
                    self.execute_cmd(cmd, arg);
                }
            }
            reg::PORTSEL => {
                self.portsel = value as u16;
                debug!(
                    "SDMMC port select: {} ({})",
                    self.portsel,
                    if self.portsel == 0 { "SD card" } else { "NAND" }
                );
            }
            reg::CMDARG0 => {
                self.cmdarg0 = value as u16;
                debug!("SDMMC command arg0: {:#X}", self.cmdarg0);
            }
            reg::CMDARG1 => {
                self.cmdarg1 = value as u16;
                debug!("SDMMC command arg1: {:#X}", self.cmdarg1);
            }
            reg::STOP => {
                self.stop = value as u16;
                debug!("SDMMC stop: {:#X}", self.stop);
            }
            reg::BLKCOUNT => {
                self.blkcount = value as u16;
                debug!("SDMMC block count: {}", self.blkcount);
            }
            reg::RESP0 => {
                self.resp[0] = value as u16;
                trace!("SDMMC response 0: {:#X}", self.resp[0]);
            }
            reg::RESP1 => {
                self.resp[1] = value as u16;
                trace!("SDMMC response 1: {:#X}", self.resp[1]);
            }
            reg::RESP2 => {
                self.resp[2] = value as u16;
                trace!("SDMMC response 2: {:#X}", self.resp[2]);
            }
            reg::RESP3 => {
                self.resp[3] = value as u16;
                trace!("SDMMC response 3: {:#X}", self.resp[3]);
            }
            reg::RESP4 => {
                self.resp[4] = value as u16;
                trace!("SDMMC response 4: {:#X}", self.resp[4]);
            }
            reg::RESP5 => {
                self.resp[5] = value as u16;
                trace!("SDMMC response 5: {:#X}", self.resp[5]);
            }
            reg::RESP6 => {
                self.resp[6] = value as u16;
                trace!("SDMMC response 6: {:#X}", self.resp[6]);
            }
            reg::RESP7 => {
                self.resp[7] = value as u16;
                trace!("SDMMC response 7: {:#X}", self.resp[7]);
            }
            reg::STATUS0 => {
                // Write value as mask: bits set in value are kept, others cleared
                self.status0 &= value as u16;
                trace!("SDMMC status0: {:#X}", self.status0);
            }
            reg::STATUS1 => {
                // Write value as mask: bits set in value are kept, others cleared
                self.status1 &= value as u16;
                trace!("SDMMC status1: {:#X}", self.status1);
            }
            reg::IRQ_MASK0 => {
                self.irq_mask0 = value as u16;
                debug!("SDMMC IRQ mask0: {:#X}", self.irq_mask0);
            }
            reg::IRQ_MASK1 => {
                self.irq_mask1 = value as u16;
                debug!("SDMMC IRQ mask1: {:#X}", self.irq_mask1);
            }
            reg::CLKCTL => {
                self.clkctl = value as u16;
                debug!("SDMMC clock control: {:#X}", self.clkctl);
            }
            reg::BLKLEN => {
                self.blklen = value as u16;
                debug!("SDMMC block length: {}", self.blklen);
            }
            reg::OPT => {
                self.opt = value as u16;
                debug!("SDMMC options: {:#X}", self.opt);
            }
            reg::ERROR_DETAIL_STATUS0 => {
                self.error_detail_status0 = value as u16;
                trace!(
                    "SDMMC error detail status0: {:#X}",
                    self.error_detail_status0
                );
            }
            reg::ERROR_DETAIL_STATUS1 => {
                self.error_detail_status1 = value as u16;
                trace!(
                    "SDMMC error detail status1: {:#X}",
                    self.error_detail_status1
                );
            }
            reg::FIFO => {
                self.fifo = value as u16;
                trace!("SDMMC FIFO write: {:#X}", self.fifo);
            }
            reg::DATA_CTL => {
                self.data_ctl = value as u16;
                debug!("SDMMC data control: {:#X}", self.data_ctl);
            }
            reg::RESET => {
                self.reset = value as u16;
                debug!("SDMMC reset: {:#X}", self.reset);
            }
            reg::DATA32_IRQ => {
                self.data32_irq = value as u16;
                // Firmware writes to this register using sdmmc_mask16 to clear bits 0x800/0x1000
                // These appear to be control/acknowledgement bits, but STATUS1 flags are cleared
                // separately via writes to REG_STATUS1
                debug!("SDMMC data32 IRQ: {:#X}", self.data32_irq);
            }
            reg::DATA32_BLK_LEN => {
                self.data32_blk_len = value as u16;
                debug!("SDMMC data32 block length: {}", self.data32_blk_len);
            }
            reg::DATA32_BLK_COUNT => {
                self.data32_blk_count = value as u16;
                debug!("SDMMC data32 block count: {}", self.data32_blk_count);
            }
            reg::DATA32_FIFO => {
                self.data32_fifo = value;
                self.write_fifo32(value);
            }
            _ => {
                // Unknown register - log at warn level
                warn!(
                    "Unknown SDMMC register write: offset={:#X}, value={:#X}",
                    offset, value
                );
            }
        }
    }

    /// Handle a read from an SDMMC register
    pub fn read(&mut self, offset: u32, _size: usize) -> u32 {
        trace!("SDMMC register read: offset={:#X}", offset);

        match offset {
            reg::CMD => self.cmd as u32,
            reg::PORTSEL => self.portsel as u32,
            reg::CMDARG0 => self.cmdarg0 as u32,
            reg::CMDARG1 => self.cmdarg1 as u32,
            reg::STOP => self.stop as u32,
            reg::BLKCOUNT => self.blkcount as u32,
            reg::RESP0 => self.resp[0] as u32,
            reg::RESP1 => self.resp[1] as u32,
            reg::RESP2 => self.resp[2] as u32,
            reg::RESP3 => self.resp[3] as u32,
            reg::RESP4 => self.resp[4] as u32,
            reg::RESP5 => self.resp[5] as u32,
            reg::RESP6 => self.resp[6] as u32,
            reg::RESP7 => self.resp[7] as u32,
            reg::STATUS0 => {
                // Add card inserted and write protect bits
                let mut status = self.status0;
                status |= TMIO_STAT0_CARD_INSERTED; // Card always inserted
                status |= TMIO_STAT0_WRPROTECT; // Not write-protected
                trace!(
                    "STATUS0 read: {:#X} (CMDRESPEND={} DATAEND={})",
                    status,
                    status & TMIO_STAT0_CMDRESPEND != 0,
                    status & TMIO_STAT0_DATAEND != 0
                );
                status as u32
            }
            reg::STATUS1 => self.status1 as u32,
            reg::IRQ_MASK0 => self.irq_mask0 as u32,
            reg::IRQ_MASK1 => self.irq_mask1 as u32,
            reg::CLKCTL => self.clkctl as u32,
            reg::BLKLEN => self.blklen as u32,
            reg::OPT => self.opt as u32,
            reg::ERROR_DETAIL_STATUS0 => self.error_detail_status0 as u32,
            reg::ERROR_DETAIL_STATUS1 => self.error_detail_status1 as u32,
            reg::FIFO => self.fifo as u32,
            reg::DATA_CTL => self.data_ctl as u32,
            reg::RESET => self.reset as u32,
            reg::DATA32_IRQ => {
                // REG_DATACTL32 - bits 8-9 reflect RXRDY/TXRQ status
                let mut val = self.data32_irq;
                if self.status1 & TMIO_STAT1_RXRDY != 0 {
                    val |= 0x100; // Bit 8: read buffer ready (set when data available)
                }
                // Bit 9 has INVERTED semantics: clear when ready to transmit
                // Firmware checks !(ctl32 & 0x200) for write readiness (sdmmc.c:157)
                if self.status1 & TMIO_STAT1_TXRQ == 0 {
                    val |= 0x200; // Bit 9: transmit buffer full (clear = ready to write)
                }
                debug!(
                    "DATA32_IRQ/DATACTL32 read: {:#X} (RXRDY bit set: {})",
                    val,
                    val & 0x100 != 0
                );
                val as u32
            }
            reg::DATA32_BLK_LEN => self.data32_blk_len as u32,
            reg::DATA32_BLK_COUNT => self.data32_blk_count as u32,
            reg::DATA32_FIFO => self.read_fifo32(),
            _ => {
                warn!("Unknown SDMMC register read: offset={:#X}", offset);
                0
            }
        }
    }

    // ========================================================================
    // Helper methods for command execution
    // ========================================================================

    /// Get current MMC state from STATUS1 register (bits 9-12)
    fn get_state(&self) -> MmcState {
        let state_bits = (self.status1 >> 9) & 0xF;
        match state_bits {
            0 => MmcState::Idle,
            1 => MmcState::Ready,
            2 => MmcState::Identify,
            3 => MmcState::Standby,
            4 => MmcState::Transfer,
            5 => MmcState::Data,
            6 => MmcState::Receive,
            7 => MmcState::Program,
            _ => MmcState::Idle,
        }
    }

    /// Set MMC state in STATUS1 register (bits 9-12)
    fn set_state(&mut self, state: MmcState) {
        self.status1 = (self.status1 & !0x1E00) | ((state as u16) << 9);
    }

    /// Generate R1 response format
    fn get_r1_response(&self) -> u32 {
        let mut r1 = 0u32;
        r1 |= (self.app_command_next as u32) << 5;
        r1 |= (self.get_state() as u32) << 9;
        // Ready for data if no transfer in progress
        if self.transfer_blocks_remaining == 0 {
            r1 |= 1 << 8;
        }
        r1
    }

    /// Mark command as completed
    fn command_end(&mut self) {
        // Clear CMD_BUSY flag in STATUS1
        self.status1 &= !TMIO_STAT1_CMD_BUSY;
        // Set CMDRESPEND flag in STATUS0
        self.status0 |= TMIO_STAT0_CMDRESPEND;
    }

    /// Get full 32-bit argument from CMDARG0 and CMDARG1
    fn get_argument(&self) -> u32 {
        (self.cmdarg1 as u32) << 16 | self.cmdarg0 as u32
    }

    /// Write 128-bit response (4x u32) to RESP0-7 registers
    fn set_response_128(&mut self, resp: &[u32; 4]) {
        for (i, r) in resp.iter().enumerate() {
            self.resp[i * 2] = (r & 0xFFFF) as u16;
            self.resp[i * 2 + 1] = (r >> 16) as u16;
        }
    }

    /// Write 32-bit response to RESP0-1 registers
    fn set_response_32(&mut self, resp: u32) {
        self.resp[0] = (resp & 0xFFFF) as u16;
        self.resp[1] = (resp >> 16) as u16;
    }

    /// Check if NAND is currently selected (portsel == 1)
    fn nand_selected(&self) -> bool {
        self.portsel == 1
    }

    // ========================================================================
    // Command execution
    // ========================================================================

    /// Execute an SD/MMC command
    fn execute_cmd(&mut self, cmd: u8, arg: u32) {
        debug!("SDMMC CMD{}", cmd);

        match cmd {
            0 => self.cmd0_go_idle_state(),
            1 => self.cmd1_send_op_cond(),
            2 => self.cmd2_all_send_cid(),
            3 => self.cmd3_send_relative_addr(arg),
            7 => self.cmd7_select_card(),
            8 => self.cmd8_send_if_cond(),
            9 => self.cmd9_send_csd(),
            10 => self.cmd10_send_cid(),
            12 => self.cmd12_stop_transmission(),
            13 => self.cmd13_send_status(),
            16 => self.cmd16_set_blocklen(arg),
            18 => self.cmd18_read_multiple_block(arg),
            25 => self.cmd25_write_multiple_block(arg),
            55 => self.cmd55_app_cmd(),
            _ => {
                warn!("Unimplemented SDMMC CMD{}", cmd);
                self.command_end();
            }
        }
    }

    /// Execute an application-specific command (after CMD55)
    fn execute_acmd(&mut self, cmd: u8, arg: u32) {
        debug!("SDMMC ACMD{}", cmd);

        match cmd {
            6 => self.acmd6_set_bus_width(arg),
            13 => self.acmd13_sd_status(),
            41 => self.acmd41_sd_send_op_cond(arg),
            42 => self.acmd42_set_clr_card_detect(),
            51 => self.acmd51_send_scr(),
            _ => {
                warn!("Unimplemented SDMMC ACMD{}", cmd);
                self.command_end();
            }
        }
    }

    // ========================================================================
    // Individual command implementations
    // ========================================================================

    /// CMD0: GO_IDLE_STATE - Reset card to idle state
    fn cmd0_go_idle_state(&mut self) {
        self.set_state(MmcState::Idle);
        self.set_response_32(1 << 9); // Card ready bit
        self.command_end();
    }

    /// CMD1: SEND_OP_COND - Send operating conditions
    fn cmd1_send_op_cond(&mut self) {
        let ocr = 0x80FF8080u32; // Operating conditions register
        self.set_response_32(ocr);
        self.command_end();
    }

    /// CMD2: ALL_SEND_CID - Send card identification
    fn cmd2_all_send_cid(&mut self) {
        // CID for NAND or SD card
        let cid = if self.nand_selected() {
            // NAND CID (from Corgi3DS - would normally be loaded from essentials.exefs)
            [0x00000000u32, 0x00000000, 0x00000000, 0x00000000]
        } else {
            // SD card CID (from Corgi3DS)
            [0xD71C65CD, 0x4445147B, 0x4D324731, 0x00150100]
        };
        self.set_response_128(&cid);
        self.command_end();

        if self.get_state() == MmcState::Ready {
            self.set_state(MmcState::Identify);
        }
    }

    /// CMD3: SEND_RELATIVE_ADDR - Get/set relative card address
    fn cmd3_send_relative_addr(&mut self, _arg: u32) {
        let rca = 0x00010000u32; // Relative card address
        let status = self.get_r1_response();
        self.set_response_32(rca | status);
        self.command_end();

        if self.get_state() == MmcState::Identify {
            self.set_state(MmcState::Standby);
        }
    }

    /// CMD7: SELECT_CARD - Select/deselect card
    fn cmd7_select_card(&mut self) {
        self.set_response_32(self.get_r1_response());
        self.command_end();
    }

    /// CMD8: SEND_IF_COND - Send interface condition
    fn cmd8_send_if_cond(&mut self) {
        self.set_response_32(0x1AA); // Voltage accepted, check pattern
        self.command_end();
    }

    /// CMD9: SEND_CSD - Send card-specific data
    fn cmd9_send_csd(&mut self) {
        // CSD register (from Corgi3DS)
        let csd = [0xe9964040u32, 0xdff6db7f, 0x2a0f5901, 0x3f269001];
        self.set_response_128(&csd);
        self.command_end();
    }

    /// CMD10: SEND_CID - Send card identification
    fn cmd10_send_cid(&mut self) {
        // Return NAND CID (usually used for NAND)
        let cid = [0x00000000u32, 0x00000000, 0x00000000, 0x00000000];
        self.set_response_128(&cid);
        self.command_end();
    }

    /// CMD12: STOP_TRANSMISSION - Stop multi-block read/write
    fn cmd12_stop_transmission(&mut self) {
        self.set_response_32(self.get_r1_response());
        self.transfer_blocks_remaining = 0;
        self.transfer_buffer.clear();
        self.command_end();

        // State transitions
        match self.get_state() {
            MmcState::Data | MmcState::Receive => self.set_state(MmcState::Transfer),
            MmcState::Transfer => self.set_state(MmcState::Standby),
            _ => {}
        }
    }

    /// CMD13: SEND_STATUS - Send card status
    fn cmd13_send_status(&mut self) {
        self.set_response_32(self.get_r1_response());
        self.command_end();
    }

    /// CMD16: SET_BLOCKLEN - Set block length
    fn cmd16_set_blocklen(&mut self, arg: u32) {
        debug!("SDMMC set block length: {}", arg);
        self.command_end();
    }

    /// CMD18: READ_MULTIPLE_BLOCK - Read multiple blocks
    fn cmd18_read_multiple_block(&mut self, arg: u32) {
        let sector = arg;

        // Use 32-bit mode parameters if available (data32_blk_count/len are set), otherwise use 16-bit
        let (blocks, block_len) = if self.data32_blk_len > 0 {
            (self.data32_blk_count, self.data32_blk_len as usize)
        } else {
            (self.blkcount, self.blklen as usize)
        };

        debug!(
            "SDMMC read multiple blocks: sector={:#X}, blocks={}, len={} (32-bit mode: {}, port: {})",
            sector,
            blocks,
            block_len,
            self.data32_blk_len > 0,
            if self.portsel == 0 { "SD" } else { "NAND" }
        );

        self.transfer_start_addr = sector;
        self.transfer_blocks_remaining = blocks;
        self.transfer_pos = 0;
        self.set_state(MmcState::Data);

        // Prepare first block
        self.transfer_buffer = vec![0u8; block_len];

        // Read from SD card file if available and SD port is selected
        if self.portsel == 0
            && let Some(ref mut file) = self.sd_file
        {
            let offset = sector as u64 * 512; // Standard 512-byte sectors
            if let Err(e) = file.seek(SeekFrom::Start(offset)) {
                warn!("Failed to seek SD card to sector {}: {}", sector, e);
            } else if let Err(e) = file.read_exact(&mut self.transfer_buffer) {
                warn!("Failed to read from SD card sector {}: {}", sector, e);
                self.transfer_buffer.fill(0); // Fill with zeros on error
            } else {
                debug!("Read {} bytes from SD card sector {:#X}", block_len, sector);
            }
        }
        // NAND reads remain stubbed (return zeros)

        self.set_response_32(self.get_r1_response());
        self.command_end();

        // Signal data ready
        self.status1 |= TMIO_STAT1_RXRDY;
    }

    /// CMD25: WRITE_MULTIPLE_BLOCK - Write multiple blocks
    fn cmd25_write_multiple_block(&mut self, arg: u32) {
        let sector = arg;

        // Use 32-bit mode parameters if available (data32_blk_count/len are set), otherwise use 16-bit
        let (blocks, block_len) = if self.data32_blk_len > 0 {
            (self.data32_blk_count, self.data32_blk_len as usize)
        } else {
            (self.blkcount, self.blklen as usize)
        };

        debug!(
            "SDMMC write multiple blocks: sector={:#X}, blocks={}, len={} (32-bit mode: {}, port: {})",
            sector,
            blocks,
            block_len,
            self.data32_blk_len > 0,
            if self.portsel == 0 { "SD" } else { "NAND" }
        );

        self.transfer_start_addr = sector;
        self.transfer_blocks_remaining = blocks;
        self.transfer_pos = 0;
        self.set_state(MmcState::Receive);

        // Prepare buffer for receiving data
        self.transfer_buffer = vec![0u8; block_len];

        self.set_response_32(self.get_r1_response());
        self.command_end();

        // Signal ready for write data
        self.status1 |= TMIO_STAT1_TXRQ;
    }

    /// CMD55: APP_CMD - Next command is application-specific
    fn cmd55_app_cmd(&mut self) {
        self.app_command_next = true;
        self.set_response_32(self.get_r1_response());
        self.command_end();
    }

    /// ACMD6: SET_BUS_WIDTH - Set bus width
    fn acmd6_set_bus_width(&mut self, _arg: u32) {
        self.set_response_32(self.get_r1_response());
        self.command_end();
    }

    /// ACMD13: SD_STATUS - Send SD status
    fn acmd13_sd_status(&mut self) {
        self.set_response_32(self.get_r1_response());

        // Prepare SD status buffer (64 bytes, mostly zeros)
        self.transfer_buffer = vec![0u8; 64];
        self.transfer_pos = 0;

        self.command_end();
        self.status1 |= TMIO_STAT1_RXRDY;
    }

    /// ACMD41: SD_SEND_OP_COND - Send SD operating conditions
    fn acmd41_sd_send_op_cond(&mut self, _arg: u32) {
        let mut ocr = 0x80FF8080u32;

        // Set SDHC bit (bit 30) for SD cards
        if !self.nand_selected() {
            ocr |= 1 << 30;
        }

        self.set_response_32(ocr);
        self.command_end();

        if self.get_state() == MmcState::Idle {
            self.set_state(MmcState::Ready);
        }
    }

    /// ACMD42: SET_CLR_CARD_DETECT - Set/clear card detect
    fn acmd42_set_clr_card_detect(&mut self) {
        self.set_response_32(self.get_r1_response());
        self.command_end();
    }

    /// ACMD51: SEND_SCR - Send SD configuration register
    fn acmd51_send_scr(&mut self) {
        self.set_response_32(self.get_r1_response());

        // Prepare SCR buffer (8 bytes)
        let scr = [0u8, 0x00, 0x00, 0x2a, 0x01, 0x00, 0x00, 0x00];
        self.transfer_buffer = scr.to_vec();
        self.transfer_pos = 0;

        self.command_end();
        self.status1 |= TMIO_STAT1_RXRDY;
    }

    // ========================================================================
    // FIFO data transfer methods
    // ========================================================================

    /// Read 32 bits from the FIFO (for data transfer)
    fn read_fifo32(&mut self) -> u32 {
        if self.transfer_pos + 4 <= self.transfer_buffer.len() {
            let value = u32::from_le_bytes([
                self.transfer_buffer[self.transfer_pos],
                self.transfer_buffer[self.transfer_pos + 1],
                self.transfer_buffer[self.transfer_pos + 2],
                self.transfer_buffer[self.transfer_pos + 3],
            ]);
            trace!(
                "SDMMC FIFO32 read: {:#X} (pos={:#X})",
                value, self.transfer_pos
            );
            self.transfer_pos += 4;

            // Check if block is complete
            if self.transfer_pos >= self.transfer_buffer.len() {
                self.handle_block_complete_read();
            }

            value
        } else {
            warn!(
                "SDMMC FIFO32 read beyond buffer (pos={}, len={})",
                self.transfer_pos,
                self.transfer_buffer.len()
            );
            0
        }
    }

    /// Write 32 bits to the FIFO (for data transfer)
    fn write_fifo32(&mut self, value: u32) {
        trace!(
            "SDMMC FIFO32 write: {:#X} (pos={:#X})",
            value, self.transfer_pos
        );

        if self.transfer_pos + 4 <= self.transfer_buffer.len() {
            let bytes = value.to_le_bytes();
            self.transfer_buffer[self.transfer_pos..self.transfer_pos + 4].copy_from_slice(&bytes);
            self.transfer_pos += 4;

            // Check if block is complete
            if self.transfer_pos >= self.transfer_buffer.len() {
                self.handle_block_complete_write();
            }
        } else {
            warn!(
                "SDMMC FIFO32 write beyond buffer (pos={}, len={})",
                self.transfer_pos,
                self.transfer_buffer.len()
            );
        }
    }

    /// Handle completion of reading a block
    fn handle_block_complete_read(&mut self) {
        debug!(
            "SDMMC block read complete, {} blocks remaining",
            self.transfer_blocks_remaining
        );

        if self.transfer_blocks_remaining > 0 {
            self.transfer_blocks_remaining -= 1;
            self.transfer_pos = 0;
            debug!(
                "Decremented blocks remaining to {}",
                self.transfer_blocks_remaining
            );

            if self.transfer_blocks_remaining == 0 {
                // All blocks transferred
                debug!("All blocks transferred, setting DATAEND flag");
                self.status0 |= TMIO_STAT0_DATAEND;
                self.transfer_buffer.clear();
                self.set_state(MmcState::Transfer);
            } else {
                // Load next block
                let next_sector = self.transfer_start_addr
                    + (self.blkcount - self.transfer_blocks_remaining) as u32;

                // Read from SD card if available and SD port is selected
                if self.portsel == 0
                    && let Some(ref mut file) = self.sd_file
                {
                    let offset = next_sector as u64 * 512;
                    if let Err(e) = file.seek(SeekFrom::Start(offset)) {
                        warn!("Failed to seek SD card to sector {}: {}", next_sector, e);
                    } else if let Err(e) = file.read_exact(&mut self.transfer_buffer) {
                        warn!("Failed to read from SD card sector {}: {}", next_sector, e);
                        self.transfer_buffer.fill(0);
                    } else {
                        debug!("Read next block from SD card sector {:#X}", next_sector);
                    }
                }

                debug!("More blocks remaining, setting RXRDY flag");
                self.status1 |= TMIO_STAT1_RXRDY;
            }
        }
    }

    /// Handle completion of writing a block
    fn handle_block_complete_write(&mut self) {
        trace!(
            "SDMMC block write complete, {} blocks remaining",
            self.transfer_blocks_remaining
        );

        // Write to SD card if available and SD port is selected
        let current_sector =
            self.transfer_start_addr + (self.blkcount - self.transfer_blocks_remaining) as u32;

        if self.portsel == 0
            && let Some(ref mut file) = self.sd_file
        {
            let offset = current_sector as u64 * 512;
            if let Err(e) = file.seek(SeekFrom::Start(offset)) {
                warn!("Failed to seek SD card to sector {}: {}", current_sector, e);
            } else if let Err(e) = file.write_all(&self.transfer_buffer) {
                warn!(
                    "Failed to write to SD card sector {}: {}",
                    current_sector, e
                );
            } else {
                debug!(
                    "Wrote {} bytes to SD card sector {:#X}",
                    self.transfer_buffer.len(),
                    current_sector
                );
                // Ensure data is flushed to disk
                let _ = file.flush();
            }
        }
        // NAND writes remain stubbed (ignored)

        if self.transfer_blocks_remaining > 0 {
            self.transfer_blocks_remaining -= 1;
            self.transfer_pos = 0;

            if self.transfer_blocks_remaining == 0 {
                // All blocks transferred
                self.status0 |= TMIO_STAT0_DATAEND;
                self.transfer_buffer.clear();
                self.set_state(MmcState::Transfer);
            } else {
                // Ready for next block
                self.status1 |= TMIO_STAT1_TXRQ;
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
/// offsets expected by the SDMMC handler.
#[instrument(level = "trace", skip(uc))]
pub fn read_handler(uc: &mut Unicorn<'_, super::EmulatorState>, addr: u64, size: usize) -> u64 {
    uc.get_data_mut().sdmmc.read(addr as u32, size) as u64
}

/// MMIO write handler function (for use with Unicorn)
///
/// This is a thin adapter that converts Unicorn's u64 addresses and values to the u32
/// types expected by the SDMMC handler.
#[instrument(level = "trace", skip(uc))]
pub fn write_handler(
    uc: &mut Unicorn<'_, super::EmulatorState>,
    addr: u64,
    size: usize,
    value: u64,
) {
    uc.get_data_mut()
        .sdmmc
        .write(addr as u32, size, value as u32);
}
