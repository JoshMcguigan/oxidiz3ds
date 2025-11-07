pub mod args;
pub mod bootrom;
pub mod core;
pub mod cp15;
pub mod cpu_types;
pub mod display;
pub mod firm;
pub mod memory;
pub mod mmio;
pub mod scheduler;

// Re-export commonly used types
pub use args::{Args, load_firm_data};
pub use core::{EmulatorConfig, EmulatorCore, StopReason};
pub use cpu_types::ArmRegister;
pub use mmio::{EmulatorState, GpuState, PixelFormat, SdmmcState};
pub use scheduler::{QuantumResult, SchedulerConfig};
