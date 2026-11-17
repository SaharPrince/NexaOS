//! Bounded, immutable export catalogue for caller-supplied PE metadata.
//!
//! This stage performs lookup only. It does not load modules, discover files,
//! follow forwarders, or convert RVAs into process addresses.

use crate::ParsedExportDirectory;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportCatalogueLimits {
    pub max_modules: usize,
    pub max_exports_per_module: usize,
    pub max_module_name_bytes: usize,
    pub max_export_name_bytes: usize,
    pub max_forwarder_bytes: usize,
}

impl Default for ExportCatalogueLimits {
    fn default() -> Self {
        Self {
            max_modules: 256,
            max_exports_per_module: 16_384,
            max_module_name_bytes: 512,
            max_export_name_bytes: 512,
            max_forwarder_bytes: 4096,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportModule {
    pub name: String,
    pub directory: ParsedExportDirectory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportCatalogue {
    modules: Vec<ExportModule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportLookup {
    Name(String),
    Ordinal(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportLookupResult {
    Resolved { ordinal: u32, function_rva: u32 },
    Forwarder { ordinal: u32, forwarder: String },
    MissingModule,
    MissingSymbol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportCatalogueError {
    TooManyModules,
    EmptyModuleName {
        index: usize,
    },
    ModuleNameTooLong {
        index: usize,
    },
    DuplicateModuleName {
        index: usize,
    },
    TooManyExports {
        index: usize,
    },
    ExportNameTooLong {
        module_index: usize,
        export_index: usize,
    },
    DuplicateExportName {
        module_index: usize,
        export_index: usize,
    },
    ForwarderTooLong {
        module_index: usize,
        function_index: usize,
    },
    InvalidNameAssociation {
        module_index: usize,
        name_index: usize,
    },
    InvalidOrdinalSequence {
        module_index: usize,
        function_index: usize,
    },
}

impl fmt::Display for ExportCatalogueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid export catalogue: {self:?}")
    }
}

impl std::error::Error for ExportCatalogueError {}

impl ExportCatalogue {
    pub fn new(
        modules: Vec<ExportModule>,
        limits: ExportCatalogueLimits,
    ) -> Result<Self, ExportCatalogueError> {
        if modules.len() > limits.max_modules {
            return Err(ExportCatalogueError::TooManyModules);
        }

        for (index, module) in modules.iter().enumerate() {
            if module.name.is_empty() {
                return Err(ExportCatalogueError::EmptyModuleName { index });
            }
            if module.name.len() > limits.max_module_name_bytes {
                return Err(ExportCatalogueError::ModuleNameTooLong { index });
            }
            if modules[..index]
                .iter()
                .any(|previous| previous.name.eq_ignore_ascii_case(&module.name))
            {
                return Err(ExportCatalogueError::DuplicateModuleName { index });
            }

            let directory = &module.directory;
            if directory.functions.len() > limits.max_exports_per_module
                || directory.names.len() > limits.max_exports_per_module
            {
                return Err(ExportCatalogueError::TooManyExports { index });
            }

            for (name_index, association) in directory.names.iter().enumerate() {
                if association.name.len() > limits.max_export_name_bytes {
                    return Err(ExportCatalogueError::ExportNameTooLong {
                        module_index: index,
                        export_index: name_index,
                    });
                }
                if directory.names[..name_index]
                    .iter()
                    .any(|previous| previous.name == association.name)
                {
                    return Err(ExportCatalogueError::DuplicateExportName {
                        module_index: index,
                        export_index: name_index,
                    });
                }
                let slot = usize::try_from(association.function_index).ok();
                let Some(slot) = slot.filter(|slot| *slot < directory.functions.len()) else {
                    return Err(ExportCatalogueError::InvalidNameAssociation {
                        module_index: index,
                        name_index,
                    });
                };
                if directory.functions[slot].ordinal != association.ordinal {
                    return Err(ExportCatalogueError::InvalidNameAssociation {
                        module_index: index,
                        name_index,
                    });
                }
            }

            for (function_index, slot) in directory.functions.iter().enumerate() {
                if slot
                    .forwarder
                    .as_ref()
                    .is_some_and(|forwarder| forwarder.len() > limits.max_forwarder_bytes)
                {
                    return Err(ExportCatalogueError::ForwarderTooLong {
                        module_index: index,
                        function_index,
                    });
                }
                let index_u32 = u32::try_from(function_index).map_err(|_| {
                    ExportCatalogueError::InvalidOrdinalSequence {
                        module_index: index,
                        function_index,
                    }
                })?;
                if directory.ordinal_base.checked_add(index_u32) != Some(slot.ordinal) {
                    return Err(ExportCatalogueError::InvalidOrdinalSequence {
                        module_index: index,
                        function_index,
                    });
                }
            }
        }

        Ok(Self { modules })
    }

    /// Module names are ASCII case-insensitive; export names are exact-case.
    pub fn lookup(&self, module_name: &str, symbol: &ExportLookup) -> ExportLookupResult {
        let Some(module) = self
            .modules
            .iter()
            .find(|module| module.name.eq_ignore_ascii_case(module_name))
        else {
            return ExportLookupResult::MissingModule;
        };

        let slot = match symbol {
            ExportLookup::Name(name) => {
                let Some(association) = module
                    .directory
                    .names
                    .iter()
                    .find(|association| association.name == *name)
                else {
                    return ExportLookupResult::MissingSymbol;
                };
                let Ok(index) = usize::try_from(association.function_index) else {
                    return ExportLookupResult::MissingSymbol;
                };
                let Some(slot) = module.directory.functions.get(index) else {
                    return ExportLookupResult::MissingSymbol;
                };
                slot
            }
            ExportLookup::Ordinal(ordinal) => {
                let Some(index) = ordinal.checked_sub(module.directory.ordinal_base) else {
                    return ExportLookupResult::MissingSymbol;
                };
                let Ok(index) = usize::try_from(index) else {
                    return ExportLookupResult::MissingSymbol;
                };
                let Some(slot) = module.directory.functions.get(index) else {
                    return ExportLookupResult::MissingSymbol;
                };
                slot
            }
        };

        if let Some(forwarder) = &slot.forwarder {
            ExportLookupResult::Forwarder {
                ordinal: slot.ordinal,
                forwarder: forwarder.clone(),
            }
        } else if let Some(function_rva) = slot.function_rva {
            ExportLookupResult::Resolved {
                ordinal: slot.ordinal,
                function_rva,
            }
        } else {
            ExportLookupResult::MissingSymbol
        }
    }

    pub fn module_count(&self) -> usize {
        self.modules.len()
    }
}

/// Limits for resolving one parsed import table against an explicit catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportResolutionLimits {
    pub max_libraries: usize,
    pub max_symbols: usize,
    pub max_module_name_bytes: usize,
    pub max_symbol_name_bytes: usize,
    pub max_forwarder_hops: usize,
}

impl Default for ImportResolutionLimits {
    fn default() -> Self {
        Self {
            max_libraries: 256,
            max_symbols: 65_536,
            max_module_name_bytes: 512,
            max_symbol_name_bytes: 512,
            max_forwarder_hops: 16,
        }
    }
}

/// One resolved import request. The target remains an RVA in its named module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImport {
    pub import_library: String,
    pub import_symbol: crate::ParsedImportSymbol,
    pub iat_rva: u32,
    pub pointer_width: u8,
    pub target_module: String,
    pub export_ordinal: u32,
    pub function_rva: u32,
    pub forwarder_hops: usize,
}

/// Deterministic resolution output; it contains no process addresses or writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportResolutionPlan {
    pub imports: Vec<ResolvedImport>,
}

/// A specific failure while resolving an import request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportResolutionFailure {
    TooManyLibraries,
    TooManySymbols,
    EmptyModuleName,
    ModuleNameTooLong,
    SymbolNameTooLong,
    UnsupportedPointerWidth { pointer_width: u8 },
    IatRangeOverflow,
    MissingModule,
    MissingSymbol,
    InvalidForwarder,
    ForwarderCycle,
    ForwarderHopLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportResolutionError {
    pub library_index: usize,
    pub symbol_index: usize,
    pub failure: ImportResolutionFailure,
}

impl fmt::Display for ImportResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "import resolution failed at library {} symbol {}: {:?}",
            self.library_index, self.symbol_index, self.failure
        )
    }
}

