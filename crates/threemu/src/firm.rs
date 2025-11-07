/// Errors that can occur during FIRM parsing
#[derive(Debug)]
pub enum FirmError {
    /// File is too small to contain a valid FIRM header
    FileTooSmall,
    /// FIRM magic bytes are invalid (not "FIRM")
    InvalidMagic,
}

/// FIRM section header describing a loadable firmware section
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FirmSectionHeader {
    /// Byte offset to section data within the FIRM file
    pub offset: u32,
    /// Physical memory address where this section should be loaded
    pub load_address: u32,
    /// Size of the section in bytes (0 indicates section doesn't exist)
    pub size: u32,
    /// Copy method: 0=NDMA, 1=XDMA, 2=memcpy
    pub copy_method: u32,
    /// SHA-256 hash of the section data
    pub hash: [u8; 32],
}

/// FIRM format header containing ARM9/ARM11 entry points and section info
#[repr(C)]
#[derive(Debug)]
pub struct FirmHeader {
    /// Magic identifier, should be "FIRM"
    pub magic: [u8; 4],
    /// Boot priority (higher value = max priority, typically zero)
    pub boot_priority: u32,
    /// ARM11 kernel entry point address
    pub arm11_entrypoint: u32,
    /// ARM9 kernel entry point address
    pub arm9_entrypoint: u32,
    /// Reserved space
    pub reserved: [u8; 0x30],
    /// Four firmware section headers (may be unused if size=0)
    pub sections: [FirmSectionHeader; 4],
    /// RSA-2048 signature of header SHA-256 hash
    pub signature: [u8; 0x100],
}

impl FirmHeader {
    /// Parse a FIRM header from raw file data
    pub fn parse(data: &[u8]) -> Result<Self, FirmError> {
        if data.len() < 0x200 {
            return Err(FirmError::FileTooSmall);
        }

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&data[0x000..0x004]);

        if &magic != b"FIRM" {
            return Err(FirmError::InvalidMagic);
        }

        let boot_priority = u32::from_le_bytes(data[0x004..0x008].try_into().unwrap());
        let arm11_entrypoint = u32::from_le_bytes(data[0x008..0x00C].try_into().unwrap());
        let arm9_entrypoint = u32::from_le_bytes(data[0x00C..0x010].try_into().unwrap());

        let mut reserved = [0u8; 0x30];
        reserved.copy_from_slice(&data[0x010..0x040]);

        // Parse section headers
        let mut sections = [FirmSectionHeader {
            offset: 0,
            load_address: 0,
            size: 0,
            copy_method: 0,
            hash: [0u8; 32],
        }; 4];

        for (i, section) in sections.iter_mut().enumerate() {
            let base = 0x040 + (i * 0x30);
            section.offset = u32::from_le_bytes(data[base..base + 4].try_into().unwrap());
            section.load_address = u32::from_le_bytes(data[base + 4..base + 8].try_into().unwrap());
            section.size = u32::from_le_bytes(data[base + 8..base + 12].try_into().unwrap());
            section.copy_method =
                u32::from_le_bytes(data[base + 12..base + 16].try_into().unwrap());
            section.hash.copy_from_slice(&data[base + 16..base + 48]);
        }

        let mut signature = [0u8; 0x100];
        signature.copy_from_slice(&data[0x100..0x200]);

        Ok(FirmHeader {
            magic,
            boot_priority,
            arm11_entrypoint,
            arm9_entrypoint,
            reserved,
            sections,
            signature,
        })
    }
}
