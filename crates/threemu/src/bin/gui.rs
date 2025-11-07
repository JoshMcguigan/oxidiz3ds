use clap::Parser;
use threemu::{Args, EmulatorCore, display, load_firm_data};
use tracing::info;

fn main() {
    // Parse command-line arguments
    let args = Args::parse();

    // Validate arguments
    if let Err(e) = args.validate() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Load FIRM data
    let firm_data =
        load_firm_data(&args).unwrap_or_else(|e| panic!("Failed to load FIRM file: {}", e));

    // Create emulator config from args
    let config = args.to_emulator_config();

    // Create emulator
    info!("=== Creating Emulator ===");
    let emulator = EmulatorCore::new(&firm_data, config)
        .unwrap_or_else(|e| panic!("Failed to create emulator: {}", e));

    // Run with display
    info!("=== Starting Emulator with Display ===");
    info!("ARM9 Entry: {:#X}", emulator.arm9_pc());
    info!("ARM11 Entry: {:#X}", emulator.arm11_pc());

    display::run(emulator).expect("Failed to run display");
}
