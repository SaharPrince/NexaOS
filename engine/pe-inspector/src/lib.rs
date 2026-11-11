use serde::Serialize;
use std::fmt;

const MAX_SECTIONS: usize = 96;
const MAX_DIRECTORIES: usize = 16;

#[derive(Debug, Clone, Serialize)]
pub struct PeReport {
    pub format: String,
    pub execution: &'static str,
    pub machine: String,
    pub machine_raw: u16,
    pub sections: Vec<Section>,
    pub image_base: u64,
    pub entry_point_rva: u32,
    pub subsystem: String,
    pub directories: Vec<DataDirectory>,
    pub imports: Vec<ImportLibrary>,
    pub exports: Vec<String>,
    pub relocation_blocks: usize,
    pub relocation_entries: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct Section {
    pub name: String,
    pub virtual_size: u32,
    pub virtual_address: u32,
    pub raw_size: u32,
    pub raw_offset: u32,
    pub characteristics: u32,
}
#[derive(Debug, Clone, Serialize)]
pub struct DataDirectory {
    pub index: usize,
    pub name: String,
    /// Address value from IMAGE_DATA_DIRECTORY. This is a file offset for
    /// IMAGE_DIRECTORY_ENTRY_SECURITY and an RVA for other standard entries.
    pub address: u32,
    pub address_kind: String,
    pub size: u32,
}
#[derive(Debug, Clone, Serialize)]
pub struct ImportLibrary {
    pub name: String,
    pub symbols: Vec<String>,
}

#[derive(Debug)]
pub enum PeError {
    Truncated(&'static str),
    Invalid(&'static str),
    Unsupported(&'static str),
    OutOfBounds(&'static str),
}
impl fmt::Display for PeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated(s) => write!(f, "truncated file while reading {s}"),
            Self::Invalid(s) => write!(f, "invalid PE structure: {s}"),
            Self::Unsupported(s) => write!(f, "unsupported PE feature: {s}"),
            Self::OutOfBounds(s) => write!(f, "file range out of bounds: {s}"),
        }
    }
}
impl std::error::Error for PeError {}
type Result<T> = std::result::Result<T, PeError>;

fn bytes<'a>(d: &'a [u8], o: usize, n: usize, what: &'static str) -> Result<&'a [u8]> {
    let end = o.checked_add(n).ok_or(PeError::OutOfBounds(what))?;
    d.get(o..end).ok_or(PeError::Truncated(what))
}
fn u16at(d: &[u8], o: usize, w: &'static str) -> Result<u16> {
    Ok(u16::from_le_bytes(bytes(d, o, 2, w)?.try_into().unwrap()))
}
fn u32at(d: &[u8], o: usize, w: &'static str) -> Result<u32> {
    Ok(u32::from_le_bytes(bytes(d, o, 4, w)?.try_into().unwrap()))
}
fn u64at(d: &[u8], o: usize, w: &'static str) -> Result<u64> {
    Ok(u64::from_le_bytes(bytes(d, o, 8, w)?.try_into().unwrap()))
}
fn cstr(d: &[u8], o: usize, available: usize, max: usize, w: &'static str) -> Result<String> {
    let b = bytes(d, o, available.min(max), w)?;
    let end = b.iter().position(|x| *x == 0).ok_or(PeError::Invalid(w))?;
    Ok(String::from_utf8_lossy(&b[..end]).into_owned())
}
fn ranges_overlap(a_start: u64, a_end: u64, b_start: u64, b_end: u64) -> bool {
    a_start < b_end && b_start < a_end
}

fn machine(v: u16) -> String {
    match v {
        0x014c => "x86",
        0x8664 => "AMD64",
        0x01c0 | 0x01c4 => "ARM",
        0xaa64 => "ARM64",
        _ => "Unknown",
    }
    .into()
}
fn dirname(i: usize) -> String {
    match i {
        0 => "Export",
        1 => "Import",
        2 => "Resource",
        3 => "Exception",
        4 => "Security",
        5 => "Base Relocation",
        6 => "Debug",
        9 => "TLS",
        10 => "Load Config",
        12 => "IAT",
        13 => "Delay Import",
        _ => "Other",
    }
    .into()
}
fn subsystem(v: u16) -> String {
    match v {
        1 => "Native",
        2 => "Windows GUI",
        3 => "Windows Console",
        7 => "POSIX Console",
        9 => "Windows CE GUI",
        10 => "EFI Application",
        11 => "EFI Boot Service Driver",
        12 => "EFI Runtime Driver",
        14 => "Xbox",
        _ => "Unknown",
    }
    .into()
}

pub fn inspect(d: &[u8]) -> Result<PeReport> {
    if bytes(d, 0, 2, "DOS signature")? != b"MZ" {
        return Err(PeError::Invalid("missing MZ signature"));
    }
    let pe = u32at(d, 0x3c, "e_lfanew")? as usize;
    if bytes(d, pe, 4, "PE signature")? != b"PE\0\0" {
        return Err(PeError::Invalid("missing PE signature"));
    }
    let coff = pe
        .checked_add(4)
        .ok_or(PeError::OutOfBounds("COFF header"))?;
    let mach = u16at(d, coff, "machine")?;
    let nsec = u16at(d, coff + 2, "section count")? as usize;
    if nsec > MAX_SECTIONS {
        return Err(PeError::Invalid("section count exceeds safety limit"));
    }
    let opt_size = u16at(d, coff + 16, "optional header size")? as usize;
    let opt = coff
        .checked_add(20)
        .ok_or(PeError::OutOfBounds("optional header"))?;
    bytes(d, opt, opt_size, "optional header")?;
    let magic = u16at(d, opt, "optional header magic")?;
    let (fmt, base_off, count_off, dirs_off, min_size) = match magic {
        0x10b => ("PE32", opt + 28, opt + 92, opt + 96, 96usize),
        0x20b => ("PE32+", opt + 24, opt + 108, opt + 112, 112usize),
        _ => return Err(PeError::Unsupported("optional header magic")),
    };
    if opt_size < min_size {
        return Err(PeError::Truncated("required optional header fields"));
    }
    let entry = u32at(d, opt + 16, "entry point")?;
    let image_base = if magic == 0x20b {
        u64at(d, base_off, "image base")?
    } else {
        u32at(d, base_off, "image base")? as u64
    };
    let sub = subsystem(u16at(d, opt + 68, "subsystem")?);
    let declared_directories = u32at(d, count_off, "directory count")? as usize;
    let supported_directories = declared_directories.min(MAX_DIRECTORIES);
    let available_directories = opt_size.saturating_sub(dirs_off - opt) / 8;
    if supported_directories > available_directories {
        return Err(PeError::Truncated("data directory entries"));
    }
    let ndirs = supported_directories;
    let st = opt
        .checked_add(opt_size)
        .ok_or(PeError::OutOfBounds("section table"))?;
    bytes(
        d,
        st,
        nsec.checked_mul(40)
            .ok_or(PeError::OutOfBounds("section table"))?,
        "section table",
    )?;
    let mut sections = Vec::with_capacity(nsec);
    for i in 0..nsec {
        let o = st + i * 40;
        let name = bytes(d, o, 8, "section name")?;
        let end = name.iter().position(|b| *b == 0).unwrap_or(8);
        let raw_size = u32at(d, o + 16, "raw size")?;
        let raw_offset = u32at(d, o + 20, "raw offset")?;
        if raw_size > 0 {
            let raw_end = raw_offset
                .checked_add(raw_size)
                .ok_or(PeError::OutOfBounds("section raw range"))?;
            if raw_end as usize > d.len() {
                return Err(PeError::OutOfBounds("section raw range"));
            }
        }
        sections.push(Section {
            name: String::from_utf8_lossy(&name[..end]).into_owned(),
            virtual_size: u32at(d, o + 8, "virtual size")?,
            virtual_address: u32at(d, o + 12, "virtual address")?,
            raw_size,
            raw_offset,
            characteristics: u32at(d, o + 36, "section flags")?,
        });
    }
    let mut directories = Vec::new();
    for i in 0..ndirs {
        let o = dirs_off + i * 8;
        directories.push(DataDirectory {
            index: i,
            name: dirname(i),
            address: u32at(d, o, "directory address")?,
            address_kind: if i == 4 { "file_offset" } else { "rva" }.into(),
            size: u32at(d, o + 4, "directory size")?,
        });
    }
    let size_of_headers = u32at(d, opt + 60, "size of headers")?;
    let section_table_size = nsec
        .checked_mul(40)
        .ok_or(PeError::OutOfBounds("section table size"))?;
    let section_table_end = st
        .checked_add(section_table_size)
        .ok_or(PeError::OutOfBounds("section table end"))?;
    if size_of_headers as usize > d.len()
        || (nsec > 0 && (size_of_headers as usize) < section_table_end)
    {
        return Err(PeError::Invalid("invalid size of headers"));
    }
    for (i, section) in sections.iter().enumerate() {
        if section.raw_size > 0 && section.raw_offset < size_of_headers {
            return Err(PeError::Invalid("section raw data overlaps headers"));
        }
        let virtual_len = section.raw_size.max(section.virtual_size) as u64;
        if virtual_len > 0 {
            let start = section.virtual_address as u64;
            let end = start + virtual_len;
            for other in sections.iter().skip(i + 1) {
                let other_len = other.raw_size.max(other.virtual_size) as u64;
                if other_len == 0 {
                    continue;
                }
                let other_start = other.virtual_address as u64;
                let other_end = other_start + other_len;
                if ranges_overlap(start, end, other_start, other_end) {
                    return Err(PeError::Invalid("overlapping section RVA ranges"));
                }
            }
        }

        if section.raw_size > 0 {
            let start = section.raw_offset as u64;
            let end = start + section.raw_size as u64;
            for other in sections.iter().skip(i + 1) {
                if other.raw_size == 0 {
                    continue;
                }
                let other_start = other.raw_offset as u64;
                let other_end = other_start + other.raw_size as u64;
                if ranges_overlap(start, end, other_start, other_end) {
                    return Err(PeError::Invalid("overlapping section raw ranges"));
                }
            }
        }
    }

    let map_rva_range = |rva: u32, len: usize| -> Result<(usize, usize)> {
        let len_u32 = u32::try_from(len).map_err(|_| PeError::OutOfBounds("RVA range length"))?;
        let requested_end = (rva as usize)
            .checked_add(len)
            .ok_or(PeError::OutOfBounds("RVA range"))?;
        if rva < size_of_headers {
            if requested_end > size_of_headers as usize || requested_end > d.len() {
                return Err(PeError::OutOfBounds("header RVA range"));
            }
            return Ok((rva as usize, size_of_headers as usize - rva as usize));
        }
        for s in &sections {
            let span = s.raw_size.max(s.virtual_size);
            let virtual_end = s
                .virtual_address
                .checked_add(span)
                .ok_or(PeError::OutOfBounds("section RVA range"))?;
            if rva >= s.virtual_address && rva < virtual_end {
                let delta = rva - s.virtual_address;
                if delta >= s.raw_size {
                    return Err(PeError::OutOfBounds(
                        "RVA points to uninitialized section data",
                    ));
                }
                let available = s.raw_size - delta;
                if len > available as usize {
                    return Err(PeError::OutOfBounds("RVA range exceeds section raw data"));
                }
                let file_offset = s
                    .raw_offset
                    .checked_add(delta)
                    .ok_or(PeError::OutOfBounds("RVA file offset"))?;
                let file_end = file_offset
                    .checked_add(len_u32)
                    .ok_or(PeError::OutOfBounds("RVA file range"))?;
                if file_end as usize > d.len() {
                    return Err(PeError::OutOfBounds("RVA file range"));
                }
                return Ok((file_offset as usize, available as usize));
            }
        }
        Err(PeError::OutOfBounds("RVA does not map to file data"))
    };

    // The security directory is the PE-format exception: its address is a
    // file offset, while every other standard data-directory address is an RVA.
    for directory in &directories {
        if directory.address == 0 || directory.size == 0 {
            continue;
        }
        if directory.index == 4 {
            let end = directory
                .address
                .checked_add(directory.size)
                .ok_or(PeError::OutOfBounds("security directory file range"))?;
            if end as usize > d.len() {
                return Err(PeError::OutOfBounds("security directory file range"));
            }
        } else {
            map_rva_range(directory.address, directory.size as usize)?;
        }
    }

    let getdir = |i: usize| directories.iter().find(|x| x.index == i).cloned();
    let mut imports = Vec::new();
    if let Some(x) = getdir(1) {
        if x.address != 0 && x.size > 0 {
            if x.size < 20 || x.size % 20 != 0 {
                return Err(PeError::Invalid("invalid import directory size"));
            }
            let (start, _) = map_rva_range(x.address, x.size as usize)?;
            let count = (x.size as usize / 20).min(4096);
            let mut terminated = false;
            for i in 0..count {
                let o = start
                    .checked_add(i * 20)
                    .ok_or(PeError::OutOfBounds("import descriptor"))?;
                let lookup = u32at(d, o, "import lookup")?;
                let timestamp = u32at(d, o + 4, "import timestamp")?;
                let forwarder_chain = u32at(d, o + 8, "import forwarder chain")?;
                let name_rva = u32at(d, o + 12, "DLL name RVA")?;
                let iat = u32at(d, o + 16, "IAT")?;
                if lookup == 0
                    && timestamp == 0
                    && forwarder_chain == 0
                    && name_rva == 0
                    && iat == 0
                {
                    terminated = true;
                    break;
                }
                if name_rva == 0 {
                    return Err(PeError::Invalid("import descriptor has no DLL name"));
                }
                let (name_offset, name_available) = map_rva_range(name_rva, 1)?;
                let name = cstr(d, name_offset, name_available, 512, "import DLL name")?;
                let thunk = if lookup != 0 { lookup } else { iat };
                if thunk == 0 {
                    return Err(PeError::Invalid("import descriptor has no thunk table"));
                }
                let mut symbols = Vec::new();
                let width = if magic == 0x20b { 8usize } else { 4usize };
                    let (mut to, thunk_available) = map_rva_range(thunk, width)?;
                    let thunk_count = (thunk_available / width).min(4096);
                    let mut thunk_terminated = false;
                    for _ in 0..thunk_count {
                        let value = if width == 8 {
                            u64at(d, to, "import thunk")?
                        } else {
                            u32at(d, to, "import thunk")? as u64
                        };
                        if value == 0 {
                            thunk_terminated = true;
                            break;
                        }
                        let flag = if width == 8 { 1u64 << 63 } else { 1u64 << 31 };
                        if value & flag != 0 {
                            symbols.push(format!("ordinal:{}", value & 0xffff));
                        } else {
                            let name_rva = u32::try_from(value)
                                .map_err(|_| PeError::Invalid("import name RVA exceeds 32-bit range"))?;
                            let (no, name_len) = map_rva_range(name_rva, 3)?;
                            let hn = no
                                .checked_add(2)
                                .ok_or(PeError::OutOfBounds("hint/name offset"))?;
                            symbols.push(cstr(d, hn, name_len - 2, 512, "import symbol")?);
                        }
                        to = to
                            .checked_add(width)
                            .ok_or(PeError::OutOfBounds("thunk offset"))?;
                    }
                if !thunk_terminated {
                    return Err(PeError::Invalid("unterminated import thunk table"));
                }
                imports.push(ImportLibrary { name, symbols });
            }
            if !terminated {
                return Err(PeError::Invalid("unterminated import descriptor table"));
            }
        }
    }
    let mut exports = Vec::new();
    if let Some(x) = getdir(0) {
        if x.address != 0 && x.size > 0 {
            if x.size < 40 {
                return Err(PeError::Invalid("truncated export directory"));
            }
            let (o, _) = map_rva_range(x.address, x.size as usize)?;
            let count = u32at(d, o + 24, "export name count")? as usize;
            if count > 8192 {
                return Err(PeError::Invalid("export name count exceeds safety limit"));
            }
            let names = u32at(d, o + 32, "export name table")?;
            if names != 0 && count > 0 {
                let array_size = count
                    .checked_mul(4)
                    .ok_or(PeError::OutOfBounds("export name array"))?;
                let (no, _) = map_rva_range(names, array_size)?;
                for i in 0..count {
                    let ro = u32at(d, no + i * 4, "export name RVA")?;
                    let (name_offset, name_available) = map_rva_range(ro, 1)?;
                    exports.push(cstr(d, name_offset, name_available, 512, "export name")?);
                }
            }
        }
    }
    let (mut relocation_blocks, mut relocation_entries) = (0usize, 0usize);
    if let Some(x) = getdir(5) {
        if x.address != 0 && x.size > 0 {
            let (mut o, _) = map_rva_range(x.address, x.size as usize)?;
            let end = o
                .checked_add(x.size as usize)
                .ok_or(PeError::OutOfBounds("relocation directory"))?;
            while o < end {
                if end - o < 8 {
                    return Err(PeError::Invalid("incomplete relocation block"));
                }
                let size = u32at(d, o + 4, "relocation block size")? as usize;
                if size < 8 || size > end - o || !size.is_multiple_of(2) {
                    return Err(PeError::Invalid("invalid relocation block size"));
                }
                relocation_blocks += 1;
                relocation_entries += (size - 8) / 2;
                o += size;
            }
        }
    }
    Ok(PeReport {
        format: fmt.into(),
        execution: "NOT_ATTEMPTED",
        machine: machine(mach),
        machine_raw: mach,
        sections,
        image_base,
        entry_point_rva: entry,
        subsystem: sub,
        directories,
        imports,
        exports,
        relocation_blocks,
        relocation_entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pe64() -> Vec<u8> {
        let mut b = vec![0u8; 0x300];
        b[0..2].copy_from_slice(b"MZ");
        b[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        b[0x80..0x84].copy_from_slice(b"PE\0\0");
        let c = 0x84;
        b[c..c + 2].copy_from_slice(&0x8664u16.to_le_bytes());
        b[c + 2..c + 4].copy_from_slice(&1u16.to_le_bytes());
        b[c + 16..c + 18].copy_from_slice(&0xf0u16.to_le_bytes());
        let o = c + 20;
        b[o..o + 2].copy_from_slice(&0x20bu16.to_le_bytes());
        b[o + 16..o + 20].copy_from_slice(&0x1000u32.to_le_bytes());
        b[o + 24..o + 32].copy_from_slice(&0x140000000u64.to_le_bytes());
        b[o + 60..o + 64].copy_from_slice(&0x200u32.to_le_bytes());
        b[o + 68..o + 70].copy_from_slice(&3u16.to_le_bytes());
        b[o + 108..o + 112].copy_from_slice(&16u32.to_le_bytes());
        let s = o + 0xf0;
        b[s..s + 5].copy_from_slice(b".text");
        b[s + 8..s + 12].copy_from_slice(&0x100u32.to_le_bytes());
        b[s + 12..s + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        b[s + 16..s + 20].copy_from_slice(&0x100u32.to_le_bytes());
        b[s + 20..s + 24].copy_from_slice(&0x200u32.to_le_bytes());
        b
    }
    fn pe32() -> Vec<u8> {
        let mut b = pe64();
        // Move the section header 16 bytes earlier for the smaller PE32
        // optional header while preserving its raw file offset.
        b.copy_within(0x188..0x1b0, 0x178);
        b[0x94..0x96].copy_from_slice(&0xe0u16.to_le_bytes());
        b[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
        b[0xb4..0xb8].copy_from_slice(&0x400000u32.to_le_bytes());
        b[0xd4..0xd8].copy_from_slice(&0x200u32.to_le_bytes());
        b[0xf4..0xf8].copy_from_slice(&16u32.to_le_bytes());
        b[0x84..0x86].copy_from_slice(&0x014cu16.to_le_bytes());
        b
    }

    fn pe64_with_one_import() -> Vec<u8> {
        let mut b = pe64();
        let import_dir = 0x98 + 112 + 8;
        b[import_dir..import_dir + 4].copy_from_slice(&0x1000u32.to_le_bytes());
        b[import_dir + 4..import_dir + 8].copy_from_slice(&40u32.to_le_bytes());

        // IMAGE_IMPORT_DESCRIPTOR
        b[0x200..0x204].copy_from_slice(&0x1040u32.to_le_bytes());
        b[0x20c..0x210].copy_from_slice(&0x1030u32.to_le_bytes());
        b[0x210..0x214].copy_from_slice(&0x1050u32.to_le_bytes());
        b[0x230..0x23d].copy_from_slice(b"KERNEL32.dll\0");
        // IMAGE_THUNK_DATA64 followed by its null terminator.
        b[0x240..0x248].copy_from_slice(&0x1060u64.to_le_bytes());
        b[0x248..0x250].copy_from_slice(&0u64.to_le_bytes());
        // IMAGE_IMPORT_BY_NAME: two-byte hint then a symbol name.
        b[0x260..0x26d].copy_from_slice(&[0, 0, b'E', b'x', b'i', b't', b'P', b'r', b'o', b'c', b'e', b's', b's']);
        b[0x26d] = 0;
        b
    }

    #[test]
    fn extracts_valid_import_library_and_symbol() {
        let report = inspect(&pe64_with_one_import()).unwrap();
        assert_eq!(report.imports.len(), 1);
        assert_eq!(report.imports[0].name, "KERNEL32.dll");
        assert_eq!(report.imports[0].symbols, vec!["ExitProcess"]);
    }

    #[test]
    fn does_not_treat_nonzero_import_timestamp_as_null_descriptor() {
        let mut b = pe64_with_one_import();
        let descriptor = 0x200;
        b[descriptor + 4..descriptor + 8].copy_from_slice(&1u32.to_le_bytes());
        b[descriptor + 12..descriptor + 16].copy_from_slice(&0u32.to_le_bytes());
        b[descriptor + 16..descriptor + 20].copy_from_slice(&0u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid("import descriptor has no DLL name"))
        ));
    }

    #[test]
    fn rejects_import_descriptor_with_zero_dll_name_rva() {
        let mut b = pe64_with_one_import();
        let name_rva = 0x200 + 12;
        b[name_rva..name_rva + 4].copy_from_slice(&0u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid("import descriptor has no DLL name"))
        ));
    }

    #[test]
    fn security_directory_address_is_reported_as_file_offset() {
        let mut b = pe64();
        let security_dir = 0x98 + 112 + 4 * 8;
        b[security_dir..security_dir + 4].copy_from_slice(&0x250u32.to_le_bytes());
        b[security_dir + 4..security_dir + 8].copy_from_slice(&0x10u32.to_le_bytes());

        let report = inspect(&b).unwrap();
        assert_eq!(report.directories[4].address, 0x250);
        assert_eq!(report.directories[4].address_kind, "file_offset");
    }

    #[test]
    fn rejects_security_directory_file_range_outside_input() {
        let mut b = pe64();
        let security_dir = 0x98 + 112 + 4 * 8;
        b[security_dir..security_dir + 4].copy_from_slice(&0x2f8u32.to_le_bytes());
        b[security_dir + 4..security_dir + 8].copy_from_slice(&0x20u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::OutOfBounds("security directory file range"))
        ));
    }

