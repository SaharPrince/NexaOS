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
    EmptyModuleName { index: usize },
    ModuleNameTooLong { index: usize },
    DuplicateModuleName { index: usize },
    TooManyExports { index: usize },
    ExportNameTooLong { module_index: usize, export_index: usize },
    DuplicateExportName { module_index: usize, export_index: usize },
    ForwarderTooLong { module_index: usize, function_index: usize },
    InvalidNameAssociation { module_index: usize, name_index: usize },
    InvalidOrdinalSequence { module_index: usize, function_index: usize },
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
        let error = ExportCatalogue::new(
            vec![module(), duplicate],
            ExportCatalogueLimits::default(),
        )
        .unwrap_err();
        assert_eq!(error, ExportCatalogueError::DuplicateModuleName { index: 1 });
    }

    #[test]
    fn rejects_inconsistent_name_associations() {
        let mut bad = module();
        bad.directory.names[0].function_index = 2;
        let error =
            ExportCatalogue::new(vec![bad], ExportCatalogueLimits::default()).unwrap_err();
        assert_eq!(
            error,
            ExportCatalogueError::InvalidNameAssociation {
                module_index: 0,
                name_index: 0
            }
        );
    }
}