impl std::error::Error for ImportResolutionError {}

fn resolution_error(
    library_index: usize,
    symbol_index: usize,
    failure: ImportResolutionFailure,
) -> ImportResolutionError {
    ImportResolutionError {
        library_index,
        symbol_index,
        failure,
    }
}

fn parse_forwarder(
    value: &str,
    limits: ImportResolutionLimits,
    library_index: usize,
    symbol_index: usize,
) -> Result<(String, ExportLookup), ImportResolutionError> {
    let Some(separator) = value.rfind('.') else {
        return Err(resolution_error(
            library_index,
            symbol_index,
            ImportResolutionFailure::InvalidForwarder,
        ));
    };
    let module = &value[..separator];
    let target = &value[separator + 1..];
    if module.is_empty()
        || target.is_empty()
        || module.len() > limits.max_module_name_bytes
        || module.contains('/')
        || module.contains('\\\\')
        || module.contains(':')
        || target.len() > limits.max_symbol_name_bytes
    {
        return Err(resolution_error(
            library_index,
            symbol_index,
            ImportResolutionFailure::InvalidForwarder,
        ));
    }

    let lookup = if let Some(ordinal) = target.strip_prefix('#') {
        let ordinal = ordinal.parse::<u32>().map_err(|_| {
            resolution_error(
                library_index,
                symbol_index,
                ImportResolutionFailure::InvalidForwarder,
            )
        })?;
        ExportLookup::Ordinal(ordinal)
    } else {
        ExportLookup::Name(target.to_owned())
    };
    Ok((module.to_owned(), lookup))
}

