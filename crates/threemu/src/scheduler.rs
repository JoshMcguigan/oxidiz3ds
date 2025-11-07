//! CPU scheduling for dual-core 3DS emulation.
//!
//! This module handles the interleaving of ARM9 and ARM11 execution,
//! maintaining timing ratios based on real hardware clock speeds.

use crate::mmio;
use tracing::error;
use unicorn_engine::{RegisterARM, Unicorn};

// ================================================================================================
// Emulation Timing Constants
// ================================================================================================

/// Nintendo 3DS ARM11 CPU frequency in Hz
/// Reference: https://www.3dbrew.org/wiki/Hardware
pub const ARM11_FREQ_HZ: usize = 268_000_000; // 268 MHz

/// Nintendo 3DS ARM9 CPU frequency in Hz
/// Reference: https://www.3dbrew.org/wiki/Hardware
pub const ARM9_FREQ_HZ: usize = 134_000_000; // 134 MHz

/// Target frames per second (3DS screens run at 60Hz)
pub const TARGET_FPS: usize = 60;

/// Number of emulation quanta per frame
/// Each frame is divided into multiple quanta to allow ARM9 and ARM11 to interleave execution
pub const QUANTUMS_PER_FRAME: usize = 10;

/// ARM11 instructions to execute per frame at 60fps
pub const ARM11_INSTRUCTIONS_PER_FRAME: usize = ARM11_FREQ_HZ / TARGET_FPS; // ~4,466,667

/// ARM9 instructions to execute per frame at 60fps
pub const ARM9_INSTRUCTIONS_PER_FRAME: usize = ARM9_FREQ_HZ / TARGET_FPS; // ~2,233,333

/// ARM11 instructions to execute per quantum
pub const ARM11_INSTRUCTIONS_PER_QUANTUM: usize = ARM11_INSTRUCTIONS_PER_FRAME / QUANTUMS_PER_FRAME; // ~446,667

/// ARM9 instructions to execute per quantum
pub const ARM9_INSTRUCTIONS_PER_QUANTUM: usize = ARM9_INSTRUCTIONS_PER_FRAME / QUANTUMS_PER_FRAME; // ~223,333

/// Result of running a single quantum
#[derive(Debug, Clone, PartialEq)]
pub enum QuantumResult {
    /// Quantum completed successfully, continue execution
    Continue,
    /// An error occurred during execution
    Error(String),
}

/// Configuration for the scheduler
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// ARM9 instructions per quantum
    pub arm9_quantum: usize,
    /// ARM11 instructions per quantum
    pub arm11_quantum: usize,
    /// Stop when ARM9 PC reaches this address
    pub arm9_stop_pc: Option<u64>,
    /// Stop when ARM11 PC reaches this address
    pub arm11_stop_pc: Option<u64>,
    /// Stop after this many total instructions
    pub max_instructions: Option<usize>,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            arm9_quantum: ARM9_INSTRUCTIONS_PER_QUANTUM,
            arm11_quantum: ARM11_INSTRUCTIONS_PER_QUANTUM,
            arm9_stop_pc: None,
            arm11_stop_pc: None,
            max_instructions: None,
        }
    }
}

/// Scheduler for interleaving ARM9 and ARM11 execution
pub struct Scheduler {
    config: SchedulerConfig,
    arm9_pc: u64,
    arm11_pc: u64,
    total_executed: usize,
    arm9_stopped: bool,
    arm11_stopped: bool,
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new(config: SchedulerConfig, arm9_entry: u64, arm11_entry: u64) -> Self {
        Self {
            config,
            arm9_pc: arm9_entry,
            arm11_pc: arm11_entry,
            total_executed: 0,
            arm9_stopped: false,
            arm11_stopped: false,
        }
    }

    /// Check if ARM9 is stopped
    pub fn arm9_stopped(&self) -> bool {
        self.arm9_stopped
    }

    /// Check if ARM11 is stopped
    pub fn arm11_stopped(&self) -> bool {
        self.arm11_stopped
    }

    /// Check if both cores are stopped
    pub fn all_stopped(&self) -> bool {
        self.arm9_stopped && self.arm11_stopped
    }

