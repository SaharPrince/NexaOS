//! Validation of symbolic IAT patch plans.
//!
//! This module validates destinations in the importing image. Targets remain
//! module-relative RVAs: computing a process address requires an explicit
//! loaded-module base and is deliberately deferred to a later stage.

use crate::{ImportResolutionPlan, ParsedImportSymbol};
use std::fmt;

/// Caller-configured limits for validating an IAT patch plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IatPatchPlanLimits {
    pub max_patches: usize,
    pub max_module_name_bytes: usize,
    pub max_symbol_name_bytes: usize,
}

impl Default for IatPatchPlanLimits {
    fn default() -> Self {
        Self {
            max_patches: 65_536,
            max_module_name_bytes: 512,
            max_symbol_name_bytes: 512,
        }
    }
}

/// One validated, symbolic IAT patch destination.
///
/// The target is intentionally represented as a module name and RVA, not an
/// absolute address. No image bytes are changed by this module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IatPatch {
    pub import_library: String,
    pub import_symbol: ParsedImportSymbol,
    pub iat_rva: u32,
    pub pointer_width: u8,
    pub target_module: String,
    pub export_ordinal: u32,
    pub function_rva: u32,
    pub forwarder_hops: usize,
}

/// A complete set of validated IAT destinations, preserving resolver order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IatPatchPlan {
    pub patches: Vec<IatPatch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IatPatchPlanFailure {
    EmptyImage,
    TooManyPatches,
    AllocationFailed,
    EmptyModuleName,
    ModuleNameTooLong,
    EmptySymbolName,
    SymbolNameTooLong,
    UnsupportedPointerWidth { pointer_width: u8 },
    DestinationOverflow,
    DestinationOutOfBounds,
    OverlappingDestinations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IatPatchPlanError {
    /// None denotes a plan-wide failure rather than one malformed patch.
    pub patch_index: Option<usize>,
    pub failure: IatPatchPlanFailure,
}

impl fmt::Display for IatPatchPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "IAT patch-plan validation failed at {:?}: {:?}",
            self.patch_index, self.failure
        )
    }
}

impl std::error::Error for IatPatchPlanError {}

fn fail(patch_index: Option<usize>, failure: IatPatchPlanFailure) -> IatPatchPlanError {
    IatPatchPlanError {
        patch_index,
        failure,
    }
}

