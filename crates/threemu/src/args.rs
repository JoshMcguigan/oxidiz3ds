use crate::EmulatorConfig;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// Path to FIRM file to execute. If --entry-firm-in-sd-card is set,
    /// this is a path inside the SD card image (e.g., "luma/payloads/firm.firm").
    /// Otherwise, it's a path on the local filesystem.
    pub firm: PathBuf,

    /// Path to SD card image (raw disk image with MBR + FAT32)
    #[arg(long)]
    pub sd_card: Option<PathBuf>,

    /// Interpret FIRM path as a path inside the SD card image instead of local filesystem.
    /// Requires --sd-card to be specified.
    #[arg(long)]
    pub entry_firm_in_sd_card: bool,

    /// Stop when ARM9 reaches this PC (hex: 0x1234 or decimal: 1234)
    #[arg(long, value_parser = parse_hex_or_dec)]
    pub arm9_stop_pc: Option<u64>,

    /// Stop when ARM11 reaches this PC (hex: 0x1234 or decimal: 1234)
    #[arg(long, value_parser = parse_hex_or_dec)]
    pub arm11_stop_pc: Option<u64>,

    /// Stop after this many instructions (total across both cores)
    #[arg(long, short = 'i')]
    pub max_instructions: Option<u64>,
}

impl Args {
    /// Validate that the arguments are consistent
    pub fn validate(&self) -> Result<(), String> {
        if self.entry_firm_in_sd_card && self.sd_card.is_none() {
            return Err("--entry-firm-in-sd-card requires --sd-card to be specified".to_string());
        }
        Ok(())
    }

    /// Convert Args to EmulatorConfig
    pub fn to_emulator_config(&self) -> EmulatorConfig {
        EmulatorConfig {
            sd_card: self.sd_card.clone(),
            arm9_stop_pc: self.arm9_stop_pc,
            arm11_stop_pc: self.arm11_stop_pc,
            max_instructions: self.max_instructions.map(|v| v as usize),
            timeout_ms: None,
        }
    }
}

pub fn parse_hex_or_dec(s: &str) -> Result<u64, std::num::ParseIntError> {
    if let Some(hex) = s.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
    } else {
        s.parse()
    }
}

/// Load FIRM data from either a direct file path or from inside an SD card image
pub fn load_firm_data(args: &Args) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::io::Read;
    use tracing::info;

    if args.entry_firm_in_sd_card {
        // Load from SD card image using fatfs
        let sd_card_path = args
            .sd_card
            .as_ref()
            .ok_or("--entry-firm-in-sd-card requires --sd-card")?;

        info!(
            "Loading FIRM from SD card image: {:?} at path: {:?}",
            sd_card_path, args.firm
        );

        use fscommon::BufStream;

        let img_file = std::fs::File::open(sd_card_path)?;
        let buf_stream = BufStream::new(img_file);
        let fs = fatfs::FileSystem::new(buf_stream, fatfs::FsOptions::new())?;
        let root_dir = fs.root_dir();

        // Convert PathBuf to string for fatfs
        let firm_path_str = args
            .firm
            .to_str()
            .ok_or("FIRM path contains invalid UTF-8")?;
        let mut firm_file = root_dir.open_file(firm_path_str)?;
        let mut contents = Vec::new();
        firm_file.read_to_end(&mut contents)?;

        info!("Successfully loaded {} bytes from SD card", contents.len());
        Ok(contents)
    } else {
        // Load directly from filesystem
        info!("Loading FIRM from file: {:?}", args.firm);
        let data = std::fs::read(&args.firm)?;
        Ok(data)
    }
}