fn same_forwarder_target(
    left: &(String, ExportLookup),
    module: &str,
    symbol: &ExportLookup,
) -> bool {
    left.0.eq_ignore_ascii_case(module) && left.1 == *symbol
}

/// Resolve all imports using only the caller-supplied export catalogue.
///
/// Module names are matched case-insensitively, export names exactly, and
/// forwarders are followed only through entries already present in the
/// catalogue. No filesystem discovery, DLL loading, address rebasing, or
/// IAT modification occurs. Forwarder module names must match catalogue keys;
/// this function does not append extensions or search for aliases.
pub fn resolve_imports(
    imports: &[crate::ParsedImportLibrary],
    catalogue: &ExportCatalogue,
    limits: ImportResolutionLimits,
) -> Result<ImportResolutionPlan, ImportResolutionError> {
    if imports.len() > limits.max_libraries {
        return Err(resolution_error(
            0,
            0,
            ImportResolutionFailure::TooManyLibraries,
        ));
    }

    let mut total_symbols = 0usize;
    let mut resolved = Vec::new();

    for (library_index, library) in imports.iter().enumerate() {
        if library.name.is_empty() {
            return Err(resolution_error(
                library_index,
                0,
                ImportResolutionFailure::EmptyModuleName,
            ));
        }
        if library.name.len() > limits.max_module_name_bytes {
            return Err(resolution_error(
                library_index,
                0,
                ImportResolutionFailure::ModuleNameTooLong,
            ));
        }
        total_symbols = total_symbols.checked_add(library.symbols.len()).ok_or_else(|| {
            resolution_error(
                library_index,
                0,
                ImportResolutionFailure::TooManySymbols,
            )
        })?;
        if total_symbols > limits.max_symbols {
            return Err(resolution_error(
                library_index,
                0,
                ImportResolutionFailure::TooManySymbols,
            ));
        }

        for (symbol_index, import) in library.symbols.iter().enumerate() {
            if import.pointer_width != 8 {
                return Err(resolution_error(
                    library_index,
                    symbol_index,
                    ImportResolutionFailure::UnsupportedPointerWidth {
                        pointer_width: import.pointer_width,
                    },
                ));
            }
            if import.iat_rva.checked_add(u32::from(import.pointer_width)).is_none() {
                return Err(resolution_error(
                    library_index,
                    symbol_index,
                    ImportResolutionFailure::IatRangeOverflow,
                ));
            }

            let initial_lookup = match &import.symbol {
                crate::ParsedImportSymbol::Name { name, .. } => {
                    if name.len() > limits.max_symbol_name_bytes {
                        return Err(resolution_error(
                            library_index,
                            symbol_index,
                            ImportResolutionFailure::SymbolNameTooLong,
                        ));
                    }
                    ExportLookup::Name(name.clone())
                }
                crate::ParsedImportSymbol::Ordinal { ordinal } => {
                    ExportLookup::Ordinal(u32::from(*ordinal))
                }
            };

            let mut current_module = library.name.clone();
            let mut current_lookup = initial_lookup;
            let mut visited: Vec<(String, ExportLookup)> = Vec::new();
            let mut forwarder_hops = 0usize;

            let (target_module, export_ordinal, function_rva) = loop {
                if visited.iter().any(|entry| {
                    same_forwarder_target(entry, &current_module, &current_lookup)
                }) {
                    return Err(resolution_error(
                        library_index,
                        symbol_index,
                        ImportResolutionFailure::ForwarderCycle,
                    ));
                }
                visited.push((current_module.clone(), current_lookup.clone()));

                match catalogue.lookup(&current_module, &current_lookup) {
                    ExportLookupResult::Resolved {
                        ordinal,
                        function_rva,
                    } => break (current_module, ordinal, function_rva),
                    ExportLookupResult::MissingModule => {
                        return Err(resolution_error(
                            library_index,
                            symbol_index,
                            ImportResolutionFailure::MissingModule,
                        ));
                    }
                    ExportLookupResult::MissingSymbol => {
                        return Err(resolution_error(
                            library_index,
                            symbol_index,
                            ImportResolutionFailure::MissingSymbol,
                        ));
                    }
                    ExportLookupResult::Forwarder { forwarder, .. } => {
                        if forwarder_hops >= limits.max_forwarder_hops {
                            return Err(resolution_error(
                                library_index,
                                symbol_index,
                                ImportResolutionFailure::ForwarderHopLimit,
                            ));
                        }
                        let (module, lookup) = parse_forwarder(
                            &forwarder,
                            limits,
                            library_index,
                            symbol_index,
                        )?;
                        current_module = module;
                        current_lookup = lookup;
                        forwarder_hops += 1;
                    }
                }
            };

            resolved.push(ResolvedImport {
                import_library: library.name.clone(),
                import_symbol: import.symbol.clone(),
                iat_rva: import.iat_rva,
                pointer_width: import.pointer_width,
                target_module,
                export_ordinal,
                function_rva,
                forwarder_hops,
            });
        }
    }

    Ok(ImportResolutionPlan { imports: resolved })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExportFunctionSlot, ExportNameAssociation};

    fn module() -> ExportModule {
        ExportModule {
            name: "KERNEL32.dll".into(),
            directory: ParsedExportDirectory {
                module_name: Some("KERNEL32.dll".into()),
                ordinal_base: 10,
                functions: vec![
                    ExportFunctionSlot {
                        ordinal: 10,
                        function_rva: Some(0x1000),
                        forwarder: None,
                    },
                    ExportFunctionSlot {
                        ordinal: 11,
                        function_rva: None,
                        forwarder: None,
                    },
                    ExportFunctionSlot {
                        ordinal: 12,
                        function_rva: None,
                        forwarder: Some("OTHER.Func".into()),
                    },
                ],
                names: vec![ExportNameAssociation {
                    name: "ExitProcess".into(),
                    function_index: 0,
                    ordinal: 10,
                }],
            },
        }
    }

    fn catalogue() -> ExportCatalogue {
        ExportCatalogue::new(vec![module()], ExportCatalogueLimits::default()).unwrap()
    }

    #[test]
    fn matches_module_case_insensitively_and_export_names_exactly() {
        let catalogue = catalogue();
        assert_eq!(
            catalogue.lookup("kernel32.DLL", &ExportLookup::Name("ExitProcess".into())),
            ExportLookupResult::Resolved {
                ordinal: 10,
                function_rva: 0x1000
            }
        );
        assert_eq!(
            catalogue.lookup("KERNEL32.dll", &ExportLookup::Name("exitprocess".into())),
            ExportLookupResult::MissingSymbol
        );
    }

    #[test]
    fn resolves_public_ordinals_using_export_base() {
        assert_eq!(
            catalogue().lookup("KERNEL32.dll", &ExportLookup::Ordinal(10)),
            ExportLookupResult::Resolved {
                ordinal: 10,
                function_rva: 0x1000
            }
        );
    }

    #[test]
    fn distinguishes_sparse_slots_and_forwarders() {
        let catalogue = catalogue();
        assert_eq!(
            catalogue.lookup("KERNEL32.dll", &ExportLookup::Ordinal(11)),
            ExportLookupResult::MissingSymbol
        );
        assert_eq!(
            catalogue.lookup("KERNEL32.dll", &ExportLookup::Ordinal(12)),
            ExportLookupResult::Forwarder {
                ordinal: 12,
                forwarder: "OTHER.Func".into()
            }
        );
    }

    #[test]
    fn distinguishes_missing_modules_from_missing_symbols() {
        let catalogue = catalogue();
        assert_eq!(
            catalogue.lookup("USER32.dll", &ExportLookup::Name("MessageBoxW".into())),
            ExportLookupResult::MissingModule
        );
        assert_eq!(
            catalogue.lookup("KERNEL32.dll", &ExportLookup::Name("Missing".into())),
            ExportLookupResult::MissingSymbol
        );
    }

    #[test]
    fn rejects_case_insensitive_duplicate_modules() {
        let duplicate = ExportModule {
            name: "kernel32.DLL".into(),
            directory: module().directory,
        };
        let error =
            ExportCatalogue::new(vec![module(), duplicate], ExportCatalogueLimits::default())
                .unwrap_err();
        assert_eq!(
            error,
            ExportCatalogueError::DuplicateModuleName { index: 1 }
        );
    }

    #[test]
    fn rejects_duplicate_export_names() {
        let bad = module();
        let mut bad = bad;
        bad.directory.names.push(bad.directory.names[0].clone());
        let error = ExportCatalogue::new(vec![bad], ExportCatalogueLimits::default()).unwrap_err();
        assert_eq!(
            error,
            ExportCatalogueError::DuplicateExportName {
                module_index: 0,
                export_index: 1
            }
        );
    }

    #[test]
    fn rejects_forwarders_over_configured_string_limit() {
        let mut bad = module();
        let limits = ExportCatalogueLimits {
            max_forwarder_bytes: 4,
            ..ExportCatalogueLimits::default()
        };
        let error = ExportCatalogue::new(vec![bad.clone()], limits).unwrap_err();
        assert_eq!(
            error,
            ExportCatalogueError::ForwarderTooLong {
                module_index: 0,
                function_index: 2
            }
        );
        bad.directory.functions[2].forwarder = Some("OK".into());
        assert!(ExportCatalogue::new(vec![bad], limits).is_ok());
    }

    fn import_library(symbol: crate::ParsedImportSymbol) -> crate::ParsedImportLibrary {
        crate::ParsedImportLibrary {
            name: "KERNEL32.dll".into(),
            original_first_thunk: 0x2000,
            first_thunk: 0x3000,
            symbols: vec![crate::ParsedImportEntry {
                symbol,
                lookup_rva: 0x2000,
                iat_rva: 0x3000,
                pointer_width: 8,
            }],
        }
    }

    fn forwarder_module(name: &str, target: &str) -> ExportModule {
        ExportModule {
            name: name.into(),
            directory: ParsedExportDirectory {
                module_name: Some(name.into()),
                ordinal_base: 1,
                functions: vec![ExportFunctionSlot {
                    ordinal: 1,
                    function_rva: None,
                    forwarder: Some(target.into()),
                }],
                names: vec![ExportNameAssociation {
                    name: "Func".into(),
                    function_index: 0,
                    ordinal: 1,
                }],
            },
        }
    }

    fn target_module() -> ExportModule {
        ExportModule {
            name: "OTHER".into(),
            directory: ParsedExportDirectory {
                module_name: Some("OTHER".into()),
                ordinal_base: 20,
                functions: vec![ExportFunctionSlot {
                    ordinal: 20,
                    function_rva: Some(0x2200),
                    forwarder: None,
                }],
                names: vec![ExportNameAssociation {
                    name: "Func".into(),
                    function_index: 0,
                    ordinal: 20,
                }],
            },
        }
    }

    #[test]
    fn resolves_named_imports_to_module_rvas_without_writing_iat() {
        let plan = resolve_imports(
            &[import_library(crate::ParsedImportSymbol::Name {
                name: "ExitProcess".into(),
                hint: 0,
            })],
            &catalogue(),
            ImportResolutionLimits::default(),
        )
        .unwrap();
        assert_eq!(plan.imports.len(), 1);
        assert_eq!(plan.imports[0].iat_rva, 0x3000);
        assert_eq!(plan.imports[0].pointer_width, 8);
        assert_eq!(plan.imports[0].target_module, "KERNEL32.dll");
        assert_eq!(plan.imports[0].function_rva, 0x1000);
        assert_eq!(plan.imports[0].forwarder_hops, 0);
    }

    #[test]
    fn follows_named_forwarders_only_through_supplied_catalogue() {
        let catalogue = ExportCatalogue::new(
            vec![module(), target_module()],
            ExportCatalogueLimits::default(),
        )
        .unwrap();
        let mut import = import_library(crate::ParsedImportSymbol::Ordinal { ordinal: 12 });
        let plan = resolve_imports(
            &[import.clone()],
            &catalogue,
            ImportResolutionLimits::default(),
        )
        .unwrap();
        assert_eq!(plan.imports[0].target_module, "OTHER");
        assert_eq!(plan.imports[0].export_ordinal, 20);
        assert_eq!(plan.imports[0].function_rva, 0x2200);
        assert_eq!(plan.imports[0].forwarder_hops, 1);

        import.symbols[0].symbol = crate::ParsedImportSymbol::Ordinal { ordinal: 12 };
        let mut source = module();
        source.directory.functions[2].forwarder = Some("OTHER.#20".into());
        let ordinal_catalogue =
            ExportCatalogue::new(vec![source, target_module()], ExportCatalogueLimits::default())
                .unwrap();
        let ordinal_plan = resolve_imports(
            &[import],
            &ordinal_catalogue,
            ImportResolutionLimits::default(),
        )
        .unwrap();
        assert_eq!(ordinal_plan.imports[0].function_rva, 0x2200);
    }

    #[test]
    fn rejects_forwarder_cycles_and_hop_limit_exhaustion() {
        let cycle_catalogue = ExportCatalogue::new(
            vec![
                forwarder_module("A.dll", "B.Func"),
                forwarder_module("B.dll", "A.Func"),
            ],
            ExportCatalogueLimits::default(),
        )
        .unwrap();
        let cycle_import = crate::ParsedImportLibrary {
            name: "A.dll".into(),
            original_first_thunk: 0x2000,
            first_thunk: 0x3000,
            symbols: vec![crate::ParsedImportEntry {
                symbol: crate::ParsedImportSymbol::Name {
                    name: "Func".into(),
                    hint: 0,
                },
                lookup_rva: 0x2000,
                iat_rva: 0x3000,
                pointer_width: 8,
            }],
        };
        assert_eq!(
            resolve_imports(
                &[cycle_import],
                &cycle_catalogue,
                ImportResolutionLimits::default()
            )
            .unwrap_err()
            .failure,
            ImportResolutionFailure::ForwarderCycle
        );

        assert_eq!(
            resolve_imports(
                &[import_library(crate::ParsedImportSymbol::Ordinal { ordinal: 12 })],
                &ExportCatalogue::new(
                    vec![module(), target_module()],
                    ExportCatalogueLimits::default()
                )
                .unwrap(),
                ImportResolutionLimits {
                    max_forwarder_hops: 0,
                    ..ImportResolutionLimits::default()
                }
            )
            .unwrap_err()
            .failure,
            ImportResolutionFailure::ForwarderHopLimit
        );
    }

    #[test]
    fn rejects_missing_imports_invalid_forwarders_and_non_amd64_widths() {
        let missing = resolve_imports(
            &[import_library(crate::ParsedImportSymbol::Name {
                name: "NotThere".into(),
                hint: 0,
            })],
            &catalogue(),
            ImportResolutionLimits::default(),
        )
        .unwrap_err();
        assert_eq!(missing.failure, ImportResolutionFailure::MissingSymbol);

        let mut bad_forwarder = module();
        bad_forwarder.directory.functions[2].forwarder = Some("not-a-forwarder".into());
        let bad_catalogue =
            ExportCatalogue::new(vec![bad_forwarder], ExportCatalogueLimits::default()).unwrap();
        let invalid = resolve_imports(
            &[import_library(crate::ParsedImportSymbol::Ordinal { ordinal: 12 })],
            &bad_catalogue,
            ImportResolutionLimits::default(),
        )
        .unwrap_err();
        assert_eq!(invalid.failure, ImportResolutionFailure::InvalidForwarder);

        let mut pe32_import = import_library(crate::ParsedImportSymbol::Ordinal { ordinal: 10 });
        pe32_import.symbols[0].pointer_width = 4;
        let width = resolve_imports(
            &[pe32_import],
            &catalogue(),
            ImportResolutionLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            width.failure,
            ImportResolutionFailure::UnsupportedPointerWidth { pointer_width: 4 }
        );
    }

    #[test]
    fn rejects_inconsistent_name_associations() {
        let mut bad = module();
        bad.directory.names[0].function_index = 2;
        let error = ExportCatalogue::new(vec![bad], ExportCatalogueLimits::default()).unwrap_err();
        assert_eq!(
            error,
            ExportCatalogueError::InvalidNameAssociation {
                module_index: 0,
                name_index: 0
            }
        );
    }
}
