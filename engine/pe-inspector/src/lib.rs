use serde::Serialize;
use std::fmt;

mod image_buffer;
mod image_layout;
mod resolver;
mod typed_metadata;
pub use image_buffer::{
    allocate_image_buffer, populate_image_buffer, ImageAllocationError, ImageAllocator,
    ImageBuffer, SystemImageAllocator,
};
pub use image_layout::{
    plan_image_layout, ImageCopyRange, ImageLayoutError, ImageLayoutLimits, ImageLayoutPlan,
    ImageZeroFillRange, SectionImageLayout,
};
pub use resolver::{
    ExportCatalogue, ExportCatalogueError, ExportCatalogueLimits, ExportLookup, ExportLookupResult,
    ExportModule, ImportResolutionError, ImportResolutionFailure, ImportResolutionLimits,
    ImportResolutionPlan, ResolvedImport, resolve_imports,
};
pub use typed_metadata::{
    typed_exports, typed_imports, ExportSymbol, ImportLibraryMetadata, ImportSymbol, MetadataError,
};

const MAX_SECTIONS: usize = 96;
const MAX_DIRECTORIES: usize = 16;
const MAX_DEBUG_DIRECTORY_ENTRIES: usize = 4096;

#[derive(Debug, Clone, Serialize)]
pub struct PeReport {
    pub format: String,
    pub execution: &'static str,
    pub machine: String,
    pub machine_raw: u16,
    pub sections: Vec<Section>,
    pub image_base: u64,
    pub size_of_image: u32,
    pub size_of_headers: u32,
    pub section_alignment: u32,
    pub file_alignment: u32,
    pub entry_point_rva: u32,
    pub subsystem: String,
    pub directories: Vec<DataDirectory>,
    pub imports: Vec<ImportLibrary>,
    /// Lossless import metadata used by NCE-003. Legacy `imports` remains stable.
    pub import_metadata: Vec<ParsedImportLibrary>,
    pub exports: Vec<String>,
    /// Complete export address/name tables when an export directory is present.
    pub export_metadata: Option<ParsedExportDirectory>,
    pub relocation_blocks: usize,
    pub relocation_entries: usize,
    /// Total input file length in bytes.
    pub file_size: u64,
    /// Trailing bytes after the headers and all section raw-data ranges.
    /// This may include certificate and unmapped debug data.
    pub overlay: Option<FileRange>,
    /// Metadata for IMAGE_DEBUG_DIRECTORY entries. Raw file pointers and
    /// mapped RVAs are retained separately because they describe different
    /// address spaces.
    pub debug_entries: Vec<DebugDirectoryEntry>,
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
pub struct FileRange {
    pub offset: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugDirectoryEntry {
    pub characteristics: u32,
    pub timestamp: u32,
    pub major_version: u16,
    pub minor_version: u16,
    pub type_code: u32,
    pub size_of_data: u32,
    pub address_of_raw_data: u32,
    pub pointer_to_raw_data: u32,
    /// File offset obtained by mapping AddressOfRawData as an RVA.
    /// None means the debug data is not mapped (AddressOfRawData is zero)
    /// or its declared data size is zero.
    pub mapped_file_offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportLibrary {
    pub name: String,
    pub symbols: Vec<String>,
}

/// A parsed import symbol with the original lookup and IAT slot locations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParsedImportSymbol {
    Name { name: String, hint: u16 },
    Ordinal { ordinal: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParsedImportEntry {
    pub symbol: ParsedImportSymbol,
    pub lookup_rva: u32,
    pub iat_rva: u32,
    pub pointer_width: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParsedImportLibrary {
    pub name: String,
    pub original_first_thunk: u32,
    pub first_thunk: u32,
    pub symbols: Vec<ParsedImportEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportFunctionSlot {
    /// Public PE ordinal: ordinal_base + function-table index.
    pub ordinal: u32,
    /// None represents an unused (zero) function-table slot.
    pub function_rva: Option<u32>,
    /// Present when the function RVA points into the export directory.
    pub forwarder: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportNameAssociation {
    pub name: String,
    pub function_index: u32,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParsedExportDirectory {
    /// Optional module name stored in IMAGE_EXPORT_DIRECTORY.Name.
    pub module_name: Option<String>,
    pub ordinal_base: u32,
    pub functions: Vec<ExportFunctionSlot>,
    pub names: Vec<ExportNameAssociation>,
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

fn validate_certificate_table(d: &[u8], offset: u32, size: u32) -> Result<()> {
    if !offset.is_multiple_of(8) {
        return Err(PeError::Invalid(
            "security directory file offset is not 8-byte aligned",
        ));
    }
    if !size.is_multiple_of(8) {
        return Err(PeError::Invalid(
            "security directory size is not 8-byte aligned",
        ));
    }

    let start = offset as usize;
    let table_size = size as usize;
    let end = start
        .checked_add(table_size)
        .ok_or(PeError::OutOfBounds("security directory file range"))?;
    if end > d.len() {
        return Err(PeError::OutOfBounds("security directory file range"));
    }

    let mut cursor = start;
    while cursor < end {
        let remaining = end - cursor;
        if remaining < 8 {
            return Err(PeError::Invalid("truncated WIN_CERTIFICATE header"));
        }
        let record_length = u32at(d, cursor, "WIN_CERTIFICATE length")? as usize;
        if record_length < 8 {
            return Err(PeError::Invalid("invalid WIN_CERTIFICATE length"));
        }
        if !record_length.is_multiple_of(8) {
            return Err(PeError::Invalid(
                "WIN_CERTIFICATE length does not include 8-byte alignment padding",
            ));
        }
        if record_length > remaining {
            return Err(PeError::OutOfBounds("WIN_CERTIFICATE record"));
        }
        // The PE format defines dwLength as the finalized record length,
        // including any padding required to reach the next 8-byte boundary.
        cursor = cursor
            .checked_add(record_length)
            .ok_or(PeError::OutOfBounds("WIN_CERTIFICATE record"))?;
    }

    Ok(())
}

pub fn inspect(d: &[u8]) -> Result<PeReport> {
    if bytes(d, 0, 2, "DOS signature")? != b"MZ" {
        return Err(PeError::Invalid("missing MZ signature"));
    }
    let pe = u32at(d, 0x3c, "e_lfanew")? as usize;
    if pe < 0x40 {
        return Err(PeError::Invalid("PE header overlaps the DOS header"));
    }
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
    let machine_conflicts_with_format = match magic {
        0x10b => matches!(mach, 0x8664 | 0xaa64 | 0x0200 | 0xa641 | 0xa64e),
        0x20b => matches!(mach, 0x014c | 0x01c0 | 0x01c2 | 0x01c4),
        _ => false,
    };
    if machine_conflicts_with_format {
        return Err(PeError::Invalid(
            "machine type conflicts with optional header format",
        ));
    }
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
    if declared_directories > available_directories {
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
    if size_of_headers as usize > d.len() || (size_of_headers as usize) < section_table_end {
        return Err(PeError::Invalid("invalid size of headers"));
    }
    for (i, section) in sections.iter().enumerate() {
        if section.raw_size > 0 && section.raw_offset < size_of_headers {
            return Err(PeError::Invalid("section raw data overlaps headers"));
        }
        let virtual_len = section.raw_size.max(section.virtual_size) as u64;
        if virtual_len > 0 {
            let start = section.virtual_address as u64;
            if start < size_of_headers as u64 {
                return Err(PeError::Invalid("section virtual range overlaps headers"));
            }
            let end = start + virtual_len;
            if end > u32::MAX as u64 + 1 {
                return Err(PeError::OutOfBounds(
                    "section RVA range exceeds 32-bit address space",
                ));
            }
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
            // Use a wider type for the exclusive end: a valid section may
            // end exactly at 2^32, which cannot be represented by a u32 RVA.
            let virtual_end = s.virtual_address as u64 + span as u64;
            if rva >= s.virtual_address && (rva as u64) < virtual_end {
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
        if (directory.address == 0) != (directory.size == 0) {
            return Err(PeError::Invalid(
                "data directory has an address/size mismatch",
            ));
        }
        if directory.address == 0 {
            continue;
        }
        if directory.index == 4 {
            // The security directory is a file-offset table of 8-byte-aligned
            // WIN_CERTIFICATE records, not an RVA-mapped data directory.
            validate_certificate_table(d, directory.address, directory.size)?;
        } else {
            map_rva_range(directory.address, directory.size as usize)?;
        }
    }

    let getdir = |i: usize| directories.iter().find(|x| x.index == i).cloned();
    let mut imports = Vec::new();
    let mut import_metadata = Vec::new();
    if let Some(x) = getdir(1) {
        if x.address != 0 && x.size > 0 {
            if x.size < 20 {
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
                if iat == 0 {
                    return Err(PeError::Invalid("import descriptor has no IAT"));
                }
                let thunk = if lookup != 0 { lookup } else { iat };
                if thunk == 0 {
                    return Err(PeError::Invalid("import descriptor has no thunk table"));
                }
                let mut symbols = Vec::new();
                let mut parsed_symbols = Vec::new();
                let width = if magic == 0x20b { 8usize } else { 4usize };
                let pointer_width = width as u8;
                let (mut to, thunk_available) = map_rva_range(thunk, width)?;
                let thunk_count = (thunk_available / width).min(4096);
                let mut thunk_terminated = false;
                for thunk_index in 0..thunk_count {
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
                        // IMAGE_THUNK_DATA reserves every ordinal-form bit
                        // except the high ordinal flag and low 16-bit ordinal.
                        if value & (flag - 1) & !0xffff != 0 {
                            return Err(PeError::Invalid(
                                "ordinal import thunk has nonzero reserved bits",
                            ));
                        }
                        let ordinal = (value & 0xffff) as u16;
                        symbols.push(format!("ordinal:{}", ordinal));
                        parsed_symbols.push(ParsedImportEntry {
                            symbol: ParsedImportSymbol::Ordinal { ordinal },
                            lookup_rva: thunk
                                .checked_add(
                                    u32::try_from(thunk_index * width)
                                        .map_err(|_| PeError::OutOfBounds("import lookup RVA"))?,
                                )
                                .ok_or(PeError::OutOfBounds("import lookup RVA"))?,
                            iat_rva: iat
                                .checked_add(
                                    u32::try_from(thunk_index * width)
                                        .map_err(|_| PeError::OutOfBounds("IAT RVA"))?,
                                )
                                .ok_or(PeError::OutOfBounds("IAT RVA"))?,
                            pointer_width,
                        });
                    } else {
                        let name_rva = u32::try_from(value).map_err(|_| {
                            PeError::Invalid("import name RVA exceeds 32-bit range")
                        })?;
                        let (no, name_len) = map_rva_range(name_rva, 3)?;
                        let hint = u16at(d, no, "import hint")?;
                        let hn = no
                            .checked_add(2)
                            .ok_or(PeError::OutOfBounds("hint/name offset"))?;
                        let name = cstr(d, hn, name_len - 2, 512, "import symbol")?;
                        symbols.push(name.clone());
                        parsed_symbols.push(ParsedImportEntry {
                            symbol: ParsedImportSymbol::Name { name, hint },
                            lookup_rva: thunk
                                .checked_add(
                                    u32::try_from(thunk_index * width)
                                        .map_err(|_| PeError::OutOfBounds("import lookup RVA"))?,
                                )
                                .ok_or(PeError::OutOfBounds("import lookup RVA"))?,
                            iat_rva: iat
                                .checked_add(
                                    u32::try_from(thunk_index * width)
                                        .map_err(|_| PeError::OutOfBounds("IAT RVA"))?,
                                )
                                .ok_or(PeError::OutOfBounds("IAT RVA"))?,
                            pointer_width,
                        });
                    }
                    to = to
                        .checked_add(width)
                        .ok_or(PeError::OutOfBounds("thunk offset"))?;
                }
                if !thunk_terminated {
                    return Err(PeError::Invalid("unterminated import thunk table"));
                }
                imports.push(ImportLibrary {
                    name: name.clone(),
                    symbols,
                });
                import_metadata.push(ParsedImportLibrary {
                    name,
                    original_first_thunk: lookup,
                    first_thunk: iat,
                    symbols: parsed_symbols,
                });
            }
            if !terminated {
                return Err(PeError::Invalid("unterminated import descriptor table"));
            }
        }
    }
    let mut exports = Vec::new();
    let mut export_metadata = None;
    if let Some(x) = getdir(0) {
        if x.address != 0 && x.size > 0 {
            if x.size < 40 {
                return Err(PeError::Invalid("truncated export directory"));
            }
            let (o, _) = map_rva_range(x.address, x.size as usize)?;
            let module_name_rva = u32at(d, o + 12, "export module name RVA")?;
            let module_name = if module_name_rva == 0 {
                None
            } else {
                let (module_offset, module_available) = map_rva_range(module_name_rva, 1)?;
                Some(cstr(
                    d,
                    module_offset,
                    module_available,
                    512,
                    "export module name",
                )?)
            };
            let ordinal_base = u32at(d, o + 16, "export ordinal base")?;
            let function_count = u32at(d, o + 20, "export function count")? as usize;
            let name_count = u32at(d, o + 24, "export name count")? as usize;
            if function_count > 16384 || name_count > 8192 {
                return Err(PeError::Invalid("export table count exceeds safety limit"));
            }
            if name_count > function_count {
                return Err(PeError::Invalid("export name count exceeds function count"));
            }
            let functions_rva = u32at(d, o + 28, "export function table")?;
            let names_rva = u32at(d, o + 32, "export name table")?;
            let ordinals_rva = u32at(d, o + 36, "export ordinal table")?;
            let mut function_slots = Vec::with_capacity(function_count);
            if function_count > 0 {
                if functions_rva == 0 {
                    return Err(PeError::Invalid("missing export function table"));
                }
                let function_bytes = function_count
                    .checked_mul(4)
                    .ok_or(PeError::OutOfBounds("export function array"))?;
                let (functions_offset, _) = map_rva_range(functions_rva, function_bytes)?;
                let export_end = x.address as u64 + x.size as u64;
                for i in 0..function_count {
                    let function_rva = u32at(d, functions_offset + i * 4, "export function RVA")?;
                    let forwarder = if function_rva != 0
                        && (function_rva as u64) >= x.address as u64
                        && (function_rva as u64) < export_end
                    {
                        // An address-table entry inside the export directory is
                        // a forwarder string, not a code/data RVA.
                        let (forwarder_offset, file_available) = map_rva_range(function_rva, 1)?;
                        let directory_available = (export_end - function_rva as u64) as usize;
                        let available = file_available.min(directory_available);
                        Some(cstr(
                            d,
                            forwarder_offset,
                            available,
                            4096,
                            "export forwarder",
                        )?)
                    } else {
                        None
                    };
                    let index = u32::try_from(i)
                        .map_err(|_| PeError::OutOfBounds("export function index"))?;
                    let ordinal = ordinal_base
                        .checked_add(index)
                        .ok_or(PeError::OutOfBounds("export ordinal"))?;
                    function_slots.push(ExportFunctionSlot {
                        ordinal,
                        function_rva: (function_rva != 0).then_some(function_rva),
                        forwarder,
                    });
                }
            }
            let mut name_associations = Vec::with_capacity(name_count);
            if name_count > 0 {
                if names_rva == 0 || ordinals_rva == 0 {
                    return Err(PeError::Invalid("missing export name or ordinal table"));
                }
                let name_bytes = name_count
                    .checked_mul(4)
                    .ok_or(PeError::OutOfBounds("export name array"))?;
                let ordinal_bytes = name_count
                    .checked_mul(2)
                    .ok_or(PeError::OutOfBounds("export ordinal array"))?;
                let (names_offset, _) = map_rva_range(names_rva, name_bytes)?;
                let (ordinals_offset, _) = map_rva_range(ordinals_rva, ordinal_bytes)?;
                for i in 0..name_count {
                    let ordinal =
                        u16at(d, ordinals_offset + i * 2, "export name ordinal")? as usize;
                    if ordinal >= function_count {
                        return Err(PeError::Invalid("export name ordinal is out of range"));
                    }
                    let name_rva = u32at(d, names_offset + i * 4, "export name RVA")?;
                    if name_rva == 0 {
                        return Err(PeError::Invalid("export name has a zero RVA"));
                    }
                    let (name_offset, name_available) = map_rva_range(name_rva, 1)?;
                    let name = cstr(d, name_offset, name_available, 512, "export name")?;
                    let function_index = u32::try_from(ordinal)
                        .map_err(|_| PeError::OutOfBounds("export name function index"))?;
                    let public_ordinal = ordinal_base
                        .checked_add(function_index)
                        .ok_or(PeError::OutOfBounds("export name ordinal"))?;
                    exports.push(name.clone());
                    name_associations.push(ExportNameAssociation {
                        name,
                        function_index,
                        ordinal: public_ordinal,
                    });
                }
            }
            export_metadata = Some(ParsedExportDirectory {
                module_name,
                ordinal_base,
                functions: function_slots,
                names: name_associations,
            });
        }
    }
    let mut debug_entries = Vec::new();
    if let Some(debug_directory) = getdir(6) {
        if debug_directory.address != 0 {
            const DEBUG_DIRECTORY_ENTRY_SIZE: usize = 28;
            let directory_size = debug_directory.size as usize;
            if !directory_size.is_multiple_of(DEBUG_DIRECTORY_ENTRY_SIZE) {
                return Err(PeError::Invalid(
                    "debug directory size is not a multiple of entry size",
                ));
            }
            let entry_count = directory_size / DEBUG_DIRECTORY_ENTRY_SIZE;
            if entry_count > MAX_DEBUG_DIRECTORY_ENTRIES {
                return Err(PeError::Invalid(
                    "debug directory entry count exceeds safety limit",
                ));
            }
            let (directory_offset, _) = map_rva_range(debug_directory.address, directory_size)?;
            debug_entries.reserve(entry_count);

            for index in 0..entry_count {
                let offset = directory_offset
                    .checked_add(index * DEBUG_DIRECTORY_ENTRY_SIZE)
                    .ok_or(PeError::OutOfBounds("debug directory entry"))?;
                let size_of_data = u32at(d, offset + 16, "debug data size")?;
                let address_of_raw_data = u32at(d, offset + 20, "debug data RVA")?;
                let pointer_to_raw_data = u32at(d, offset + 24, "debug data file pointer")?;

                // PointerToRawData is a file offset. Validate it directly;
                // never feed it to the RVA mapper.
                if pointer_to_raw_data != 0 && size_of_data != 0 {
                    bytes(
                        d,
                        pointer_to_raw_data as usize,
                        size_of_data as usize,
                        "debug data file range",
                    )?;
                }

                // AddressOfRawData is an RVA when nonzero. Some debug data is
                // intentionally file-only and therefore has a zero RVA.
                let mapped_file_offset = if address_of_raw_data != 0 && size_of_data != 0 {
                    let (mapped_offset, _) =
                        map_rva_range(address_of_raw_data, size_of_data as usize)?;
                    Some(
                        u32::try_from(mapped_offset)
                            .map_err(|_| PeError::OutOfBounds("debug data mapped file offset"))?,
                    )
                } else {
                    None
                };

                debug_entries.push(DebugDirectoryEntry {
                    characteristics: u32at(d, offset, "debug characteristics")?,
                    timestamp: u32at(d, offset + 4, "debug timestamp")?,
                    major_version: u16at(d, offset + 8, "debug major version")?,
                    minor_version: u16at(d, offset + 10, "debug minor version")?,
                    type_code: u32at(d, offset + 12, "debug type")?,
                    size_of_data,
                    address_of_raw_data,
                    pointer_to_raw_data,
                    mapped_file_offset,
                });
            }
        }
    }

    let file_size = d.len() as u64;
    let last_section_raw_end = sections
        .iter()
        .filter(|section| section.raw_size != 0)
        .map(|section| u64::from(section.raw_offset) + u64::from(section.raw_size))
        .max()
        .unwrap_or(0);
    let overlay_offset = u64::from(size_of_headers).max(last_section_raw_end);
    let overlay = (overlay_offset < file_size).then_some(FileRange {
        offset: overlay_offset,
        size: file_size - overlay_offset,
    });

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
                let page_rva = u32at(d, o, "relocation page RVA")?;
                if !page_rva.is_multiple_of(0x1000) {
                    return Err(PeError::Invalid(
                        "relocation block page RVA is not 4KiB aligned",
                    ));
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
        size_of_image: u32at(d, opt + 56, "size of image")?,
        size_of_headers,
        section_alignment: u32at(d, opt + 32, "section alignment")?,
        file_alignment: u32at(d, opt + 36, "file alignment")?,
        entry_point_rva: entry,
        subsystem: sub,
        directories,
        imports,
        import_metadata,
        exports,
        export_metadata,
        relocation_blocks,
        relocation_entries,
        file_size,
        overlay,
        debug_entries,
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
        b[o + 32..o + 36].copy_from_slice(&0x1000u32.to_le_bytes());
        b[o + 36..o + 40].copy_from_slice(&0x200u32.to_le_bytes());
        b[o + 56..o + 60].copy_from_slice(&0x2000u32.to_le_bytes());
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
        // The PE32 directory table starts 16 bytes earlier than PE32+.
        // Clear the inherited PE32+ optional-header bytes so the synthetic
        // fixture does not accidentally encode malformed directory entries.
        b[0xf8..0x178].fill(0);
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
        b[0x260..0x26d].copy_from_slice(&[
            0, 0, b'E', b'x', b'i', b't', b'P', b'r', b'o', b'c', b'e', b's', b's',
        ]);
        b[0x26d] = 0;
        b
    }

    #[test]
    fn reports_image_layout_metadata_for_pe32_plus() {
        let report = inspect(&pe64()).unwrap();
        assert_eq!(report.size_of_image, 0x2000);
        assert_eq!(report.size_of_headers, 0x200);
        assert_eq!(report.section_alignment, 0x1000);
        assert_eq!(report.file_alignment, 0x200);
    }

    fn pe32_with_one_import() -> Vec<u8> {
        let mut b = pe32();
        let import_dir = 0x98 + 96 + 8;
        b[import_dir..import_dir + 4].copy_from_slice(&0x1000u32.to_le_bytes());
        b[import_dir + 4..import_dir + 8].copy_from_slice(&40u32.to_le_bytes());

        // IMAGE_IMPORT_DESCRIPTOR and 32-bit IMAGE_THUNK_DATA.
        b[0x200..0x204].copy_from_slice(&0x1040u32.to_le_bytes());
        b[0x20c..0x210].copy_from_slice(&0x1030u32.to_le_bytes());
        b[0x210..0x214].copy_from_slice(&0x1050u32.to_le_bytes());
        b[0x230..0x23d].copy_from_slice(b"KERNEL32.dll\0");
        b[0x240..0x244].copy_from_slice(&0x1060u32.to_le_bytes());
        b[0x244..0x248].copy_from_slice(&0u32.to_le_bytes());
        b[0x260..0x26d].copy_from_slice(&[
            0, 0, b'E', b'x', b'i', b't', b'P', b'r', b'o', b'c', b'e', b's', b's',
        ]);
        b[0x26d] = 0;
        b
    }

    fn pe64_with_one_relocation_block(block_size: u32) -> Vec<u8> {
        let mut b = pe64();
        let reloc_dir = 0x98 + 112 + 5 * 8;
        b[reloc_dir..reloc_dir + 4].copy_from_slice(&0x1000u32.to_le_bytes());
        b[reloc_dir + 4..reloc_dir + 8].copy_from_slice(&block_size.max(8).to_le_bytes());
        b[0x200..0x204].copy_from_slice(&0x1000u32.to_le_bytes());
        b[0x204..0x208].copy_from_slice(&block_size.to_le_bytes());
        b[0x208..0x20a].copy_from_slice(&0x3000u16.to_le_bytes());
        b[0x20a..0x20c].copy_from_slice(&0u16.to_le_bytes());
        b
    }

    fn pe64_with_one_export() -> Vec<u8> {
        let mut b = pe64();
        let export_dir = 0x98 + 112;
        b[export_dir..export_dir + 4].copy_from_slice(&0x1000u32.to_le_bytes());
        b[export_dir + 4..export_dir + 8].copy_from_slice(&40u32.to_le_bytes());

        // IMAGE_EXPORT_DIRECTORY with one function and one named export.
        b[0x200 + 20..0x200 + 24].copy_from_slice(&1u32.to_le_bytes());
        b[0x200 + 24..0x200 + 28].copy_from_slice(&1u32.to_le_bytes());
        b[0x200 + 28..0x200 + 32].copy_from_slice(&0x1040u32.to_le_bytes());
        b[0x200 + 32..0x200 + 36].copy_from_slice(&0x1044u32.to_le_bytes());
        b[0x200 + 36..0x200 + 40].copy_from_slice(&0x1048u32.to_le_bytes());
        b[0x240..0x244].copy_from_slice(&0x1080u32.to_le_bytes());
        b[0x244..0x248].copy_from_slice(&0x1060u32.to_le_bytes());
        b[0x248..0x24a].copy_from_slice(&0u16.to_le_bytes());
        b[0x260..0x26a].copy_from_slice(b"NexaEntry\0");
        b
    }

    #[test]
    fn extracts_pe32_import_library_and_symbol() {
        let report = inspect(&pe32_with_one_import()).unwrap();
        assert_eq!(report.format, "PE32");
        assert_eq!(report.imports.len(), 1);
        assert_eq!(report.imports[0].name, "KERNEL32.dll");
        assert_eq!(report.imports[0].symbols, vec!["ExitProcess"]);
    }

    #[test]
    fn extracts_pe32_ordinal_import() {
        let mut b = pe32_with_one_import();
        b[0x240..0x244].copy_from_slice(&0x8000_0007u32.to_le_bytes());
        let report = inspect(&b).unwrap();
        assert_eq!(report.imports[0].symbols, vec!["ordinal:7"]);
        assert_eq!(
            report.import_metadata[0].symbols[0].symbol,
            ParsedImportSymbol::Ordinal { ordinal: 7 }
        );
        assert_eq!(report.import_metadata[0].symbols[0].pointer_width, 4);
    }

    #[test]
    fn counts_valid_base_relocation_block() {
        let report = inspect(&pe64_with_one_relocation_block(12)).unwrap();
        assert_eq!(report.relocation_blocks, 1);
        assert_eq!(report.relocation_entries, 2);
    }

    #[test]
    fn rejects_unaligned_base_relocation_page_rva() {
        let mut b = pe64_with_one_relocation_block(12);
        b[0x200..0x204].copy_from_slice(&0x1001u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid(
                "relocation block page RVA is not 4KiB aligned"
            ))
        ));
    }

    #[test]
    fn rejects_base_relocation_block_smaller_than_header() {
        let error = inspect(&pe64_with_one_relocation_block(6)).unwrap_err();
        assert!(matches!(
            error,
            PeError::Invalid("invalid relocation block size")
        ));
    }

    #[test]
    fn rejects_odd_sized_base_relocation_block() {
        let error = inspect(&pe64_with_one_relocation_block(9)).unwrap_err();
        assert!(matches!(
            error,
            PeError::Invalid("invalid relocation block size")
        ));
    }

    #[test]
    fn extracts_valid_named_export() {
        let report = inspect(&pe64_with_one_export()).unwrap();
        assert_eq!(report.exports, vec!["NexaEntry"]);
        let metadata = report.export_metadata.unwrap();
        assert_eq!(metadata.ordinal_base, 0);
        assert_eq!(metadata.functions.len(), 1);
        assert_eq!(metadata.functions[0].function_rva, Some(0x1080));
        assert_eq!(metadata.names[0].name, "NexaEntry");
        assert_eq!(metadata.names[0].function_index, 0);
        assert_eq!(metadata.names[0].ordinal, 0);
    }

    #[test]
    fn retains_export_ordinal_base_sparse_slots_and_forwarders() {
        let mut b = pe64_with_one_export();
        let export_dir = 0x98 + 112;
        b[export_dir + 4..export_dir + 8].copy_from_slice(&0x80u32.to_le_bytes());
        b[0x200 + 16..0x200 + 20].copy_from_slice(&10u32.to_le_bytes());
        b[0x200 + 20..0x200 + 24].copy_from_slice(&3u32.to_le_bytes());
        b[0x200 + 28..0x200 + 32].copy_from_slice(&0x1040u32.to_le_bytes());
        b[0x200 + 32..0x200 + 36].copy_from_slice(&0x104c_u32.to_le_bytes());
        b[0x200 + 36..0x200 + 40].copy_from_slice(&0x1050u32.to_le_bytes());
        b[0x240..0x244].copy_from_slice(&0x1080u32.to_le_bytes());
        b[0x244..0x248].copy_from_slice(&0u32.to_le_bytes());
        b[0x248..0x24c].copy_from_slice(&0x1070u32.to_le_bytes());
        b[0x24c..0x250].copy_from_slice(&0x1060u32.to_le_bytes());
        b[0x250..0x252].copy_from_slice(&2u16.to_le_bytes());
        b[0x260..0x26a].copy_from_slice(b"NexaEntry\0");
        b[0x270..0x27a].copy_from_slice(b"OTHER.Func");
        b[0x27a] = 0;

        let report = inspect(&b).unwrap();
        let metadata = report.export_metadata.unwrap();
        assert_eq!(metadata.ordinal_base, 10);
        assert_eq!(metadata.functions.len(), 3);
        assert_eq!(metadata.functions[0].ordinal, 10);
        assert_eq!(metadata.functions[0].function_rva, Some(0x1080));
        assert_eq!(metadata.functions[1].ordinal, 11);
        assert_eq!(metadata.functions[1].function_rva, None);
        assert_eq!(metadata.functions[2].ordinal, 12);
        assert_eq!(
            metadata.functions[2].forwarder.as_deref(),
            Some("OTHER.Func")
        );
        assert_eq!(metadata.names[0].function_index, 2);
        assert_eq!(metadata.names[0].ordinal, 12);
    }

    #[test]
    fn accepts_export_forwarder_inside_export_directory() {
        let mut b = pe64_with_one_export();
        let export_dir = 0x98 + 112;
        b[export_dir + 4..export_dir + 8].copy_from_slice(&0x80u32.to_le_bytes());
        b[0x240..0x244].copy_from_slice(&0x1070u32.to_le_bytes());
        b[0x270..0x27a].copy_from_slice(b"OTHER.Func");
        b[0x27a] = 0;

        let report = inspect(&b).unwrap();
        assert_eq!(report.exports, vec!["NexaEntry"]);
    }

    #[test]
    fn rejects_unterminated_export_forwarder_at_directory_boundary() {
        let mut b = pe64_with_one_export();
        let export_dir = 0x98 + 112;
        b[export_dir + 4..export_dir + 8].copy_from_slice(&0x78u32.to_le_bytes());
        b[0x240..0x244].copy_from_slice(&0x1070u32.to_le_bytes());
        b[0x270..0x278].copy_from_slice(b"BADNAME!");

        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid("export forwarder"))
        ));
    }

    #[test]
    fn rejects_export_name_ordinal_outside_function_table() {
        let mut b = pe64_with_one_export();
        b[0x248..0x24a].copy_from_slice(&1u16.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid("export name ordinal is out of range"))
        ));
    }

    #[test]
    fn extracts_valid_import_library_and_symbol() {
        let report = inspect(&pe64_with_one_import()).unwrap();
        assert_eq!(report.imports.len(), 1);
        assert_eq!(report.imports[0].name, "KERNEL32.dll");
        assert_eq!(report.imports[0].symbols, vec!["ExitProcess"]);
        assert_eq!(report.import_metadata.len(), 1);
        let library = &report.import_metadata[0];
        assert_eq!(library.original_first_thunk, 0x1040);
        assert_eq!(library.first_thunk, 0x1050);
        assert_eq!(library.symbols[0].lookup_rva, 0x1040);
        assert_eq!(library.symbols[0].iat_rva, 0x1050);
        assert_eq!(library.symbols[0].pointer_width, 8);
        assert_eq!(
            library.symbols[0].symbol,
            ParsedImportSymbol::Name {
                name: "ExitProcess".into(),
                hint: 0
            }
        );
    }

    #[test]
    fn accepts_import_directory_size_covering_other_import_data() {
        let mut b = pe64_with_one_import();
        let import_dir = 0x98 + 112 + 8;
        // Some linkers size this directory over the complete import-data
        // region, not only the IMAGE_IMPORT_DESCRIPTOR array. The descriptor
        // array itself remains terminated by its all-zero entry.
        b[import_dir + 4..import_dir + 8].copy_from_slice(&0x100u32.to_le_bytes());

        let report = inspect(&b).unwrap();
        assert_eq!(report.imports.len(), 1);
        assert_eq!(report.imports[0].name, "KERNEL32.dll");
        assert_eq!(report.imports[0].symbols, vec!["ExitProcess"]);
    }

    #[test]
    fn extracts_pe32_plus_ordinal_import() {
        let mut b = pe64_with_one_import();
        b[0x240..0x248].copy_from_slice(&0x8000_0000_0000_0007u64.to_le_bytes());
        let report = inspect(&b).unwrap();
        assert_eq!(report.imports[0].symbols, vec!["ordinal:7"]);
    }

    #[test]
    fn rejects_pe32_plus_ordinal_import_with_reserved_bits_set() {
        let mut b = pe64_with_one_import();
        b[0x240..0x248].copy_from_slice(&0x8000_0000_0001_0001u64.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid(
                "ordinal import thunk has nonzero reserved bits"
            ))
        ));
    }

    #[test]
    fn rejects_pe32_ordinal_import_with_reserved_bits_set() {
        let mut b = pe32_with_one_import();
        b[0x240..0x244].copy_from_slice(&0x8001_0001u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid(
                "ordinal import thunk has nonzero reserved bits"
            ))
        ));
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
        b[security_dir..security_dir + 4].copy_from_slice(&0x258u32.to_le_bytes());
        b[security_dir + 4..security_dir + 8].copy_from_slice(&0x10u32.to_le_bytes());
        // One 16-byte WIN_CERTIFICATE record; dwLength includes its padding.
        b[0x258..0x25c].copy_from_slice(&16u32.to_le_bytes());
        b[0x25c..0x25e].copy_from_slice(&0x0200u16.to_le_bytes());
        b[0x25e..0x260].copy_from_slice(&0x0002u16.to_le_bytes());

