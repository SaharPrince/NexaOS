use crate::{ImageLayoutPlan, RelocationPlan};
use std::fmt;

/// Errors returned while creating a bounded, non-executable image buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageAllocationError {
    InvalidSize,
    ResourceLimit(&'static str),
    AllocationFailed,
    InvalidAllocatorOutput,
    InvalidPlan,
    InputOutOfBounds,
    OverlappingDestination,
}

impl fmt::Display for ImageAllocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize => write!(f, "image allocation size is invalid"),
            Self::ResourceLimit(message) => write!(f, "image allocation limit exceeded: {message}"),
            Self::AllocationFailed => write!(f, "image buffer allocation failed"),
            Self::InvalidAllocatorOutput => write!(f, "allocator returned an invalid image buffer"),
            Self::InvalidPlan => write!(f, "image layout plan is inconsistent or out of bounds"),
            Self::InputOutOfBounds => write!(f, "planned source range exceeds input file"),
            Self::OverlappingDestination => write!(f, "planned image destination ranges overlap"),
        }
    }
}

impl std::error::Error for ImageAllocationError {}

/// Allocation boundary used by the image-buffer stage and deterministic tests.
///
/// Implementations must return an owned, initialized vector. The image-buffer
/// constructor validates its length and clears it before exposing the result.
pub trait ImageAllocator {
    fn allocate(&mut self, size: usize) -> Result<Vec<u8>, ImageAllocationError>;
}

/// Default process-heap allocator. It requests capacity fallibly and then
/// initializes the requested bytes to zero; it never requests executable memory.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemImageAllocator;

impl ImageAllocator for SystemImageAllocator {
    fn allocate(&mut self, size: usize) -> Result<Vec<u8>, ImageAllocationError> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .map_err(|_| ImageAllocationError::AllocationFailed)?;
        bytes.resize(size, 0);
        Ok(bytes)
    }
}

/// Owned image storage. Bytes remain read-only to callers after population.
#[derive(Debug)]
pub struct ImageBuffer {
    bytes: Vec<u8>,
}

impl ImageBuffer {
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Failures while applying an image-relative base-relocation plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationApplyError {
    ImageSizeMismatch,
    TooManyPatches,
    EmptyPlanForRebase,
    TargetOutOfBounds { patch_index: usize },
    StalePlan { patch_index: usize },
    InvalidRelocatedValue { patch_index: usize },
    OverlappingTargets { patch_index: usize },
    AllocationFailed,
}

impl fmt::Display for RelocationApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImageSizeMismatch => {
                write!(f, "relocation plan image size does not match buffer")
            }
            Self::TooManyPatches => write!(f, "relocation patch count exceeds application limit"),
            Self::EmptyPlanForRebase => write!(f, "rebasing plan contains no relocation patches"),
            Self::TargetOutOfBounds { patch_index } => {
                write!(f, "relocation patch {patch_index} is outside the image")
            }
            Self::StalePlan { patch_index } => write!(
                f,
                "relocation patch {patch_index} no longer matches original image bytes"
            ),
            Self::InvalidRelocatedValue { patch_index } => write!(
                f,
                "relocation patch {patch_index} has an invalid relocated value"
            ),
            Self::OverlappingTargets { patch_index } => {
                write!(f, "relocation patch {patch_index} overlaps another target")
            }
            Self::AllocationFailed => write!(f, "relocation validation allocation failed"),
        }
    }
}

impl std::error::Error for RelocationApplyError {}

