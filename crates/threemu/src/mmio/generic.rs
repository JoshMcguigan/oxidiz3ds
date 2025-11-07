//! Generic MMIO handler for unimplemented I/O regions.
//!
//! This module provides stub handlers that return zero for reads and ignore writes.
//! Used for MMIO regions that don't have specific implementations yet.
//!
//! In a full emulator, these would be replaced with specific handlers for each
//! hardware component (timers, DMA, interrupts, etc.).

use tracing::{instrument, trace};
use unicorn_engine::Unicorn;

/// Generic MMIO read handler - returns zero
///
/// This is a placeholder for unimplemented MMIO regions.
/// Real hardware would return specific values based on the register.
#[instrument(level = "trace", skip(_uc))]
pub fn read_handler(_uc: &mut Unicorn<'_, super::EmulatorState>, addr: u64, size: usize) -> u64 {
    trace!("Generic MMIO read: addr={:#X}, size={}", addr, size);
    0
}

/// Generic MMIO write handler - ignores writes
///
/// This is a placeholder for unimplemented MMIO regions.
/// Real hardware would perform specific actions based on the register.
#[instrument(level = "trace", skip(_uc))]
pub fn write_handler(
    _uc: &mut Unicorn<'_, super::EmulatorState>,
    addr: u64,
    size: usize,
    value: u64,
) {
    trace!(
        "Generic MMIO write: addr={:#X}, size={}, value={:#X}",
        addr, size, value
    );
    // Ignore writes
}
