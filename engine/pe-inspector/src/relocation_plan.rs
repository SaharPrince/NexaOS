//! Pure, bounded planning for PE32+ AMD64 DIR64 base relocations.
//!
//! Planning reads an already-populated, non-executable image and returns
//! image-relative patches. It never mutates image bytes or executes code.

use crate::ParsedRelocationDirectory;
use std::fmt;

const IMAGE_REL_BASED_ABSOLUTE: u8 = 0;
const IMAGE_REL_BASED_DIR64: u8 = 10;

/// Caller-configured limits for relocation planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelocationPlanLimits {
    pub max_blocks: usize,
    pub max_entries: usize,
    pub max_patches: usize,
}

impl Default for RelocationPlanLimits {
    fn default() -> Self {
        Self {
            max_blocks: 4096,
            max_entries: 65_536,
            max_patches: 65_536,
        }
    }
}

/// One precomputed eight-byte image-relative relocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelocationPatch {
    pub target_rva: u32,
    pub original_value: u64,
    pub relocated_value: u64,
}

/// Complete deterministic relocation plan. It contains no process addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationPlan {
    pub preferred_image_base: u64,
    pub requested_image_base: u64,
    pub image_size: u64,
    pub patches: Vec<RelocationPatch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationPlanFailure {
    UnsupportedFormat,
    UnsupportedMachine,
    EmptyImage,
    ImageSizeMismatch,
    MissingRelocationData,
    NoUsableRelocations,
    InvalidMetadata,
    TooManyBlocks,
    TooManyEntries,
    TooManyPatches,
    UnsupportedRelocationType { relocation_type: u8 },
    TargetRvaOverflow,
    TargetOutOfBounds,
    BaseDeltaOverflow,
    OverlappingTargets,
    AllocationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationPlanError {
    pub block_index: Option<usize>,
    pub entry_index: Option<usize>,
    pub failure: RelocationPlanFailure,
}

impl fmt::Display for RelocationPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "base-relocation planning failed at block {:?}, entry {:?}: {:?}",
            self.block_index, self.entry_index, self.failure
        )
    }
}

impl std::error::Error for RelocationPlanError {}

fn fail(
    block_index: Option<usize>,
    entry_index: Option<usize>,
    failure: RelocationPlanFailure,
) -> RelocationPlanError {
    RelocationPlanError {
        block_index,
        entry_index,
        failure,
    }
}