/// Apply a pure relocation plan to an image buffer as one transaction.
/// All fallible validation and allocation completes before the first write.
pub fn apply_base_relocations(
    image: &mut ImageBuffer,
    plan: &RelocationPlan,
) -> Result<(), RelocationApplyError> {
    const MAX_PATCHES: usize = 65_536;

    if u64::try_from(image.len()).ok() != Some(plan.image_size) {
        return Err(RelocationApplyError::ImageSizeMismatch);
    }
    if plan.patches.len() > MAX_PATCHES {
        return Err(RelocationApplyError::TooManyPatches);
    }
    if plan.preferred_image_base != plan.requested_image_base && plan.patches.is_empty() {
        return Err(RelocationApplyError::EmptyPlanForRebase);
    }

    let delta = plan
        .requested_image_base
        .abs_diff(plan.preferred_image_base);
    let add_delta = plan.requested_image_base > plan.preferred_image_base;
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    ranges
        .try_reserve_exact(plan.patches.len())
        .map_err(|_| RelocationApplyError::AllocationFailed)?;

    for (patch_index, patch) in plan.patches.iter().enumerate() {
        let start = usize::try_from(patch.target_rva)
            .map_err(|_| RelocationApplyError::TargetOutOfBounds { patch_index })?;
        let end = start
            .checked_add(8)
            .ok_or(RelocationApplyError::TargetOutOfBounds { patch_index })?;
        let current = image
            .bytes
            .get(start..end)
            .ok_or(RelocationApplyError::TargetOutOfBounds { patch_index })?;
        let current_value = u64::from_le_bytes(current.try_into().unwrap());
        if current_value != patch.original_value {
            return Err(RelocationApplyError::StalePlan { patch_index });
        }

        let expected_value = if add_delta {
            patch.original_value.checked_add(delta)
        } else {
            patch.original_value.checked_sub(delta)
        }
        .ok_or(RelocationApplyError::InvalidRelocatedValue { patch_index })?;
        if expected_value != patch.relocated_value {
            return Err(RelocationApplyError::InvalidRelocatedValue { patch_index });
        }
        ranges.push((start, patch_index));
    }

    ranges.sort_unstable_by_key(|range| range.0);
    for pair in ranges.windows(2) {
        if pair[1].0 < pair[0].0 + 8 {
            return Err(RelocationApplyError::OverlappingTargets {
                patch_index: pair[1].1,
            });
        }
    }

    // Bounds, original bytes, arithmetic, and overlap are fully validated.
    // This loop performs no allocation or fallible operation.
    for patch in &plan.patches {
        let start = patch.target_rva as usize;
        image.bytes[start..start + 8].copy_from_slice(&patch.relocated_value.to_le_bytes());
    }
    Ok(())
}

/// Allocate exactly the image size described by a previously validated plan.
///
/// The caller supplies the allocation ceiling; no product-wide default is
/// assumed. The buffer is non-executable and starts zero-initialized.
pub fn allocate_image_buffer(
    plan: &ImageLayoutPlan,
    max_image_size: u64,
    allocator: &mut impl ImageAllocator,
) -> Result<ImageBuffer, ImageAllocationError> {
    if plan.image_size == 0 {
        return Err(ImageAllocationError::InvalidSize);
    }
    if max_image_size == 0 || plan.image_size > max_image_size {
        return Err(ImageAllocationError::ResourceLimit(
            "planned image exceeds the caller-configured maximum",
        ));
    }
    let size = usize::try_from(plan.image_size)
        .map_err(|_| ImageAllocationError::ResourceLimit("image size does not fit this host"))?;

    let mut bytes = allocator.allocate(size)?;
    if bytes.len() != size {
        return Err(ImageAllocationError::InvalidAllocatorOutput);
    }
    // Enforce the zero-initialized contract even for custom allocators.
    bytes.fill(0);
    Ok(ImageBuffer { bytes })
}

