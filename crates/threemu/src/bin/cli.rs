use clap::Parser;
use threemu::{Args, EmulatorCore, StopReason, load_firm_data};
use tracing::info;

fn main() {
    // Parse command-line arguments
    let args = Args::parse();

    // Validate arguments
    if let Err(e) = args.validate() {
        eprintln!("Error: {}", e);
        std::process::exit(2);
    }

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Load FIRM data
    let firm_data = match load_firm_data(&args) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Failed to load FIRM file: {}", e);
            std::process::exit(2);
        }
    };

    // Create emulator config from args
    let config = args.to_emulator_config();

    // Create emulator
    info!("=== Creating Emulator ===");
    let mut emulator = match EmulatorCore::new(&firm_data, config) {
        Ok(emu) => emu,
        Err(e) => {
            eprintln!("Failed to create emulator: {}", e);
            std::process::exit(2);
        }
    };

    // Log entry points
    info!("ARM9 Entry: {:#X}", emulator.arm9_pc());
    info!("ARM11 Entry: {:#X}", emulator.arm11_pc());

    // Run emulator
    info!("=== Running Emulator (Headless) ===");
    let stop_reason = emulator.run();

    // Log final state
    info!("=== Emulation Complete ===");
    info!("Stop reason: {:?}", stop_reason);
    info!(
        "ARM9 PC: {:#X} (stopped: {})",
        emulator.arm9_pc(),
        emulator.arm9_stopped()
    );
    info!(
        "ARM11 PC: {:#X} (stopped: {})",
        emulator.arm11_pc(),
        emulator.arm11_stopped()
    );
    info!("Total instructions: {}", emulator.total_executed());
    info!("Elapsed: {:?}", emulator.elapsed());

    // Determine exit code based on stop reason and whether expectations were met
    let exit_code = match stop_reason {
        StopReason::Error(msg) => {
            eprintln!("Emulator error: {}", msg);
            2
        }
        StopReason::Timeout => {
            eprintln!("Timeout reached before stop conditions met");
            1
        }
        StopReason::StopCondition => {
            // Check if the expected stop PCs were reached
            let arm9_ok = args
                .arm9_stop_pc
                .is_none_or(|expected| emulator.arm9_stopped() && emulator.arm9_pc() == expected);
            let arm11_ok = args
                .arm11_stop_pc
                .is_none_or(|expected| emulator.arm11_stopped() && emulator.arm11_pc() == expected);

            if arm9_ok && arm11_ok {
                info!("PASS: All stop conditions reached");
                0
            } else {
                // This means max_instructions was hit before both PCs were reached
                if !arm9_ok {
                    eprintln!(
                        "ARM9 did not reach expected PC {:#X} (actual: {:#X}, stopped: {})",
                        args.arm9_stop_pc.unwrap(),
                        emulator.arm9_pc(),
                        emulator.arm9_stopped()
                    );
                }
                if !arm11_ok {
                    eprintln!(
                        "ARM11 did not reach expected PC {:#X} (actual: {:#X}, stopped: {})",
                        args.arm11_stop_pc.unwrap(),
                        emulator.arm11_pc(),
                        emulator.arm11_stopped()
                    );
                }
                1
            }
        }
    };

    std::process::exit(exit_code);
}
