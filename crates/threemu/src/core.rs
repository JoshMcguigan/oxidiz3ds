//! Core emulator API for 3DS emulation.
//!
//! This module provides the main emulator interface that can be used both
//! for headless testing and as the backend for graphical frontends.

use crate::firm::FirmHeader;
use crate::memory::{self, ARM9_PRIVATE_WRAM_SIZE, AXI_WRAM_SIZE, FCRAM_SIZE, VRAM_SIZE};
use crate::mmio;
use crate::scheduler::{QuantumResult, Scheduler, SchedulerConfig};
use crate::{bootrom, cp15};
use std::path::PathBuf;
use std::time::Instant;
use tracing::info;
use unicorn_engine::{
    RegisterARM, Unicorn,
    unicorn_const::{Arch, Mode, Prot},
};

/// Configuration for the emulator
#[derive(Debug, Clone, Default)]
pub struct EmulatorConfig {
    /// Optional SD card image path
    pub sd_card: Option<PathBuf>,
    /// Stop when ARM9 PC reaches this address
    pub arm9_stop_pc: Option<u64>,
    /// Stop when ARM11 PC reaches this address
    pub arm11_stop_pc: Option<u64>,
    /// Stop after this many total instructions
    pub max_instructions: Option<usize>,
    /// Optional timeout in milliseconds
    pub timeout_ms: Option<u64>,
}

/// Result of running the emulator
#[derive(Debug, Clone, PartialEq)]
pub enum StopReason {
    /// Reached a stop condition (PC match, max instructions)
    StopCondition,
    /// Timeout reached
    Timeout,
    /// Emulation error occurred
    Error(String),
}

/// Core emulator for 3DS
pub struct EmulatorCore {
    arm9_emu: Unicorn<'static, mmio::EmulatorState>,
    arm11_emu: Unicorn<'static, mmio::EmulatorState>,
    scheduler: Scheduler,

    // Shared memory (leaked for 'static lifetime)
    fcram: &'static mut [u8],
    vram: &'static mut [u8],

    // Configuration
    timeout_ms: Option<u64>,
    start_time: Instant,
}