        let report = inspect(&b).unwrap();
        assert_eq!(report.directories[4].address, 0x258);
        assert_eq!(report.directories[4].address_kind, "file_offset");
    }

    #[test]
    fn rejects_unaligned_security_directory_file_offset() {
        let mut b = pe64();
        let security_dir = 0x98 + 112 + 4 * 8;
        b[security_dir..security_dir + 4].copy_from_slice(&0x251u32.to_le_bytes());
        b[security_dir + 4..security_dir + 8].copy_from_slice(&0x10u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid(
                "security directory file offset is not 8-byte aligned"
            ))
        ));
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
    fn rejects_security_directory_size_not_aligned_to_eight_bytes() {
        let mut b = pe64();
        let security_dir = 0x98 + 112 + 4 * 8;
        b[security_dir..security_dir + 4].copy_from_slice(&0x258u32.to_le_bytes());
        b[security_dir + 4..security_dir + 8].copy_from_slice(&9u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid(
                "security directory size is not 8-byte aligned"
            ))
        ));
    }

    #[test]
    fn rejects_security_directory_with_invalid_certificate_length() {
        let mut b = pe64();
        let security_dir = 0x98 + 112 + 4 * 8;
        b[security_dir..security_dir + 4].copy_from_slice(&0x258u32.to_le_bytes());
        b[security_dir + 4..security_dir + 8].copy_from_slice(&8u32.to_le_bytes());
        b[0x258..0x25c].copy_from_slice(&7u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid("invalid WIN_CERTIFICATE length"))
        ));
    }

    #[test]
    fn rejects_certificate_length_that_excludes_alignment_padding() {
        let mut b = pe64();
        let security_dir = 0x98 + 112 + 4 * 8;
        b[security_dir..security_dir + 4].copy_from_slice(&0x258u32.to_le_bytes());
        b[security_dir + 4..security_dir + 8].copy_from_slice(&16u32.to_le_bytes());
        // The physical slot is 16 bytes, but dwLength incorrectly says 12.
        b[0x258..0x25c].copy_from_slice(&12u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid(
                "WIN_CERTIFICATE length does not include 8-byte alignment padding"
            ))
        ));
    }

    #[test]
    fn rejects_certificate_record_that_exceeds_security_directory() {
        let mut b = pe64();
        let security_dir = 0x98 + 112 + 4 * 8;
        b[security_dir..security_dir + 4].copy_from_slice(&0x258u32.to_le_bytes());
        b[security_dir + 4..security_dir + 8].copy_from_slice(&8u32.to_le_bytes());
        b[0x258..0x25c].copy_from_slice(&16u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::OutOfBounds("WIN_CERTIFICATE record"))
        ));
    }

    #[test]
    fn accepts_multiple_aligned_certificate_records() {
        let mut b = pe64();
        let security_dir = 0x98 + 112 + 4 * 8;
        b[security_dir..security_dir + 4].copy_from_slice(&0x258u32.to_le_bytes());
        b[security_dir + 4..security_dir + 8].copy_from_slice(&16u32.to_le_bytes());
        // Two minimal WIN_CERTIFICATE records, each occupying one 8-byte slot.
        b[0x258..0x25c].copy_from_slice(&8u32.to_le_bytes());
        b[0x260..0x264].copy_from_slice(&8u32.to_le_bytes());

        let report = inspect(&b).unwrap();
        assert_eq!(report.directories[4].address_kind, "file_offset");
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
    fn rejects_rva_directory_with_zero_address_and_nonzero_size() {
        let mut b = pe64();
        let resource_dir = 0x98 + 112 + 2 * 8;
        b[resource_dir..resource_dir + 4].copy_from_slice(&0u32.to_le_bytes());
        b[resource_dir + 4..resource_dir + 8].copy_from_slice(&0x20u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid(
                "data directory has an address/size mismatch"
            ))
        ));
    }

    #[test]
    fn rejects_security_directory_with_nonzero_offset_and_zero_size() {
        let mut b = pe64();
        let security_dir = 0x98 + 112 + 4 * 8;
        b[security_dir..security_dir + 4].copy_from_slice(&0x250u32.to_le_bytes());
        b[security_dir + 4..security_dir + 8].copy_from_slice(&0u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid(
                "data directory has an address/size mismatch"
            ))
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
    fn rejects_declared_directory_count_above_physical_storage() {
        let mut b = pe64();
        b[0x98 + 108..0x98 + 112].copy_from_slice(&17u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Truncated("data directory entries"))
        ));
    }

    #[test]
    fn rejects_size_of_headers_before_section_table_end_without_sections() {
        let mut b = pe64();
        b[0x86..0x88].copy_from_slice(&0u16.to_le_bytes());
        b[0x98 + 60..0x98 + 64].copy_from_slice(&0x100u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid("invalid size of headers"))
        ));
    }

    fn pe64_with_debug_entry(
        address_of_raw_data: u32,
        pointer_to_raw_data: u32,
        size_of_data: u32,
        file_size: usize,
    ) -> Vec<u8> {
        let mut b = pe64();
        b.resize(file_size, 0);
        let debug_directory = 0x98 + 112 + 6 * 8;
        b[debug_directory..debug_directory + 4].copy_from_slice(&0x1000u32.to_le_bytes());
        b[debug_directory + 4..debug_directory + 8].copy_from_slice(&28u32.to_le_bytes());

        // IMAGE_DEBUG_DIRECTORY at file offset 0x200.
        b[0x200..0x204].copy_from_slice(&0u32.to_le_bytes());
        b[0x204..0x208].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        b[0x208..0x20a].copy_from_slice(&1u16.to_le_bytes());
        b[0x20a..0x20c].copy_from_slice(&2u16.to_le_bytes());
        b[0x20c..0x210].copy_from_slice(&2u32.to_le_bytes());
        b[0x210..0x214].copy_from_slice(&size_of_data.to_le_bytes());
        b[0x214..0x218].copy_from_slice(&address_of_raw_data.to_le_bytes());
        b[0x218..0x21c].copy_from_slice(&pointer_to_raw_data.to_le_bytes());
        b
    }

    #[test]
    fn reports_mapped_debug_data_rva_and_file_pointer_separately() {
        let b = pe64_with_debug_entry(0x1080, 0x288, 8, 0x300);
        let report = inspect(&b).unwrap();

        assert_eq!(report.debug_entries.len(), 1);
        let entry = &report.debug_entries[0];
        assert_eq!(entry.type_code, 2);
        assert_eq!(entry.size_of_data, 8);
        assert_eq!(entry.address_of_raw_data, 0x1080);
        assert_eq!(entry.pointer_to_raw_data, 0x288);
        // The mapped RVA resolves to 0x280; the independent raw file pointer
        // remains 0x288 and is not silently substituted.
        assert_eq!(entry.mapped_file_offset, Some(0x280));
    }

    #[test]
    fn reports_unmapped_debug_data_as_file_only_overlay_bytes() {
        let b = pe64_with_debug_entry(0, 0x300, 0x10, 0x310);
        let report = inspect(&b).unwrap();

        assert_eq!(report.debug_entries.len(), 1);
        assert_eq!(report.debug_entries[0].address_of_raw_data, 0);
        assert_eq!(report.debug_entries[0].pointer_to_raw_data, 0x300);
        assert_eq!(report.debug_entries[0].mapped_file_offset, None);
        assert_eq!(report.overlay.as_ref().unwrap().offset, 0x300);
        assert_eq!(report.overlay.as_ref().unwrap().size, 0x10);
        assert_eq!(report.file_size, 0x310);
    }

    #[test]
    fn reports_trailing_file_overlay_after_section_raw_data() {
        let mut b = pe64();
        b.resize(0x340, 0xa5);

        let report = inspect(&b).unwrap();

        assert_eq!(report.file_size, 0x340);
        assert_eq!(report.overlay.as_ref().unwrap().offset, 0x300);
        assert_eq!(report.overlay.as_ref().unwrap().size, 0x40);
    }

    #[test]
    fn rejects_debug_data_file_pointer_outside_input() {
        let b = pe64_with_debug_entry(0, 0x300, 0x10, 0x308);

        assert!(matches!(
            inspect(&b),
            Err(PeError::Truncated("debug data file range"))
        ));
    }

    #[test]
    fn rejects_debug_directory_size_not_divisible_by_entry_size() {
        let mut b = pe64();
        let debug_directory = 0x98 + 112 + 6 * 8;
        b[debug_directory..debug_directory + 4].copy_from_slice(&0x1000u32.to_le_bytes());
        b[debug_directory + 4..debug_directory + 8].copy_from_slice(&29u32.to_le_bytes());

        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid(
                "debug directory size is not a multiple of entry size"
            ))
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
    fn rejects_pe_header_offset_inside_dos_header() {
        let mut b = pe64();
        b[0x3c..0x40].copy_from_slice(&0x20u32.to_le_bytes());
        b[0x20..0x24].copy_from_slice(b"PE\0\0");
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid("PE header overlaps the DOS header"))
        ));
    }

    #[test]
    fn rejects_pe32_plus_with_32_bit_x86_machine() {
        let mut b = pe64();
        b[0x84..0x86].copy_from_slice(&0x014cu16.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid(
                "machine type conflicts with optional header format"
            ))
        ));
    }

    #[test]
    fn rejects_pe32_with_64_bit_amd64_machine() {
        let mut b = pe32();
        b[0x84..0x86].copy_from_slice(&0x8664u16.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid(
                "machine type conflicts with optional header format"
            ))
        ));
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
    fn rejects_section_virtual_range_exceeding_rva_address_space() {
        let mut b = pe64();
        let section = 0x188;
        b[section + 12..section + 16].copy_from_slice(&0xffff_f000u32.to_le_bytes());
        b[section + 8..section + 12].copy_from_slice(&0x2000u32.to_le_bytes());
        b[section + 16..section + 20].copy_from_slice(&0u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::OutOfBounds(
                "section RVA range exceeds 32-bit address space"
            ))
        ));
    }

    #[test]
    fn rejects_section_virtual_range_overlapping_headers() {
        let mut b = pe64();
        b[0x188 + 12..0x188 + 16].copy_from_slice(&0x180u32.to_le_bytes());
        assert!(matches!(
            inspect(&b),
            Err(PeError::Invalid("section virtual range overlaps headers"))
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
    #[test]
    fn maps_valid_rva_range_when_section_ends_at_32_bit_boundary() {
        let mut b = pe64();
        b.resize(0x400, 0);
        let section = 0x188;
        b[section + 8..section + 12].copy_from_slice(&0x200u32.to_le_bytes());
        b[section + 12..section + 16].copy_from_slice(&0xffff_fe00u32.to_le_bytes());
        b[section + 16..section + 20].copy_from_slice(&0x200u32.to_le_bytes());
        b[section + 20..section + 24].copy_from_slice(&0x200u32.to_le_bytes());

        // This valid directory is inside the section, whose exclusive end is 2^32.
        let resource_dir = 0x98 + 112 + 2 * 8;
        b[resource_dir..resource_dir + 4].copy_from_slice(&0xffff_ff00u32.to_le_bytes());
        b[resource_dir + 4..resource_dir + 8].copy_from_slice(&0x10u32.to_le_bytes());

        let report = inspect(&b).unwrap();
        assert_eq!(report.directories[2].address, 0xffff_ff00);
    }
}
