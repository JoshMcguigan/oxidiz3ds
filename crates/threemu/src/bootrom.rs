//! This module implements stubs for bootrom functions that emulated code
//! may branch to.
//!
//! The only documentation I've found for these functions exists here:
//! <https://github.com/linux-3ds/arm9linuxfw/blob/206978444c04c65d1fc9e5a841196f7bd1623926/include/arm/bfn.h>

use tracing::{trace, warn};
use unicorn_engine::RegisterARM;

pub const ARM9_REGION_START: u32 = 0xFFFF_0000;
pub const ARM9_REGION_END: u32 = 0xFFFF_FFFF;
pub const ARM9_REGION_LEN: u32 = (ARM9_REGION_END - ARM9_REGION_START) + 1;

const WAIT_CYCLES_FN_ADDR_OFFSET: u32 = 0x0198;

pub fn handle_instruction(
    uc: &mut unicorn_engine::Unicorn<'_, crate::mmio::EmulatorState>,
    addr: u32,
) {
    let addr_offset = addr % 0x1_0000;
    match addr_offset {
        WAIT_CYCLES_FN_ADDR_OFFSET => {
            trace!("handling bootrom function at WAIT_CYCLES_FN_ADDR_OFFSET");
            // Handling WAIT_CYCLES_FN_ADDR_OFFSET as a no-op.
        }
        _ => {
            warn!(
                "attempting to execute unknown bootrom instruction at address offset {addr_offset:#x}"
            );
        }
    }

    uc.reg_write(RegisterARM::PC, uc.reg_read(RegisterARM::LR).unwrap())
        .unwrap();
}