/// Validate every IAT destination against the importing image's mapped size.
///
/// This function independently checks a public/forged resolution plan rather
/// than trusting the resolver. It accepts only AMD64 eight-byte slots, checks
/// complete destination ranges, and rejects duplicate or partially overlapping
/// slots. It does not compute target virtual addresses: the target module's
/// loaded base is not available at this stage.
pub fn validate_iat_patch_plan(
    resolution: &ImportResolutionPlan,
    image_size: u64,
    limits: IatPatchPlanLimits,
) -> Result<IatPatchPlan, IatPatchPlanError> {
    if image_size == 0 {
        return Err(fail(None, IatPatchPlanFailure::EmptyImage));
    }
    if resolution.imports.len() > limits.max_patches {
        return Err(fail(None, IatPatchPlanFailure::TooManyPatches));
    }

    let mut ranges: Vec<(u64, u64, usize)> = Vec::new();
    ranges
        .try_reserve_exact(resolution.imports.len())
        .map_err(|_| fail(None, IatPatchPlanFailure::AllocationFailed))?;

    for (index, import) in resolution.imports.iter().enumerate() {
        if import.import_library.is_empty() || import.target_module.is_empty() {
            return Err(fail(Some(index), IatPatchPlanFailure::EmptyModuleName));
        }
        if import.import_library.len() > limits.max_module_name_bytes
            || import.target_module.len() > limits.max_module_name_bytes
        {
            return Err(fail(Some(index), IatPatchPlanFailure::ModuleNameTooLong));
        }
        if let ParsedImportSymbol::Name { name, .. } = &import.import_symbol {
            if name.is_empty() {
                return Err(fail(Some(index), IatPatchPlanFailure::EmptySymbolName));
            }
            if name.len() > limits.max_symbol_name_bytes {
                return Err(fail(Some(index), IatPatchPlanFailure::SymbolNameTooLong));
            }
        }
        if import.pointer_width != 8 {
            return Err(fail(
                Some(index),
                IatPatchPlanFailure::UnsupportedPointerWidth {
                    pointer_width: import.pointer_width,
                },
            ));
        }

        let start = u64::from(import.iat_rva);
        let end = start
            .checked_add(u64::from(import.pointer_width))
            .ok_or_else(|| fail(Some(index), IatPatchPlanFailure::DestinationOverflow))?;
        if end > image_size {
            return Err(fail(
                Some(index),
                IatPatchPlanFailure::DestinationOutOfBounds,
            ));
        }
        ranges.push((start, end, index));
    }

    ranges.sort_unstable_by_key(|(start, end, _)| (*start, *end));
    for pair in ranges.windows(2) {
        if pair[1].0 < pair[0].1 {
            return Err(fail(
                Some(pair[1].2),
                IatPatchPlanFailure::OverlappingDestinations,
            ));
        }
    }

    let mut patches = Vec::new();
    patches
        .try_reserve_exact(resolution.imports.len())
        .map_err(|_| fail(None, IatPatchPlanFailure::AllocationFailed))?;
    patches.extend(resolution.imports.iter().map(|import| IatPatch {
        import_library: import.import_library.clone(),
        import_symbol: import.import_symbol.clone(),
        iat_rva: import.iat_rva,
        pointer_width: import.pointer_width,
        target_module: import.target_module.clone(),
        export_ordinal: import.export_ordinal,
        function_rva: import.function_rva,
        forwarder_hops: import.forwarder_hops,
    }));

    Ok(IatPatchPlan { patches })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResolvedImport;

    fn patch(iat_rva: u32) -> ResolvedImport {
        ResolvedImport {
            import_library: "KERNEL32.dll".into(),
            import_symbol: ParsedImportSymbol::Name {
                name: "ExitProcess".into(),
                hint: 0,
            },
            iat_rva,
            pointer_width: 8,
            target_module: "KERNEL32.dll".into(),
            export_ordinal: 1,
            function_rva: 0x1234,
            forwarder_hops: 0,
        }
    }

    fn plan(imports: Vec<ResolvedImport>) -> ImportResolutionPlan {
        ImportResolutionPlan { imports }
    }

    #[test]
    fn validates_in_bounds_slots_and_preserves_resolver_order() {
        let resolution = plan(vec![patch(0x200), patch(0x100)]);
        let validated =
            validate_iat_patch_plan(&resolution, 0x208, IatPatchPlanLimits::default()).unwrap();

        assert_eq!(validated.patches.len(), 2);
        assert_eq!(validated.patches[0].iat_rva, 0x200);
        assert_eq!(validated.patches[1].iat_rva, 0x100);
        assert_eq!(validated.patches[0].function_rva, 0x1234);
    }

    #[test]
    fn accepts_slot_ending_exactly_at_image_boundary() {
        let validated = validate_iat_patch_plan(
            &plan(vec![patch(0x1f8)]),
            0x200,
            IatPatchPlanLimits::default(),
        )
        .unwrap();

        assert_eq!(validated.patches[0].iat_rva, 0x1f8);
    }

    #[test]
    fn rejects_slot_outside_image() {
        let error = validate_iat_patch_plan(
            &plan(vec![patch(0x1f9)]),
            0x200,
            IatPatchPlanLimits::default(),
        )
        .unwrap_err();

        assert_eq!(error.patch_index, Some(0));
        assert_eq!(error.failure, IatPatchPlanFailure::DestinationOutOfBounds);
    }

    #[test]
    fn rejects_duplicate_and_partially_overlapping_slots() {
        for second_rva in [0x100, 0x104] {
            let error = validate_iat_patch_plan(
                &plan(vec![patch(0x100), patch(second_rva)]),
                0x200,
                IatPatchPlanLimits::default(),
            )
            .unwrap_err();

            assert_eq!(error.patch_index, Some(1));
            assert_eq!(error.failure, IatPatchPlanFailure::OverlappingDestinations);
        }
    }

    #[test]
    fn rejects_non_amd64_pointer_width() {
        let mut invalid = patch(0x100);
        invalid.pointer_width = 4;
        let error =
            validate_iat_patch_plan(&plan(vec![invalid]), 0x200, IatPatchPlanLimits::default())
                .unwrap_err();

        assert_eq!(
            error.failure,
            IatPatchPlanFailure::UnsupportedPointerWidth { pointer_width: 4 }
        );
    }

    #[test]
    fn rejects_empty_or_oversized_names() {
        let mut empty = patch(0x100);
        empty.target_module.clear();
        assert_eq!(
            validate_iat_patch_plan(&plan(vec![empty]), 0x200, IatPatchPlanLimits::default())
                .unwrap_err()
                .failure,
            IatPatchPlanFailure::EmptyModuleName
        );

        let mut long = patch(0x100);
        long.import_symbol = ParsedImportSymbol::Name {
            name: "x".repeat(9),
            hint: 0,
        };
        let limits = IatPatchPlanLimits {
            max_symbol_name_bytes: 8,
            ..IatPatchPlanLimits::default()
        };
        assert_eq!(
            validate_iat_patch_plan(&plan(vec![long]), 0x200, limits)
                .unwrap_err()
                .failure,
            IatPatchPlanFailure::SymbolNameTooLong
        );
    }

    #[test]
    fn rejects_plan_over_limit_and_zero_sized_image() {
        let resolution = plan(vec![patch(0x100)]);
        let limits = IatPatchPlanLimits {
            max_patches: 0,
            ..IatPatchPlanLimits::default()
        };
        assert_eq!(
            validate_iat_patch_plan(&resolution, 0x200, limits)
                .unwrap_err()
                .failure,
            IatPatchPlanFailure::TooManyPatches
        );
        assert_eq!(
            validate_iat_patch_plan(&resolution, 0, IatPatchPlanLimits::default())
                .unwrap_err()
                .failure,
            IatPatchPlanFailure::EmptyImage
        );
    }

    #[test]
    fn allows_an_empty_import_plan_for_a_nonempty_image() {
        let validated =
            validate_iat_patch_plan(&plan(Vec::new()), 0x200, IatPatchPlanLimits::default())
                .unwrap();

        assert!(validated.patches.is_empty());
    }
}
