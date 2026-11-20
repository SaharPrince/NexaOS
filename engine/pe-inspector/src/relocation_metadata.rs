//! Lossless, bounded metadata for PE base-relocation directories.
//!
//! This module stores encoded relocation entries without deciding which
//! relocation types are supported. Relocation planning applies that policy.

use serde::Serialize;

/// A validated base-relocation directory and its ordered blocks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParsedRelocationDirectory {
    /// RVA declared by IMAGE_DIRECTORY_ENTRY_BASERELOC.
    pub directory_rva: u32,
    /// Size declared by IMAGE_DIRECTORY_ENTRY_BASERELOC.
    pub directory_size: u32,
    /// Blocks in their original file order.
    pub blocks: Vec<ParsedRelocationBlock>,
}

/// One IMAGE_BASE_RELOCATION block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParsedRelocationBlock {
    /// 4-KiB-aligned page RVA from the block header.
    pub page_rva: u32,
    /// Encoded entries in their original order.
    pub entries: Vec<ParsedRelocationEntry>,
}

/// One 16-bit relocation slot, preserving its exact encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ParsedRelocationEntry {
    /// Original 16-bit IMAGE_RELOCATION entry.
    pub raw: u16,
    /// High four bits of the encoded entry. Unknown values are retained.
    pub relocation_type: u8,
    /// Low twelve bits of the encoded entry.
    pub offset: u16,
}