    #[test]
    fn validates_unparsed_rva_directory_ranges() {
        let mut b = pe64();
        let resource_dir = 0x98 + 112 + 2 * 8;
        b[resource_dir..resource_dir + 4].copy_from_slice(&0x10f0u32.to_le_bytes());
        b[resource_dir + 4..resource_dir + 8].copy_from_slice(&0x20u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::OutOfBounds("RVA range exceeds section raw data"))
        ));
    }

    #[test]
    fn rejects_directory_count_exceeding_optional_header_storage() {
        let mut b = pe64();
        // Declare one data directory while the optional header contains
        // space for none.
        b[0x94..0x96].copy_from_slice(&112u16.to_le_bytes());
        b[0x98 + 108..0x98 + 112].copy_from_slice(&1u32.to_le_bytes());
        let error = inspect(&b).unwrap_err();
        assert!(matches!(
            error,
            PeError::Truncated("data directory entries")
        ));
    }

    #[test]
    fn parses_pe32_plus() {
        let r = inspect(&pe64()).unwrap();
        assert_eq!(r.format, "PE32+");
        assert_eq!(r.machine, "AMD64");
        assert_eq!(r.sections[0].name, ".text");
        assert_eq!(r.subsystem, "Windows Console");
    }

    #[test]
    fn parses_pe32_x86() {
        let r = inspect(&pe32()).unwrap();
        assert_eq!(r.format, "PE32");
        assert_eq!(r.machine, "x86");
        assert_eq!(r.image_base, 0x400000);
        assert_eq!(r.sections[0].name, ".text");
    }
    #[test]
    fn json_report_is_valid_and_marks_execution_unattempted() {
        let report = inspect(&pe64()).unwrap();
        let encoded = serde_json::to_string(&report).unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded["execution"], "NOT_ATTEMPTED");
    }
    #[test]
    fn rejects_bad_dos() {
        assert!(matches!(inspect(b"no"), Err(PeError::Invalid(_))));
    }
    #[test]
    fn rejects_truncated() {
        assert!(inspect(b"MZ").is_err());
    }
    #[test]
    fn rejects_bad_offset() {
        let mut b = pe64();
        b[0x3c..0x40].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(inspect(&b).is_err());
    }
    #[test]
    fn rejects_too_many_sections() {
        let mut b = pe64();
        b[0x86..0x88].copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(inspect(&b).is_err());
    }
    #[test]
    fn rejects_section_raw_data_outside_file() {
        let mut b = pe64();
        b[0x198..0x19c].copy_from_slice(&0x2000u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::OutOfBounds("section raw range"))
        ));
    }
    #[test]
    fn rejects_section_raw_data_overlapping_headers() {
        let mut b = pe64();
        b[0x188 + 20..0x188 + 24].copy_from_slice(&0x180u32.to_le_bytes());
        b[0x188 + 16..0x188 + 20].copy_from_slice(&0x80u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid("section raw data overlaps headers"))
        ));
    }

    #[test]
    fn rejects_overlapping_section_virtual_ranges() {
        let mut b = pe64();
        b.resize(0x380, 0);
        b[0x86..0x88].copy_from_slice(&2u16.to_le_bytes());
        let second = 0x188 + 40;
        b[second..second + 5].copy_from_slice(b".data");
        b[second + 8..second + 12].copy_from_slice(&0x80u32.to_le_bytes());
        b[second + 12..second + 16].copy_from_slice(&0x1080u32.to_le_bytes());
        b[second + 16..second + 20].copy_from_slice(&0x80u32.to_le_bytes());
        b[second + 20..second + 24].copy_from_slice(&0x300u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid("overlapping section RVA ranges"))
        ));
    }

    #[test]
    fn rejects_overlapping_section_raw_ranges() {
        let mut b = pe64();
        b[0x86..0x88].copy_from_slice(&2u16.to_le_bytes());
        let second = 0x188 + 40;
        b[second..second + 5].copy_from_slice(b".data");
        b[second + 8..second + 12].copy_from_slice(&0x80u32.to_le_bytes());
        b[second + 12..second + 16].copy_from_slice(&0x1200u32.to_le_bytes());
        b[second + 16..second + 20].copy_from_slice(&0x80u32.to_le_bytes());
        b[second + 20..second + 24].copy_from_slice(&0x280u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid("overlapping section raw ranges"))
        ));
    }

    #[test]
    fn rejects_directory_range_past_section_raw_data() {
        let mut b = pe64();
        let import_dir = 0x98 + 112 + 8;
        b[import_dir..import_dir + 4].copy_from_slice(&0x10f0u32.to_le_bytes());
        b[import_dir + 4..import_dir + 8].copy_from_slice(&0x28u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::OutOfBounds("RVA range exceeds section raw data"))
        ));
    }
    #[test]
    fn rejects_directory_in_virtual_only_section_data() {
        let mut b = pe64();
        let section = 0x98 + 0xf0;
        b[section + 8..section + 12].copy_from_slice(&0x200u32.to_le_bytes());
        let import_dir = 0x98 + 112 + 8;
        b[import_dir..import_dir + 4].copy_from_slice(&0x1100u32.to_le_bytes());
        b[import_dir + 4..import_dir + 8].copy_from_slice(&0x14u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::OutOfBounds(
                "RVA points to uninitialized section data"
            ))
        ));
    }
    #[test]
    fn rejects_export_directory_range_past_section_raw_data() {
        let mut b = pe64();
        let export_dir = 0x98 + 112;
        b[export_dir..export_dir + 4].copy_from_slice(&0x10f0u32.to_le_bytes());
        b[export_dir + 4..export_dir + 8].copy_from_slice(&0x28u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::OutOfBounds("RVA range exceeds section raw data"))
        ));
    }
    #[test]
    fn rejects_relocation_directory_range_past_section_raw_data() {
        let mut b = pe64();
        let reloc_dir = 0x98 + 112 + 5 * 8;
        b[reloc_dir..reloc_dir + 4].copy_from_slice(&0x10f0u32.to_le_bytes());
        b[reloc_dir + 4..reloc_dir + 8].copy_from_slice(&0x20u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::OutOfBounds("RVA range exceeds section raw data"))
        ));
    }
}