impl EmulatorCore {
    /// Create a new emulator from FIRM data
    pub fn new(firm_data: &[u8], config: EmulatorConfig) -> Result<Self, String> {
        // Parse FIRM header
        let firm =
            FirmHeader::parse(firm_data).map_err(|e| format!("Failed to parse FIRM: {:?}", e))?;

        info!("FIRM Magic: {}", String::from_utf8_lossy(&firm.magic));
        info!("ARM11 Entry: {:#X}", firm.arm11_entrypoint);
        info!("ARM9 Entry: {:#X}", firm.arm9_entrypoint);

        // Create shared backing memory (leaked to get 'static lifetime)
        // These are shared between ARM9 and ARM11, so we use raw pointers to allow
        // passing to both emulators.
        info!("=== Creating Shared Memory ===");
        let fcram: &'static mut [u8] = Box::leak(vec![0u8; FCRAM_SIZE].into_boxed_slice());
        let vram: &'static mut [u8] = Box::leak(vec![0u8; VRAM_SIZE].into_boxed_slice());
        let axi_wram: &'static mut [u8] = Box::leak(vec![0u8; AXI_WRAM_SIZE].into_boxed_slice());
        let arm9_private_wram: &'static mut [u8] =
            Box::leak(vec![0u8; ARM9_PRIVATE_WRAM_SIZE].into_boxed_slice());
        info!(
            "Allocated FCRAM ({}MB), VRAM ({}MB), AXI WRAM ({}KB), and ARM9 private WRAM ({}KB)",
            FCRAM_SIZE / (1024 * 1024),
            VRAM_SIZE / (1024 * 1024),
            AXI_WRAM_SIZE / 1024,
            ARM9_PRIVATE_WRAM_SIZE / 1024
        );

        // Get raw pointers for shared memory regions that need to be mapped to both emulators
        // SAFETY: These pointers are leaked and will remain valid for the lifetime of the program.
        // Both ARM9 and ARM11 map the same physical memory regions, which is intentional.
        let fcram_ptr = fcram.as_mut_ptr();
        let vram_ptr = vram.as_mut_ptr();
        let axi_wram_ptr = axi_wram.as_mut_ptr();

        // Create shared emulator state
        let emu_state = mmio::EmulatorState::new(config.sd_card.clone());

        // Initialize ARM11 emulator
        info!("=== ARM11 Setup ===");
        let mut arm11_emu = Unicorn::new_with_data(Arch::ARM, Mode::LITTLE_ENDIAN, emu_state)
            .map_err(|e| format!("Failed to initialize ARM11: {:?}", e))?;

        // SAFETY: We're intentionally sharing memory between emulators
        unsafe {
            let fcram_slice = std::slice::from_raw_parts_mut(fcram_ptr, FCRAM_SIZE);
            let vram_slice = std::slice::from_raw_parts_mut(vram_ptr, VRAM_SIZE);
            let axi_wram_slice = std::slice::from_raw_parts_mut(axi_wram_ptr, AXI_WRAM_SIZE);
            memory::setup_arm11_memory(&mut arm11_emu, fcram_slice, axi_wram_slice, vram_slice);
        }
        memory::load_sections(&mut arm11_emu, &firm.sections, firm_data, false);

        arm11_emu
            .reg_write(RegisterARM::R0, 123)
            .expect("failed to write to R0");
        arm11_emu
            .reg_write(RegisterARM::R5, 1337)
            .expect("failed to write to R5");

        // Initialize ARM9 emulator
        info!("=== ARM9 Setup ===");
        let mut arm9_emu = Unicorn::new_with_data(
            Arch::ARM,
            Mode::LITTLE_ENDIAN,
            mmio::EmulatorState::new(config.sd_card.clone()),
        )
        .map_err(|e| format!("Failed to initialize ARM9: {:?}", e))?;

        // SAFETY: We're intentionally sharing memory between emulators
        unsafe {
            let fcram_slice = std::slice::from_raw_parts_mut(fcram_ptr, FCRAM_SIZE);
            let vram_slice = std::slice::from_raw_parts_mut(vram_ptr, VRAM_SIZE);
            let axi_wram_slice = std::slice::from_raw_parts_mut(axi_wram_ptr, AXI_WRAM_SIZE);
            memory::setup_arm9_memory(
                &mut arm9_emu,
                fcram_slice,
                axi_wram_slice,
                vram_slice,
                arm9_private_wram,
            );
        }
        memory::load_sections(&mut arm9_emu, &firm.sections, firm_data, true);

        // Add CP15 hook for ARM9
        arm9_emu
            .add_code_hook(0, u64::MAX, |uc, addr, _size| {
                let mut insn_bytes = [0u8; 4];
                if uc.mem_read(addr, &mut insn_bytes).is_ok() {
                    let insn = u32::from_le_bytes(insn_bytes);
                    cp15::handle_cp15_instruction(uc, addr, insn);
                }
            })
            .map_err(|e| format!("Failed to add CP15 hook: {:?}", e))?;

        // Add bootrom hooks for ARM9
        arm9_emu
            .mem_map(
                bootrom::ARM9_REGION_START as u64,
                bootrom::ARM9_REGION_LEN as u64,
                Prot::ALL,
            )
            .map_err(|e| format!("Failed to map bootrom: {:?}", e))?;
        arm9_emu
            .add_code_hook(
                bootrom::ARM9_REGION_START as u64,
                bootrom::ARM9_REGION_END as u64,
                |uc, addr, _size| {
                    bootrom::handle_instruction(uc, addr.try_into().expect("addr must be 32 bit"));
                },
            )
            .map_err(|e| format!("Failed to add bootrom hook: {:?}", e))?;

        // Create scheduler
        let scheduler_config = SchedulerConfig {
            arm9_stop_pc: config.arm9_stop_pc,
            arm11_stop_pc: config.arm11_stop_pc,
            max_instructions: config.max_instructions,
            ..Default::default()
        };
        let scheduler = Scheduler::new(
            scheduler_config,
            firm.arm9_entrypoint as u64,
            firm.arm11_entrypoint as u64,
        );

        Ok(Self {
            arm9_emu,
            arm11_emu,
            scheduler,
            fcram,
            vram,
            timeout_ms: config.timeout_ms,
            start_time: Instant::now(),
        })
    }

    /// Run a single quantum of execution
    pub fn step(&mut self) -> QuantumResult {
        self.scheduler
            .run_quantum(&mut self.arm9_emu, &mut self.arm11_emu)
    }

    /// Check if any stop condition is met
    pub fn should_stop(&self) -> bool {
        // Check scheduler stop conditions
        if self.scheduler.check_stop_conditions() {
            return true;
        }

        // Check timeout
        if let Some(timeout_ms) = self.timeout_ms {
            let elapsed_ms = self.start_time.elapsed().as_millis() as u64;
            if elapsed_ms >= timeout_ms {
                info!("Timeout reached: {} ms", elapsed_ms);
                return true;
            }
        }

        false
    }

    /// Run until a stop condition is reached
    pub fn run(&mut self) -> StopReason {
        loop {
            // Check stop conditions first
            if self.should_stop() {
                return StopReason::StopCondition;
            }

            // Run a quantum
            match self.step() {
                QuantumResult::Continue => {}
                QuantumResult::Error(e) => return StopReason::Error(e),
            }
        }
    }

    /// Get the current ARM9 PC
    pub fn arm9_pc(&self) -> u64 {
        self.scheduler.arm9_pc()
    }

