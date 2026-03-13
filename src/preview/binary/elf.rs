use super::{BinaryMetadata, ByteOrder, format_hex, read_u16, read_u32, read_u64};

pub(super) fn parse(bytes: &[u8]) -> Option<BinaryMetadata> {
    if bytes.get(0..4) != Some(b"\x7FELF".as_slice()) {
        return None;
    }

    let bits = match bytes.get(4).copied()? {
        1 => "32-bit",
        2 => "64-bit",
        _ => return None,
    };
    let order = match bytes.get(5).copied()? {
        1 => ByteOrder::Little,
        2 => ByteOrder::Big,
        _ => return None,
    };
    let elf_type = read_u16(bytes, 16, order)?;
    let machine = read_u16(bytes, 18, order)?;
    let entry_point = if bits == "32-bit" {
        read_u32(bytes, 24, order).map(|value| format_hex(u64::from(value)))
    } else {
        read_u64(bytes, 24, order).map(format_hex)
    };
    let section_count = if bits == "32-bit" {
        read_u16(bytes, 48, order).map(usize::from)
    } else {
        read_u16(bytes, 60, order).map(usize::from)
    };

    Some(BinaryMetadata {
        detail: type_detail(elf_type),
        format: "ELF",
        kind: type_name(elf_type).map(str::to_string),
        architecture: machine_name(machine).map(str::to_string),
        bits: Some(bits),
        endianness: Some(order.label()),
        abi: abi_name(bytes.get(7).copied()?).map(str::to_string),
        subsystem: None,
        entry_point,
        section_count,
        command_count: None,
    })
}

fn machine_name(machine: u16) -> Option<&'static str> {
    match machine {
        0x03 => Some("x86"),
        0x08 => Some("MIPS"),
        0x14 => Some("PowerPC"),
        0x15 => Some("PowerPC64"),
        0x16 => Some("S390"),
        0x28 => Some("ARM"),
        0x3E => Some("x86_64"),
        0xB7 => Some("AArch64"),
        0xF3 => Some("RISC-V"),
        0x102 => Some("LoongArch"),
        _ => None,
    }
}

fn type_name(elf_type: u16) -> Option<&'static str> {
    match elf_type {
        1 => Some("Relocatable object"),
        2 => Some("Executable"),
        3 => Some("Shared object"),
        4 => Some("Core dump"),
        _ => None,
    }
}

fn type_detail(elf_type: u16) -> &'static str {
    match elf_type {
        1 => "ELF relocatable object",
        2 => "ELF executable",
        3 => "ELF shared object",
        4 => "ELF core file",
        _ => "ELF binary",
    }
}

fn abi_name(abi: u8) -> Option<&'static str> {
    match abi {
        0 => Some("System V"),
        1 => Some("HP-UX"),
        2 => Some("NetBSD"),
        3 => Some("Linux"),
        6 => Some("Solaris"),
        9 => Some("FreeBSD"),
        12 => Some("OpenBSD"),
        _ => None,
    }
}
