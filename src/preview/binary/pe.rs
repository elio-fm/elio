use super::{BinaryMetadata, ByteOrder, format_hex, read_u16_le, read_u32_le};

pub(super) fn parse(bytes: &[u8]) -> Option<BinaryMetadata> {
    if bytes.get(0..2) != Some(b"MZ".as_slice()) {
        return None;
    }

    let pe_offset = read_u32_le(bytes, 0x3c)? as usize;
    if bytes.get(pe_offset..pe_offset + 4) != Some(b"PE\0\0".as_slice()) {
        return None;
    }

    let coff = pe_offset + 4;
    let machine = read_u16_le(bytes, coff)?;
    let section_count = usize::from(read_u16_le(bytes, coff + 2)?);
    let optional_size = usize::from(read_u16_le(bytes, coff + 16)?);
    let characteristics = read_u16_le(bytes, coff + 18)?;
    let optional = coff + 20;
    let optional_magic = read_u16_le(bytes, optional)?;

    let bits = match optional_magic {
        0x10b => Some("32-bit"),
        0x20b => Some("64-bit"),
        _ => None,
    };
    let subsystem_offset = match optional_magic {
        0x10b => optional + 68,
        0x20b => optional + 88,
        _ => optional,
    };
    let subsystem = if optional + optional_size >= subsystem_offset + 2 {
        read_u16_le(bytes, subsystem_offset).and_then(subsystem_name)
    } else {
        None
    };
    let entry_point = if optional + optional_size >= optional + 20 {
        read_u32_le(bytes, optional + 16).map(|value| format_hex(u64::from(value)))
    } else {
        None
    };

    let is_dll = characteristics & 0x2000 != 0;
    let kind = if is_dll {
        Some("Dynamic library".to_string())
    } else if characteristics & 0x0002 != 0 {
        Some("Executable".to_string())
    } else {
        Some("COFF image".to_string())
    };

    Some(BinaryMetadata {
        detail: if is_dll {
            "PE dynamic library"
        } else {
            "PE executable"
        },
        format: "PE/COFF",
        kind,
        architecture: machine_name(machine).map(str::to_string),
        bits,
        endianness: Some(ByteOrder::Little.label()),
        abi: None,
        subsystem: subsystem.map(str::to_string),
        entry_point,
        section_count: Some(section_count),
        command_count: None,
    })
}

pub(super) fn parse_dos_mz(bytes: &[u8]) -> Option<BinaryMetadata> {
    if bytes.get(0..2) != Some(b"MZ".as_slice()) {
        return None;
    }

    Some(BinaryMetadata {
        detail: "DOS executable",
        format: "MZ",
        kind: Some("Executable".to_string()),
        architecture: Some("x86".to_string()),
        bits: Some("16-bit"),
        endianness: Some(ByteOrder::Little.label()),
        abi: None,
        subsystem: None,
        entry_point: None,
        section_count: None,
        command_count: None,
    })
}

fn machine_name(machine: u16) -> Option<&'static str> {
    match machine {
        0x014c => Some("x86"),
        0x8664 => Some("x86_64"),
        0x01c0 => Some("ARM"),
        0x01c4 => Some("ARMv7"),
        0xaa64 => Some("AArch64"),
        0x5032 => Some("RISC-V 32"),
        0x5064 => Some("RISC-V 64"),
        0x5128 => Some("RISC-V 128"),
        _ => None,
    }
}

fn subsystem_name(subsystem: u16) -> Option<&'static str> {
    match subsystem {
        1 => Some("Native"),
        2 => Some("GUI"),
        3 => Some("Console"),
        5 => Some("OS/2 console"),
        7 => Some("POSIX console"),
        9 => Some("Windows CE"),
        10 => Some("EFI application"),
        11 => Some("EFI boot service driver"),
        12 => Some("EFI runtime driver"),
        13 => Some("EFI ROM"),
        14 => Some("Xbox"),
        16 => Some("Windows boot application"),
        _ => None,
    }
}