/// Validate every planned source and destination range before changing the
/// buffer. The buffer is consumed so a failed population can never expose a
/// partially populated image as a successful result.
pub fn populate_image_buffer(
    plan: &ImageLayoutPlan,
    input: &[u8],
    mut image: ImageBuffer,
) -> Result<ImageBuffer, ImageAllocationError> {
    if plan.image_size == 0 || plan.image_size != image.len() as u64 {
        return Err(ImageAllocationError::InvalidPlan);
    }
    if plan.headers.file_offset != 0 || plan.headers.image_rva != 0 || plan.headers.length == 0 {
        return Err(ImageAllocationError::InvalidPlan);
    }

    // Each section contributes at most one copy and one zero-fill range;
    // headers contribute one additional range. Reserve before validation so
    // every subsequent push is allocation-free and allocation failure is
    // reported through the stage's typed error.
    let max_ranges = plan
        .sections
        .len()
        .checked_mul(2)
        .and_then(|count| count.checked_add(1))
        .ok_or(ImageAllocationError::InvalidPlan)?;
    let mut destinations: Vec<(usize, usize)> = Vec::new();
    destinations
        .try_reserve_exact(max_ranges)
        .map_err(|_| ImageAllocationError::AllocationFailed)?;
    let mut sources: Vec<(u64, u64)> = Vec::new();
    sources
        .try_reserve_exact(max_ranges)
        .map_err(|_| ImageAllocationError::AllocationFailed)?;
    let mut copy_total = 0u64;
    let mut zero_total = 0u64;

    let mut validate_range = |source: Option<u64>, destination: u64, length: u64| {
        let end = destination
            .checked_add(length)
            .ok_or(ImageAllocationError::InvalidPlan)?;
        if length == 0 || end > plan.image_size {
            return Err(ImageAllocationError::InvalidPlan);
        }
        let start = usize::try_from(destination).map_err(|_| ImageAllocationError::InvalidPlan)?;
        let end = usize::try_from(end).map_err(|_| ImageAllocationError::InvalidPlan)?;
        if let Some(source) = source {
            let source_end = source
                .checked_add(length)
                .ok_or(ImageAllocationError::InvalidPlan)?;
            if source_end > input.len() as u64 {
                return Err(ImageAllocationError::InputOutOfBounds);
            }
            if sources
                .iter()
                .any(|(other_start, other_end)| source < *other_end && *other_start < source_end)
            {
                return Err(ImageAllocationError::InvalidPlan);
            }
            sources.push((source, source_end));
            copy_total = copy_total
                .checked_add(length)
                .ok_or(ImageAllocationError::InvalidPlan)?;
        } else {
            zero_total = zero_total
                .checked_add(length)
                .ok_or(ImageAllocationError::InvalidPlan)?;
        }
        if destinations.iter().any(|(a, b)| start < *b && *a < end) {
            return Err(ImageAllocationError::OverlappingDestination);
        }
        destinations.push((start, end));
        Ok(())
    };

    validate_range(
        Some(plan.headers.file_offset),
        plan.headers.image_rva,
        plan.headers.length,
    )?;

    for section in &plan.sections {
        if section.mapped_size == 0 {
            return Err(ImageAllocationError::InvalidPlan);
        }
        let section_end = section
            .image_rva
            .checked_add(section.mapped_size)
            .ok_or(ImageAllocationError::InvalidPlan)?;
        if section_end > plan.image_size {
            return Err(ImageAllocationError::InvalidPlan);
        }

        let copy_length = section.copy.as_ref().map_or(0, |copy| copy.length);
        let zero_length = section.zero_fill.as_ref().map_or(0, |zero| zero.length);
        if copy_length.checked_add(zero_length) != Some(section.mapped_size) {
            return Err(ImageAllocationError::InvalidPlan);
        }

        if let Some(copy) = &section.copy {
            if copy.image_rva != section.image_rva {
                return Err(ImageAllocationError::InvalidPlan);
            }
            validate_range(Some(copy.file_offset), copy.image_rva, copy.length)?;
        }
        if let Some(zero) = &section.zero_fill {
            let expected_zero_rva = section
                .image_rva
                .checked_add(copy_length)
                .ok_or(ImageAllocationError::InvalidPlan)?;
            if zero.image_rva != expected_zero_rva {
                return Err(ImageAllocationError::InvalidPlan);
            }
            validate_range(None, zero.image_rva, zero.length)?;
        }
        if section.copy.is_none() && section.zero_fill.is_none() {
            return Err(ImageAllocationError::InvalidPlan);
        }
    }
    if copy_total != plan.total_copy_bytes || zero_total != plan.total_zero_fill_bytes {
        return Err(ImageAllocationError::InvalidPlan);
    }

    let header_len =
        usize::try_from(plan.headers.length).map_err(|_| ImageAllocationError::InvalidPlan)?;
    image.bytes[..header_len].copy_from_slice(&input[..header_len]);
    for section in &plan.sections {
        if let Some(copy) = &section.copy {
            let src =
                usize::try_from(copy.file_offset).map_err(|_| ImageAllocationError::InvalidPlan)?;
            let dst =
                usize::try_from(copy.image_rva).map_err(|_| ImageAllocationError::InvalidPlan)?;
            let len =
                usize::try_from(copy.length).map_err(|_| ImageAllocationError::InvalidPlan)?;
            image.bytes[dst..dst + len].copy_from_slice(&input[src..src + len]);
        }
        if let Some(zero) = &section.zero_fill {
            let dst =
                usize::try_from(zero.image_rva).map_err(|_| ImageAllocationError::InvalidPlan)?;
            let len =
                usize::try_from(zero.length).map_err(|_| ImageAllocationError::InvalidPlan)?;
            image.bytes[dst..dst + len].fill(0);
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ImageCopyRange, ImageLayoutPlan};

    #[derive(Default)]
    struct TestAllocator {
        calls: usize,
        fail: bool,
        wrong_length: bool,
    }

    impl ImageAllocator for TestAllocator {
        fn allocate(&mut self, size: usize) -> Result<Vec<u8>, ImageAllocationError> {
            self.calls += 1;
            if self.fail {
                return Err(ImageAllocationError::AllocationFailed);
            }
            if self.wrong_length {
                return Ok(vec![0; size.saturating_sub(1)]);
            }
            Ok(vec![0xa5; size])
        }
    }

    fn plan(size: u64) -> ImageLayoutPlan {
        ImageLayoutPlan {
            image_size: size,
            headers: ImageCopyRange {
                file_offset: 0,
                image_rva: 0,
                length: 0,
            },
            sections: Vec::new(),
            total_copy_bytes: 0,
            total_zero_fill_bytes: 0,
        }
    }

    #[test]
    fn populates_headers_sections_and_zero_fill() {
        use crate::{ImageCopyRange, ImageZeroFillRange, SectionImageLayout};
        let plan = ImageLayoutPlan {
            image_size: 0x1000,
            headers: ImageCopyRange {
                file_offset: 0,
                image_rva: 0,
                length: 4,
            },
            sections: vec![SectionImageLayout {
                name: ".text".into(),
                image_rva: 0x200,
                mapped_size: 8,
                copy: Some(ImageCopyRange {
                    file_offset: 4,
                    image_rva: 0x200,
                    length: 4,
                }),
                zero_fill: Some(ImageZeroFillRange {
                    image_rva: 0x204,
                    length: 4,
                }),
            }],
            total_copy_bytes: 8,
            total_zero_fill_bytes: 4,
        };
        let mut allocator = TestAllocator::default();
        let image = allocate_image_buffer(&plan, 0x1000, &mut allocator).unwrap();
        let image = populate_image_buffer(&plan, b"HEADDATA", image).unwrap();
        assert_eq!(&image.as_slice()[..4], b"HEAD");
        assert_eq!(&image.as_slice()[0x200..0x204], b"DATA");
        assert_eq!(&image.as_slice()[0x204..0x208], &[0; 4]);
        assert_eq!(image.as_slice()[0x208], 0);
    }

    #[test]
    fn does_not_copy_raw_alignment_padding_past_virtual_size() {
        use crate::{ImageCopyRange, SectionImageLayout};

        let plan = ImageLayoutPlan {
            image_size: 0x2000,
            headers: ImageCopyRange {
                file_offset: 0,
                image_rva: 0,
                length: 0x200,
            },
            sections: vec![SectionImageLayout {
                name: ".text".into(),
                image_rva: 0x1000,
                mapped_size: 0x100,
                copy: Some(ImageCopyRange {
                    file_offset: 0x200,
                    image_rva: 0x1000,
                    length: 0x100,
                }),
                zero_fill: None,
            }],
            total_copy_bytes: 0x300,
            total_zero_fill_bytes: 0,
        };
        let mut input = vec![0xa5; 0x600];
        input[..0x200].fill(0x11);
        let mut allocator = TestAllocator::default();
        let image = allocate_image_buffer(&plan, 0x2000, &mut allocator).unwrap();
        let image = populate_image_buffer(&plan, &input, image).unwrap();

        assert!(image.as_slice()[..0x200].iter().all(|byte| *byte == 0x11));
        assert!(image.as_slice()[0x1000..0x1100]
            .iter()
            .all(|byte| *byte == 0xa5));
        assert!(image.as_slice()[0x1100..0x1200]
            .iter()
            .all(|byte| *byte == 0));
    }

    #[test]
    fn rejects_out_of_bounds_source_without_returning_image() {
        let plan = ImageLayoutPlan {
            image_size: 16,
            headers: ImageCopyRange {
                file_offset: 0,
                image_rva: 0,
                length: 4,
            },
            sections: vec![],
            total_copy_bytes: 4,
            total_zero_fill_bytes: 0,
        };
        let mut allocator = TestAllocator::default();
        let image = allocate_image_buffer(&plan, 16, &mut allocator).unwrap();
        assert_eq!(
            populate_image_buffer(&plan, b"abc", image).unwrap_err(),
            ImageAllocationError::InputOutOfBounds
        );
    }

    #[test]
    fn rejects_overlapping_destinations_before_copying() {
        use crate::{ImageCopyRange, SectionImageLayout};
        let plan = ImageLayoutPlan {
            image_size: 32,
            headers: ImageCopyRange {
                file_offset: 0,
                image_rva: 0,
                length: 8,
            },
            sections: vec![SectionImageLayout {
                name: ".bad".into(),
                image_rva: 4,
                mapped_size: 4,
                copy: Some(ImageCopyRange {
                    file_offset: 8,
                    image_rva: 4,
                    length: 4,
                }),
                zero_fill: None,
            }],
            total_copy_bytes: 12,
            total_zero_fill_bytes: 0,
        };
        let mut allocator = TestAllocator::default();
        let image = allocate_image_buffer(&plan, 32, &mut allocator).unwrap();
        assert_eq!(
            populate_image_buffer(&plan, b"abcdefghijkl", image).unwrap_err(),
            ImageAllocationError::OverlappingDestination
        );
    }

    #[test]
    fn rejects_inconsistent_accounting() {
        let plan = ImageLayoutPlan {
            image_size: 16,
            headers: ImageCopyRange {
                file_offset: 0,
                image_rva: 0,
                length: 4,
            },
            sections: vec![],
            total_copy_bytes: 5,
            total_zero_fill_bytes: 0,
        };
        let mut allocator = TestAllocator::default();
        let image = allocate_image_buffer(&plan, 16, &mut allocator).unwrap();
        assert_eq!(
            populate_image_buffer(&plan, b"1234", image).unwrap_err(),
            ImageAllocationError::InvalidPlan
        );
    }

    #[test]
    fn allocates_exact_zeroed_non_executable_storage() {
        let mut allocator = TestAllocator::default();
        let image = allocate_image_buffer(&plan(32), 64, &mut allocator).unwrap();
        assert_eq!(image.len(), 32);
        assert!(!image.is_empty());
        assert!(image.as_slice().iter().all(|byte| *byte == 0));
        assert_eq!(allocator.calls, 1);
    }

    #[test]
    fn rejects_over_budget_before_calling_allocator() {
        let mut allocator = TestAllocator::default();
        assert!(matches!(
            allocate_image_buffer(&plan(65), 64, &mut allocator),
            Err(ImageAllocationError::ResourceLimit(_))
        ));
        assert_eq!(allocator.calls, 0);
    }

    #[test]
    fn rejects_zero_size_before_calling_allocator() {
        let mut allocator = TestAllocator::default();
        assert_eq!(
            allocate_image_buffer(&plan(0), 64, &mut allocator).unwrap_err(),
            ImageAllocationError::InvalidSize
        );
        assert_eq!(allocator.calls, 0);
    }

    #[test]
    fn reports_allocator_failure_without_returning_partial_image() {
        let mut allocator = TestAllocator {
            fail: true,
            ..TestAllocator::default()
        };
        assert_eq!(
            allocate_image_buffer(&plan(32), 64, &mut allocator).unwrap_err(),
            ImageAllocationError::AllocationFailed
        );
    }

    #[test]
    fn rejects_allocator_output_with_wrong_length() {
        let mut allocator = TestAllocator {
            wrong_length: true,
            ..TestAllocator::default()
        };
        assert_eq!(
            allocate_image_buffer(&plan(32), 64, &mut allocator).unwrap_err(),
            ImageAllocationError::InvalidAllocatorOutput
        );
    }

    #[test]
    fn rejects_zero_budget() {
        let mut allocator = TestAllocator::default();
        assert!(matches!(
            allocate_image_buffer(&plan(1), 0, &mut allocator),
            Err(ImageAllocationError::ResourceLimit(_))
        ));
        assert_eq!(allocator.calls, 0);
    }

    fn populated_section_plan() -> ImageLayoutPlan {
        use crate::{ImageCopyRange, ImageZeroFillRange, SectionImageLayout};
        ImageLayoutPlan {
            image_size: 64,
            headers: ImageCopyRange {
                file_offset: 0,
                image_rva: 0,
                length: 8,
            },
            sections: vec![SectionImageLayout {
                name: ".data".into(),
                image_rva: 16,
                mapped_size: 12,
                copy: Some(ImageCopyRange {
                    file_offset: 8,
                    image_rva: 16,
                    length: 8,
                }),
                zero_fill: Some(ImageZeroFillRange {
                    image_rva: 24,
                    length: 4,
                }),
            }],
            total_copy_bytes: 16,
            total_zero_fill_bytes: 4,
        }
    }

    fn assert_invalid_population_plan(plan: &ImageLayoutPlan) {
        let mut allocator = TestAllocator::default();
        let image = allocate_image_buffer(plan, 64, &mut allocator).unwrap();
        assert_eq!(
            populate_image_buffer(plan, b"0123456789abcdefghijklmnop", image).unwrap_err(),
            ImageAllocationError::InvalidPlan
        );
    }

    #[test]
    fn rejects_section_copy_not_matching_declared_image_rva() {
        let mut plan = populated_section_plan();
        plan.sections[0].copy.as_mut().unwrap().image_rva = 17;
        assert_invalid_population_plan(&plan);
    }

    #[test]
    fn rejects_section_ranges_that_do_not_cover_mapped_size() {
        let mut plan = populated_section_plan();
        plan.sections[0].zero_fill.as_mut().unwrap().length = 3;
        plan.total_zero_fill_bytes = 3;
        assert_invalid_population_plan(&plan);
    }

    #[test]
    fn rejects_zero_fill_not_immediately_following_copy() {
        let mut plan = populated_section_plan();
        plan.sections[0].zero_fill.as_mut().unwrap().image_rva = 25;
        assert_invalid_population_plan(&plan);
    }

    #[test]
    fn rejects_overlapping_source_ranges_in_forged_plan() {
        let mut plan = populated_section_plan();
        plan.sections[0].copy.as_mut().unwrap().file_offset = 4;
        assert_invalid_population_plan(&plan);
    }

    #[test]
    fn rejects_section_range_overflow_in_forged_plan() {
        let mut plan = populated_section_plan();
        plan.sections[0].image_rva = u64::MAX - 4;
        plan.sections[0].copy.as_mut().unwrap().image_rva = u64::MAX - 4;
        assert_invalid_population_plan(&plan);
    }

    fn relocation_test_buffer() -> ImageBuffer {
        let mut bytes = vec![0u8; 0x2000];
        bytes[0x1100..0x1108].copy_from_slice(&0x1400_1100u64.to_le_bytes());
        bytes[0x1200..0x1208].copy_from_slice(&0x1400_1200u64.to_le_bytes());
        ImageBuffer { bytes }
    }

    fn relocation_test_plan() -> RelocationPlan {
        RelocationPlan {
            preferred_image_base: 0x1400_0000,
            requested_image_base: 0x1500_0000,
            image_size: 0x2000,
            patches: vec![
                crate::RelocationPatch {
                    target_rva: 0x1100,
                    original_value: 0x1400_1100,
                    relocated_value: 0x1500_1100,
                },
                crate::RelocationPatch {
                    target_rva: 0x1200,
                    original_value: 0x1400_1200,
                    relocated_value: 0x1500_1200,
                },
            ],
        }
    }

    #[test]
    fn applies_valid_multi_patch_relocation_plan() {
        let mut image = relocation_test_buffer();
        apply_base_relocations(&mut image, &relocation_test_plan()).unwrap();
        assert_eq!(
            u64::from_le_bytes(image.as_slice()[0x1100..0x1108].try_into().unwrap()),
            0x1500_1100
        );
        assert_eq!(
            u64::from_le_bytes(image.as_slice()[0x1200..0x1208].try_into().unwrap()),
            0x1500_1200
        );
    }

    #[test]
    fn stale_relocation_plan_leaves_image_unchanged() {
        let mut image = relocation_test_buffer();
        image.bytes[0x1200] ^= 1;
        let before = image.as_slice().to_vec();
        assert_eq!(
            apply_base_relocations(&mut image, &relocation_test_plan()),
            Err(RelocationApplyError::StalePlan { patch_index: 1 })
        );
        assert_eq!(image.as_slice(), before);
    }

    #[test]
    fn forged_out_of_bounds_relocation_leaves_image_unchanged() {
        let mut image = relocation_test_buffer();
        let mut plan = relocation_test_plan();
        plan.patches[1].target_rva = 0x1ffc;
        let before = image.as_slice().to_vec();
        assert_eq!(
            apply_base_relocations(&mut image, &plan),
            Err(RelocationApplyError::TargetOutOfBounds { patch_index: 1 })
        );
        assert_eq!(image.as_slice(), before);
    }

    #[test]
    fn forged_duplicate_and_overlapping_relocations_leave_image_unchanged() {
        for second_target in [0x1100u32, 0x1104u32] {
            let mut image = relocation_test_buffer();
            let mut plan = relocation_test_plan();
            let start = second_target as usize;
            let original =
                u64::from_le_bytes(image.as_slice()[start..start + 8].try_into().unwrap());
            plan.patches[1] = crate::RelocationPatch {
                target_rva: second_target,
                original_value: original,
                relocated_value: original + 0x0100_0000,
            };
            let before = image.as_slice().to_vec();
            assert_eq!(
                apply_base_relocations(&mut image, &plan),
                Err(RelocationApplyError::OverlappingTargets { patch_index: 1 })
            );
            assert_eq!(image.as_slice(), before);
        }
    }

    #[test]
    fn forged_relocated_value_leaves_image_unchanged() {
        let mut image = relocation_test_buffer();
        let mut plan = relocation_test_plan();
        plan.patches[0].relocated_value ^= 1;
        let before = image.as_slice().to_vec();
        assert_eq!(
            apply_base_relocations(&mut image, &plan),
            Err(RelocationApplyError::InvalidRelocatedValue { patch_index: 0 })
        );
        assert_eq!(image.as_slice(), before);
    }

    #[test]
    fn rejects_section_without_copy_or_zero_fill() {
        let mut plan = populated_section_plan();
        plan.sections[0].copy = None;
        plan.sections[0].zero_fill = None;
        plan.sections[0].mapped_size = 0;
        assert_invalid_population_plan(&plan);
    }
}
