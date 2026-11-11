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
    pub rva: u32,
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
fn cstr(d: &[u8], o: usize, max: usize, w: &'static str) -> Result<String> {
    let available = d.len().checked_sub(o).ok_or(PeError::OutOfBounds(w))?;
    let b = bytes(d, o, available.min(max), w)?;
    let end = b.iter().position(|x| *x == 0).ok_or(PeError::Invalid(w))?;
    Ok(String::from_utf8_lossy(&b[..end]).into_owned())
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
    let ndirs = (u32at(d, count_off, "directory count")? as usize)
        .min(MAX_DIRECTORIES)
        .min(opt_size.saturating_sub(dirs_off - opt) / 8);
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
        sections.push(Section {
            name: String::from_utf8_lossy(&name[..end]).into_owned(),
            virtual_size: u32at(d, o + 8, "virtual size")?,
            virtual_address: u32at(d, o + 12, "virtual address")?,
            raw_size: u32at(d, o + 16, "raw size")?,
            raw_offset: u32at(d, o + 20, "raw offset")?,
            characteristics: u32at(d, o + 36, "section flags")?,
        });
    }
    let mut directories = Vec::new();
    for i in 0..ndirs {
        let o = dirs_off + i * 8;
        directories.push(DataDirectory {
            index: i,
            name: dirname(i),
            rva: u32at(d, o, "directory RVA")?,
            size: u32at(d, o + 4, "directory size")?,
        });
    }
    let map_rva = |rva: u32| -> Result<usize> {
        for s in &sections {
            let span = s.raw_size.max(s.virtual_size);
            let end = s
                .virtual_address
                .checked_add(span)
                .ok_or(PeError::OutOfBounds("section RVA range"))?;
            if rva >= s.virtual_address && rva < end {
                let delta = rva - s.virtual_address;
                if delta >= s.raw_size {
                    return Err(PeError::OutOfBounds(
                        "RVA points to uninitialized section data",
                    ));
                }
                let off = s
                    .raw_offset
                    .checked_add(delta)
                    .ok_or(PeError::OutOfBounds("RVA offset"))? as usize;
                if off >= d.len() {
                    return Err(PeError::OutOfBounds("RVA file offset"));
                }
                return Ok(off);
            }
        }
        if rva < st as u32 {
            Ok(rva as usize)
        } else {
            Err(PeError::OutOfBounds("RVA does not map to file data"))
        }
    };
    let getdir = |i: usize| directories.iter().find(|x| x.index == i).cloned();
    let mut imports = Vec::new();
    if let Some(x) = getdir(1) {
        if x.rva != 0 && x.size >= 20 {
            let start = map_rva(x.rva)?;
            let count = (x.size as usize / 20).min(4096);
            for i in 0..count {
                let o = start
                    .checked_add(i * 20)
                    .ok_or(PeError::OutOfBounds("import descriptor"))?;
                let lookup = u32at(d, o, "import lookup")?;
                let name_rva = u32at(d, o + 12, "DLL name RVA")?;
                let iat = u32at(d, o + 16, "IAT")?;
                if lookup == 0 && name_rva == 0 && iat == 0 {
                    break;
                }
                let name = cstr(d, map_rva(name_rva)?, 512, "import DLL name")?;
                let thunk = if lookup != 0 { lookup } else { iat };
                let mut symbols = Vec::new();
                if thunk != 0 {
                    let mut to = map_rva(thunk)?;
                    let width = if magic == 0x20b { 8usize } else { 4usize };
                    for _ in 0..4096 {
                        let value = if width == 8 {
                            u64at(d, to, "import thunk")?
                        } else {
                            u32at(d, to, "import thunk")? as u64
                        };
                        if value == 0 {
                            break;
                        }
                        let flag = if width == 8 { 1u64 << 63 } else { 1u64 << 31 };
                        if value & flag != 0 {
                            symbols.push(format!("ordinal:{}", value & 0xffff));
                        } else {
                            let no = map_rva(value as u32)?;
                            let hn = no
                                .checked_add(2)
                                .ok_or(PeError::OutOfBounds("hint/name offset"))?;
                            symbols.push(cstr(d, hn, 512, "import symbol")?);
                        }
                        to = to
                            .checked_add(width)
                            .ok_or(PeError::OutOfBounds("thunk offset"))?;
                    }
                }
                imports.push(ImportLibrary { name, symbols });
            }
        }
    }
    let mut exports = Vec::new();
    if let Some(x) = getdir(0) {
        if x.rva != 0 && x.size >= 40 {
            let o = map_rva(x.rva)?;
            let count = (u32at(d, o + 24, "export name count")? as usize).min(8192);
            let names = u32at(d, o + 32, "export name table")?;
            if names != 0 {
                let no = map_rva(names)?;
                for i in 0..count {
                    let ro = u32at(
                        d,
                        no.checked_add(i * 4)
                            .ok_or(PeError::OutOfBounds("export names"))?,
                        "export name RVA",
                    )?;
                    exports.push(cstr(d, map_rva(ro)?, 512, "export name")?);
                }
            }
        }
    }
    let (mut relocation_blocks, mut relocation_entries) = (0usize, 0usize);
    if let Some(x) = getdir(5) {
        if x.rva != 0 && x.size > 0 {
            let mut o = map_rva(x.rva)?;
            let end = o
                .checked_add(x.size as usize)
                .ok_or(PeError::OutOfBounds("relocation directory"))?;
            if end > d.len() {
                return Err(PeError::OutOfBounds("relocation directory"));
            }
            while o < end {
                if end - o < 8 {
                    return Err(PeError::Invalid("incomplete relocation block"));
                }
                let size = u32at(d, o + 4, "relocation block size")? as usize;
                if size < 8 || size > end - o || size % 2 != 0 {
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
        let mut b = vec![0u8; 0x200 + 40];
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
        b[0xf4..0xf8].copy_from_slice(&16u32.to_le_bytes());
        b[0x86..0x88].copy_from_slice(&0x014cu16.to_le_bytes());
        b
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
}
