use super::{read_u32, BinaryMetadata, ByteOrder};

pub(super) fn parse(bytes: &[u8]) -> Option<BinaryMetadata> {
    match bytes.get(0..4)? {
        [0xfe, 0xed, 0xfa, 0xce] => parse_regular(bytes, ByteOrder::Big, "32-bit"),
        [0xce, 0xfa, 0xed, 0xfe] => parse_regular(bytes, ByteOrder::Little, "32-bit"),
        [0xfe, 0xed, 0xfa, 0xcf] => parse_regular(bytes, ByteOrder::Big, "64-bit"),
        [0xcf, 0xfa, 0xed, 0xfe] => parse_regular(bytes, ByteOrder::Little, "64-bit"),
        [0xca, 0xfe, 0xba, 0xbe] => parse_fat(bytes, ByteOrder::Big),
        [0xbe, 0xba, 0xfe, 0xca] => parse_fat(bytes, ByteOrder::Little),
        _ => None,
    }
}

fn parse_regular(bytes: &[u8], order: ByteOrder, bits: &'static str) -> Option<BinaryMetadata> {
    let cpu_type = read_u32(bytes, 4, order)?;
    let file_type = read_u32(bytes, 12, order)?;
    let command_count = usize::try_from(read_u32(bytes, 16, order)?).ok()?;

    Some(BinaryMetadata {
        detail: file_type_detail(file_type),
        format: "Mach-O",
        kind: file_type_name(file_type).map(str::to_string),
        architecture: cpu_name(cpu_type).map(str::to_string),
        bits: Some(bits),
        endianness: Some(order.label()),
        abi: None,
        subsystem: None,
        entry_point: None,
        section_count: None,
        command_count: Some(command_count),
    })
}

fn parse_fat(bytes: &[u8], order: ByteOrder) -> Option<BinaryMetadata> {
    let arch_count = usize::try_from(read_u32(bytes, 4, order)?).ok()?;
    let mut architectures = Vec::new();
    for index in 0..arch_count.min(4) {
        let offset = 8 + index * 20;
        let cpu_type = read_u32(bytes, offset, order)?;
        if let Some(name) = cpu_name(cpu_type) {
            architectures.push(name);
        }
    }

    Some(BinaryMetadata {
        detail: "Mach-O universal binary",
        format: "Mach-O (fat)",
        kind: Some("Universal binary".to_string()),
        architecture: (!architectures.is_empty()).then(|| architectures.join(", ")),
        bits: None,
        endianness: Some(order.label()),
        abi: None,
        subsystem: None,
        entry_point: None,
        section_count: Some(arch_count),
        command_count: None,
    })
}

fn cpu_name(cpu_type: u32) -> Option<&'static str> {
    match cpu_type {
        7 => Some("x86"),
        0x01000007 => Some("x86_64"),
        12 => Some("ARM"),
        0x0100000C => Some("ARM64"),
        18 => Some("PowerPC"),
        0x01000012 => Some("PowerPC64"),
        _ => None,
    }
}

fn file_type_name(file_type: u32) -> Option<&'static str> {
    match file_type {
        1 => Some("Relocatable object"),
        2 => Some("Executable"),
        6 => Some("Dynamic library"),
        8 => Some("Bundle"),
        11 => Some("Dynamic linker"),
        12 => Some("Kernel extension"),
        _ => None,
    }
}

fn file_type_detail(file_type: u32) -> &'static str {
    match file_type {
        1 => "Mach-O relocatable object",
        2 => "Mach-O executable",
        6 => "Mach-O dynamic library",
        8 => "Mach-O bundle",
        11 => "Mach-O dynamic linker",
        12 => "Mach-O kernel extension",
        _ => "Mach-O binary",
    }
}