/// Build a pure PE32+ AMD64 relocation plan for an already-populated image.
///
/// The caller supplies the format, machine, preferred/requested bases, mapped
/// image size, parsed relocation metadata, and image bytes explicitly. Only
/// ABSOLUTE and DIR64 entries are accepted. No bytes are changed.
#[allow(clippy::too_many_arguments)]
pub fn plan_base_relocations(
    format: &str,
    machine: u16,
    preferred_image_base: u64,
    requested_image_base: u64,
    image_size: u64,
    relocation_metadata: Option<&ParsedRelocationDirectory>,
    image: &[u8],
    limits: RelocationPlanLimits,
) -> Result<RelocationPlan, RelocationPlanError> {
    if format != "PE32+" {
        return Err(fail(None, None, RelocationPlanFailure::UnsupportedFormat));
    }
    if machine != 0x8664 {
        return Err(fail(None, None, RelocationPlanFailure::UnsupportedMachine));
    }
    if image_size == 0 {
        return Err(fail(None, None, RelocationPlanFailure::EmptyImage));
    }
    if u64::try_from(image.len()).ok() != Some(image_size) {
        return Err(fail(None, None, RelocationPlanFailure::ImageSizeMismatch));
    }

    if preferred_image_base == requested_image_base {
        return Ok(RelocationPlan {
            preferred_image_base,
            requested_image_base,
            image_size,
            patches: Vec::new(),
        });
    }

    let metadata = relocation_metadata
        .ok_or_else(|| fail(None, None, RelocationPlanFailure::MissingRelocationData))?;
    if metadata.directory_rva == 0 || metadata.directory_size == 0 {
        return Err(fail(None, None, RelocationPlanFailure::InvalidMetadata));
    }
    let directory_end = u64::from(metadata.directory_rva)
        .checked_add(u64::from(metadata.directory_size))
        .ok_or_else(|| fail(None, None, RelocationPlanFailure::InvalidMetadata))?;
    if directory_end > image_size {
        return Err(fail(None, None, RelocationPlanFailure::InvalidMetadata));
    }
    if metadata.blocks.len() > limits.max_blocks {
        return Err(fail(None, None, RelocationPlanFailure::TooManyBlocks));
    }

    let mut total_entries = 0usize;
    let mut encoded_directory_size = 0u64;
    for (block_index, block) in metadata.blocks.iter().enumerate() {
        if !block.page_rva.is_multiple_of(0x1000)
            || u64::from(block.page_rva) >= image_size
        {
            return Err(fail(
                Some(block_index),
                None,
                RelocationPlanFailure::InvalidMetadata,
            ));
        }
        total_entries = total_entries
            .checked_add(block.entries.len())
            .ok_or_else(|| fail(None, None, RelocationPlanFailure::TooManyEntries))?;
        let block_bytes = u64::try_from(block.entries.len())
            .ok()
            .and_then(|count| count.checked_mul(2))
            .and_then(|bytes| bytes.checked_add(8))
            .ok_or_else(|| {
                fail(
                    Some(block_index),
                    None,
                    RelocationPlanFailure::InvalidMetadata,
                )
            })?;
        encoded_directory_size =
            encoded_directory_size
                .checked_add(block_bytes)
                .ok_or_else(|| {
                    fail(
                        Some(block_index),
                        None,
                        RelocationPlanFailure::InvalidMetadata,
                    )
                })?;
        for (entry_index, entry) in block.entries.iter().enumerate() {
            if entry.relocation_type != (entry.raw >> 12) as u8
                || entry.offset != (entry.raw & 0x0fff)
            {
                return Err(fail(
                    Some(block_index),
                    Some(entry_index),
                    RelocationPlanFailure::InvalidMetadata,
                ));
            }
        }
    }
    if total_entries > limits.max_entries {
        return Err(fail(None, None, RelocationPlanFailure::TooManyEntries));
    }
    if encoded_directory_size != u64::from(metadata.directory_size) {
        return Err(fail(None, None, RelocationPlanFailure::InvalidMetadata));
    }

    let delta = requested_image_base.abs_diff(preferred_image_base);
    let add_delta = requested_image_base > preferred_image_base;
    let mut patches = Vec::new();
    let mut target_ranges: Vec<(u64, usize, usize)> = Vec::new();
    patches
        .try_reserve_exact(total_entries.min(limits.max_patches))
        .map_err(|_| fail(None, None, RelocationPlanFailure::AllocationFailed))?;
    target_ranges
        .try_reserve_exact(total_entries.min(limits.max_patches))
        .map_err(|_| fail(None, None, RelocationPlanFailure::AllocationFailed))?;

    for (block_index, block) in metadata.blocks.iter().enumerate() {
        for (entry_index, entry) in block.entries.iter().enumerate() {
            match entry.relocation_type {
                IMAGE_REL_BASED_ABSOLUTE => continue,
                IMAGE_REL_BASED_DIR64 => {}
                relocation_type => {
                    return Err(fail(
                        Some(block_index),
                        Some(entry_index),
                        RelocationPlanFailure::UnsupportedRelocationType { relocation_type },
                    ));
                }
            }
            if patches.len() >= limits.max_patches {
                return Err(fail(None, None, RelocationPlanFailure::TooManyPatches));
            }
            let target_rva = block
                .page_rva
                .checked_add(u32::from(entry.offset))
                .ok_or_else(|| {
                    fail(
                        Some(block_index),
                        Some(entry_index),
                        RelocationPlanFailure::TargetRvaOverflow,
                    )
                })?;
            let target_end = u64::from(target_rva).checked_add(8).ok_or_else(|| {
                fail(
                    Some(block_index),
                    Some(entry_index),
                    RelocationPlanFailure::TargetRvaOverflow,
                )
            })?;
            if target_end > image_size {
                return Err(fail(
                    Some(block_index),
                    Some(entry_index),
                    RelocationPlanFailure::TargetOutOfBounds,
                ));
            }
            let start = usize::try_from(target_rva).map_err(|_| {
                fail(
                    Some(block_index),
                    Some(entry_index),
                    RelocationPlanFailure::TargetOutOfBounds,
                )
            })?;
            let end = start.checked_add(8).ok_or_else(|| {
                fail(
                    Some(block_index),
                    Some(entry_index),
                    RelocationPlanFailure::TargetOutOfBounds,
                )
            })?;
            let bytes = image.get(start..end).ok_or_else(|| {
                fail(
                    Some(block_index),
                    Some(entry_index),
                    RelocationPlanFailure::TargetOutOfBounds,
                )
            })?;
            let original_value = u64::from_le_bytes(bytes.try_into().unwrap());
            let relocated_value = if add_delta {
                original_value.checked_add(delta)
            } else {
                original_value.checked_sub(delta)
            }
            .ok_or_else(|| {
                fail(
                    Some(block_index),
                    Some(entry_index),
                    RelocationPlanFailure::BaseDeltaOverflow,
                )
            })?;
            patches.push(RelocationPatch {
                target_rva,
                original_value,
                relocated_value,
            });
            target_ranges.push((u64::from(target_rva), block_index, entry_index));
        }
    }

    if patches.is_empty() {
        return Err(fail(None, None, RelocationPlanFailure::NoUsableRelocations));
    }

    target_ranges.sort_unstable_by_key(|range| range.0);
    for pair in target_ranges.windows(2) {
        let previous_end = pair[0]
            .0
            .checked_add(8)
            .ok_or_else(|| fail(None, None, RelocationPlanFailure::TargetRvaOverflow))?;
        if pair[1].0 < previous_end {
            return Err(fail(
                Some(pair[1].1),
                Some(pair[1].2),
                RelocationPlanFailure::OverlappingTargets,
            ));
        }
    }

    Ok(RelocationPlan {
        preferred_image_base,
        requested_image_base,
        image_size,
        patches,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ParsedRelocationBlock, ParsedRelocationEntry};

    fn entry(raw: u16) -> ParsedRelocationEntry {
        ParsedRelocationEntry {
            raw,
            relocation_type: (raw >> 12) as u8,
            offset: raw & 0x0fff,
        }
    }

    fn metadata(entries: Vec<ParsedRelocationEntry>) -> ParsedRelocationDirectory {
        ParsedRelocationDirectory {
            directory_rva: 0x800,
            directory_size: u32::try_from(8 + entries.len() * 2).unwrap(),
            blocks: vec![ParsedRelocationBlock {
                page_rva: 0x1000,
                entries,
            }],
        }
    }

    fn image_with_value(rva: usize, value: u64) -> Vec<u8> {
        let mut image = vec![0; 0x2000];
        image[rva..rva + 8].copy_from_slice(&value.to_le_bytes());
        image
    }

    fn plan(
        metadata: Option<&ParsedRelocationDirectory>,
        image: &[u8],
        preferred: u64,
        requested: u64,
    ) -> Result<RelocationPlan, RelocationPlanError> {
        plan_base_relocations(
            "PE32+",
            0x8664,
            preferred,
            requested,
            image.len() as u64,
            metadata,
            image,
            RelocationPlanLimits::default(),
        )
    }

    #[test]
    fn equal_bases_need_no_relocation_metadata() {
        let image = vec![0; 0x2000];
        let result = plan(None, &image, 0x1400_0000, 0x1400_0000).unwrap();
        assert!(result.patches.is_empty());
    }

    #[test]
    fn plans_positive_dir64_delta() {
        let image = image_with_value(0x1123, 0x1400_1234);
        let metadata = metadata(vec![entry(0xa123)]);
        let result = plan(Some(&metadata), &image, 0x1400_0000, 0x1500_0000).unwrap();
        assert_eq!(
            result.patches,
            vec![RelocationPatch {
                target_rva: 0x1123,
                original_value: 0x1400_1234,
                relocated_value: 0x1500_1234,
            }]
        );
    }

    #[test]
    fn plans_negative_dir64_delta() {
        let image = image_with_value(0x1123, 0x1500_1234);
        let metadata = metadata(vec![entry(0xa123)]);
        let result = plan(Some(&metadata), &image, 0x1500_0000, 0x1400_0000).unwrap();
        assert_eq!(result.patches[0].relocated_value, 0x1400_1234);
    }

    #[test]
    fn absolute_entries_are_noops() {
        let metadata = metadata(vec![entry(0), entry(0xa123)]);
        let image = image_with_value(0x1123, 0x1400_1234);
        let result = plan(Some(&metadata), &image, 0x1400_0000, 0x1500_0000).unwrap();
        assert_eq!(result.patches.len(), 1);
        assert_eq!(result.patches[0].target_rva, 0x1123);
    }

    #[test]
    fn rejects_unsupported_types_including_highlow() {
        let image = vec![0; 0x2000];
        for raw in [0x3123, 0xf123] {
            let metadata = metadata(vec![entry(raw)]);
            let error = plan(Some(&metadata), &image, 0x1400_0000, 0x1500_0000).unwrap_err();
            assert_eq!(
                error.failure,
                RelocationPlanFailure::UnsupportedRelocationType {
                    relocation_type: (raw >> 12) as u8
                }
            );
        }
    }

    #[test]
    fn rejects_an_absolute_only_directory_when_rebasing() {
        let image = vec![0; 0x2000];
        let metadata = metadata(vec![entry(0)]);
        assert_eq!(
            plan(Some(&metadata), &image, 0x1400_0000, 0x1500_0000)
                .unwrap_err()
                .failure,
            RelocationPlanFailure::NoUsableRelocations
        );
    }

    #[test]
    fn rejects_missing_metadata_when_rebasing() {
        let image = vec![0; 0x2000];
        assert_eq!(
            plan(None, &image, 0x1400_0000, 0x1500_0000)
                .unwrap_err()
                .failure,
            RelocationPlanFailure::MissingRelocationData
        );
    }

    #[test]
    fn rejects_targets_outside_image() {
        let image = vec![0; 0x2000];
        let metadata = metadata(vec![entry(0xafff)]);
        assert_eq!(
            plan(Some(&metadata), &image, 0x1400_0000, 0x1500_0000)
                .unwrap_err()
                .failure,
            RelocationPlanFailure::TargetOutOfBounds
        );
    }

    #[test]
    fn rejects_duplicate_and_overlapping_targets() {
        let image = image_with_value(0x1123, 1);
        let metadata = ParsedRelocationDirectory {
            directory_rva: 0x800,
            directory_size: 12,
            blocks: vec![ParsedRelocationBlock {
                page_rva: 0x1000,
                entries: vec![entry(0xa123), entry(0xa124)],
            }],
        };
        assert_eq!(
            plan(Some(&metadata), &image, 0x1400_0000, 0x1500_0000).unwrap_err().failure,
            RelocationPlanFailure::OverlappingTargets
        );
    }

    #[test]
    fn rejects_base_delta_overflow_and_underflow() {
        let metadata = metadata(vec![entry(0xa123)]);
        let overflow_image = image_with_value(0x1123, u64::MAX);
        assert_eq!(
            plan(Some(&metadata), &overflow_image, 1, 2)
                .unwrap_err()
                .failure,
            RelocationPlanFailure::BaseDeltaOverflow
        );
        let underflow_image = image_with_value(0x1123, 0);
        assert_eq!(
            plan(Some(&metadata), &underflow_image, 2, 1)
                .unwrap_err()
                .failure,
            RelocationPlanFailure::BaseDeltaOverflow
        );
    }

    #[test]
    fn rejects_forged_metadata_and_resource_limits() {
        let image = vec![0; 0x2000];
        let mut forged = metadata(vec![entry(0xa123)]);
        forged.blocks[0].entries[0].offset = 0x124;
        assert_eq!(
            plan(Some(&forged), &image, 0x1400_0000, 0x1500_0000)
                .unwrap_err()
                .failure,
            RelocationPlanFailure::InvalidMetadata
        );

        let valid = metadata(vec![entry(0xa123)]);
        let error = plan_base_relocations(
            "PE32+",
            0x8664,
            0x1400_0000,
            0x1500_0000,
            image.len() as u64,
            Some(&valid),
            &image,
            RelocationPlanLimits {
                max_blocks: 0,
                max_entries: 1,
                max_patches: 1,
            },
        )
        .unwrap_err();
        assert_eq!(error.failure, RelocationPlanFailure::TooManyBlocks);
    }

    #[test]
    fn rejects_invalid_image_identity() {
        let image = vec![0; 0x2000];
        assert_eq!(
            plan_base_relocations(
                "PE32",
                0x8664,
                1,
                2,
                image.len() as u64,
                None,
                &image,
                RelocationPlanLimits::default(),
            )
            .unwrap_err()
            .failure,
            RelocationPlanFailure::UnsupportedFormat
        );
        assert_eq!(
            plan_base_relocations(
                "PE32+",
                0x014c,
                1,
                2,
                image.len() as u64,
                None,
                &image,
                RelocationPlanLimits::default(),
            )
            .unwrap_err()
            .failure,
            RelocationPlanFailure::UnsupportedMachine
        );
    }
}