    /// Get the current ARM9 PC
    pub fn arm9_pc(&self) -> u64 {
        self.arm9_pc
    }

    /// Get the current ARM11 PC
    pub fn arm11_pc(&self) -> u64 {
        self.arm11_pc
    }

    /// Get total instructions executed
    pub fn total_executed(&self) -> usize {
        self.total_executed
    }

    /// Check if any stop condition is met
    pub fn check_stop_conditions(&self) -> bool {
        // If both cores are stopped, we're done
        if self.arm9_stopped && self.arm11_stopped {
            return true;
        }

        // Check ARM9 PC stop condition
        if let Some(stop_pc) = self.config.arm9_stop_pc
            && self.arm9_pc == stop_pc
        {
            return true;
        }

        // Check ARM11 PC stop condition
        if let Some(stop_pc) = self.config.arm11_stop_pc
            && self.arm11_pc == stop_pc
        {
            return true;
        }

        // Check max instructions
        if let Some(max) = self.config.max_instructions
            && self.total_executed >= max
        {
            return true;
        }

        false
    }

    /// Check if a specific PC matches any stop condition for ARM9
    fn is_arm9_stop_pc(&self, pc: u64) -> bool {
        self.config.arm9_stop_pc == Some(pc)
    }

    /// Check if a specific PC matches any stop condition for ARM11
    fn is_arm11_stop_pc(&self, pc: u64) -> bool {
        self.config.arm11_stop_pc == Some(pc)
    }

    /// Run a single quantum of execution for both cores
    pub fn run_quantum(
        &mut self,
        arm9_emu: &mut Unicorn<'static, mmio::EmulatorState>,
        arm11_emu: &mut Unicorn<'static, mmio::EmulatorState>,
    ) -> QuantumResult {
        // Run ARM9 quantum (only if not already stopped)
        if !self.arm9_stopped {
            let _span = tracing::error_span!("ARM9").entered();
            let arm9_stop = self.config.arm9_stop_pc.unwrap_or(u64::MAX);
            match arm9_emu.emu_start(self.arm9_pc, arm9_stop, 0, self.config.arm9_quantum) {
                Ok(_) => {
                    self.total_executed += self.config.arm9_quantum;
                    self.arm9_pc = arm9_emu.reg_read(RegisterARM::PC).unwrap();
                }
                Err(e) => {
                    self.arm9_pc = arm9_emu.reg_read(RegisterARM::PC).unwrap();
                    // Check if we hit a stop address - if so, mark as stopped rather than error
                    if self.is_arm9_stop_pc(self.arm9_pc) {
                        self.arm9_stopped = true;
                    } else {
                        error!("{:?}", e);
                        return QuantumResult::Error(format!("ARM9: {:?}", e));
                    }
                }
            }

            // Check if ARM9 hit a stop condition after successful execution
            if self.is_arm9_stop_pc(self.arm9_pc) {
                self.arm9_stopped = true;
            }
        }

        // Run ARM11 quantum (only if not already stopped)
        if !self.arm11_stopped {
            let _span = tracing::error_span!("ARM11").entered();
            let arm11_stop = self.config.arm11_stop_pc.unwrap_or(u64::MAX);
            match arm11_emu.emu_start(self.arm11_pc, arm11_stop, 0, self.config.arm11_quantum) {
                Ok(_) => {
                    self.total_executed += self.config.arm11_quantum;
                    self.arm11_pc = arm11_emu.reg_read(RegisterARM::PC).unwrap();
                }
                Err(e) => {
                    self.arm11_pc = arm11_emu.reg_read(RegisterARM::PC).unwrap();
                    // Check if we hit a stop address - if so, mark as stopped rather than error
                    if self.is_arm11_stop_pc(self.arm11_pc) {
                        self.arm11_stopped = true;
                    } else {
                        error!("{:?}", e);
                        return QuantumResult::Error(format!("ARM11: {:?}", e));
                    }
                }
            }

            // Check if ARM11 hit a stop condition after successful execution
            if self.is_arm11_stop_pc(self.arm11_pc) {
                self.arm11_stopped = true;
            }
        }

        QuantumResult::Continue
    }
}