    /// Get the current ARM11 PC
    pub fn arm11_pc(&self) -> u64 {
        self.scheduler.arm11_pc()
    }

    /// Check if ARM9 has stopped (reached a stop PC)
    pub fn arm9_stopped(&self) -> bool {
        self.scheduler.arm9_stopped()
    }

    /// Check if ARM11 has stopped (reached a stop PC)
    pub fn arm11_stopped(&self) -> bool {
        self.scheduler.arm11_stopped()
    }

    /// Get total instructions executed
    pub fn total_executed(&self) -> usize {
        self.scheduler.total_executed()
    }

    /// Get elapsed time since start
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Read an ARM9 register
    pub fn arm9_reg(&self, reg: RegisterARM) -> u64 {
        self.arm9_emu.reg_read(reg).unwrap_or(0)
    }

    /// Read an ARM11 register
    pub fn arm11_reg(&self, reg: RegisterARM) -> u64 {
        self.arm11_emu.reg_read(reg).unwrap_or(0)
    }

    /// Get a reference to the ARM11 emulator (for GPU state access)
    pub fn arm11_emu(&self) -> &Unicorn<'static, mmio::EmulatorState> {
        &self.arm11_emu
    }

    /// Get a reference to the ARM9 emulator
    pub fn arm9_emu(&self) -> &Unicorn<'static, mmio::EmulatorState> {
        &self.arm9_emu
    }

    /// Get FCRAM buffer
    pub fn fcram(&self) -> &[u8] {
        self.fcram
    }

    /// Get VRAM buffer
    pub fn vram(&self) -> &[u8] {
        self.vram
    }

    /// Read memory from ARM9's perspective
    pub fn arm9_mem_read(&self, addr: u64, size: usize) -> Result<Vec<u8>, String> {
        let mut buf = vec![0u8; size];
        self.arm9_emu
            .mem_read(addr, &mut buf)
            .map_err(|e| format!("ARM9 mem read error: {:?}", e))?;
        Ok(buf)
    }

    /// Read memory from ARM11's perspective
    pub fn arm11_mem_read(&self, addr: u64, size: usize) -> Result<Vec<u8>, String> {
        let mut buf = vec![0u8; size];
        self.arm11_emu
            .mem_read(addr, &mut buf)
            .map_err(|e| format!("ARM11 mem read error: {:?}", e))?;
        Ok(buf)
    }

    /// Print final emulator state
    pub fn print_final_state(&self) {
        info!("Total instructions executed: {}", self.total_executed());
        info!("Elapsed time: {:.2?}", self.elapsed());

        // Read ARM9 registers
        let arm9_r0 = self.arm9_reg(RegisterARM::R0);
        let arm9_r1 = self.arm9_reg(RegisterARM::R1);
        let arm9_r2 = self.arm9_reg(RegisterARM::R2);
        let arm9_r3 = self.arm9_reg(RegisterARM::R3);
        let arm9_r4 = self.arm9_reg(RegisterARM::R4);
        let arm9_r5 = self.arm9_reg(RegisterARM::R5);
        let arm9_r6 = self.arm9_reg(RegisterARM::R6);
        let arm9_sp = self.arm9_reg(RegisterARM::SP);
        let arm9_lr = self.arm9_reg(RegisterARM::LR);

        // Read ARM11 registers
        let arm11_r0 = self.arm11_reg(RegisterARM::R0);
        let arm11_r1 = self.arm11_reg(RegisterARM::R1);
        let arm11_r2 = self.arm11_reg(RegisterARM::R2);
        let arm11_r3 = self.arm11_reg(RegisterARM::R3);
        let arm11_r4 = self.arm11_reg(RegisterARM::R4);
        let arm11_r5 = self.arm11_reg(RegisterARM::R5);
        let arm11_r6 = self.arm11_reg(RegisterARM::R6);
        let arm11_sp = self.arm11_reg(RegisterARM::SP);
        let arm11_lr = self.arm11_reg(RegisterARM::LR);

        info!(
            "ARM9: pc={:#x} r0={:#x} r1={:#x} r2={:#x} r3={:#x} r4={:#x} r5={:#x} r6={:#x} sp={:#x} lr={:#x}",
            self.arm9_pc(),
            arm9_r0,
            arm9_r1,
            arm9_r2,
            arm9_r3,
            arm9_r4,
            arm9_r5,
            arm9_r6,
            arm9_sp,
            arm9_lr
        );

        info!(
            "ARM11: pc={:#x} r0={:#x} r1={:#x} r2={:#x} r3={:#x} r4={:#x} r5={:#x} r6={:#x} sp={:#x} lr={:#x}",
            self.arm11_pc(),
            arm11_r0,
            arm11_r1,
            arm11_r2,
            arm11_r3,
            arm11_r4,
            arm11_r5,
            arm11_r6,
            arm11_sp,
            arm11_lr
        );
    }
}
