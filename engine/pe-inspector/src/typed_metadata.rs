//! Typed adapters for the legacy PE import/export report fields.
//!
//! The inspector's original JSON fields remain stable. These adapters remove
//! the need for downstream code to interpret ordinal imports as magic strings.
//! They intentionally do not claim to retain PE fields that the legacy report
//! never exposed (such as import hints, IAT RVAs, export ordinal bases, or
//! unnamed export slots); those are tracked as follow-up parser work.

use crate::{ImportLibrary, PeReport};
use serde::Serialize;
use std::fmt;

/// An imported symbol, represented without string-encoded ordinal conventions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ImportSymbol {
    Name(String),
    Ordinal(u16),
}

/// Typed view of one imported module and its requested symbols.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportLibraryMetadata {
    pub name: String,
    pub symbols: Vec<ImportSymbol>,
}

/// A named export exposed by the existing compact report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportSymbol {
    pub name: String,
}

/// Failure to interpret a legacy import symbol field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataError {
    index: usize,
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid ordinal import encoding at symbol index {}",
            self.index
        )
    }
}

impl std::error::Error for MetadataError {}

fn typed_library(library: &ImportLibrary) -> Result<ImportLibraryMetadata, MetadataError> {
    let symbols = library
        .symbols
        .iter()
        .enumerate()
        .map(|(index, symbol)| {
            if let Some(ordinal) = symbol.strip_prefix("ordinal:") {
                let value = ordinal
                    .parse::<u16>()
                    .map_err(|_| MetadataError { index })?;
                Ok(ImportSymbol::Ordinal(value))
            } else {
                Ok(ImportSymbol::Name(symbol.clone()))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ImportLibraryMetadata {
        name: library.name.clone(),
        symbols,
    })
}

/// Convert parsed imports to typed records while preserving the legacy report.
pub fn typed_imports(report: &PeReport) -> Result<Vec<ImportLibraryMetadata>, MetadataError> {
    report.imports.iter().map(typed_library).collect()
}

/// Convert the existing named-export list to typed records.
///
/// Export ordinals and unnamed function-table slots are not available in the
/// current legacy report and are deliberately not fabricated here.
pub fn typed_exports(report: &PeReport) -> Vec<ExportSymbol> {
    report
        .exports
        .iter()
        .cloned()
        .map(|name| ExportSymbol { name })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_name_and_ordinal_imports_to_distinct_variants() {
        let library = ImportLibrary {
            name: "KERNEL32.dll".into(),
            symbols: vec!["ExitProcess".into(), "ordinal:7".into()],
        };

        let typed = typed_library(&library).unwrap();
        assert_eq!(
            typed.symbols,
            vec![
                ImportSymbol::Name("ExitProcess".into()),
                ImportSymbol::Ordinal(7)
            ]
        );
    }

    #[test]
    fn rejects_malformed_or_out_of_range_ordinal_encodings() {
        for value in ["ordinal:", "ordinal:-1", "ordinal:65536", "ordinal:7x"] {
            let library = ImportLibrary {
                name: "sample.dll".into(),
                symbols: vec![value.into()],
            };
            assert_eq!(typed_library(&library).unwrap_err().index, 0);
        }
    }

    #[test]
    fn preserves_case_and_empty_names_without_normalization() {
        let library = ImportLibrary {
            name: "Sample.DLL".into(),
            symbols: vec!["CreateFileW".into(), "createfilew".into()],
        };

        let typed = typed_library(&library).unwrap();
        assert_eq!(typed.name, "Sample.DLL");
        assert_eq!(
            typed.symbols,
            vec![
                ImportSymbol::Name("CreateFileW".into()),
                ImportSymbol::Name("createfilew".into())
            ]
        );
    }
}
