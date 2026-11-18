use crate::{
    allocate_image_buffer, apply_base_relocations, inspect, plan_base_relocations,
    plan_image_layout, populate_image_buffer, resolve_imports, validate_iat_patch_plan,
    ExportCatalogue, IatPatchPlan, IatPatchPlanLimits, ImageAllocationError, ImageAllocator,
    ImageBuffer, ImageLayoutError, ImageLayoutLimits, ImportResolutionLimits, RelocationApplyError,
    RelocationPlanLimits,
};
use std::fmt;

/// Identifies the preparation stage that rejected an input or intermediate result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePreparationStage {
    Inspect,
    Layout,
    Allocate,
    Populate,
    ResolveImports,
    ValidateImports,
    PlanRelocations,
    ApplyRelocations,
}

/// Failure from one bounded image-preparation stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePreparationError {
    pub stage: ImagePreparationStage,
    pub detail: String,
}

impl fmt::Display for ImagePreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(\n            f,\n            "image preparation failed during {:?}: {}",\n            self.stage, self.detail\n        )
    }
}

impl std::error::Error for ImagePreparationError {}

fn stage_error(stage: ImagePreparationStage, error: impl fmt::Display) -> ImagePreparationError {
    ImagePreparationError {
        stage,
        detail: error.to_string(),
    }
}

/// Explicit inputs and resource budgets for preparing one PE32+ AMD64 image.
///
/// The input bytes are borrowed and are parsed and populated from this same
/// immutable slice. The export catalogue is caller-owned and is never
/// populated by host DLL discovery.
pub struct ImagePreparationRequest<'a> {
    pub input: &'a [u8],
    pub requested_image_base: u64,
    pub layout_limits: ImageLayoutLimits,
    pub max_image_size: u64,
    pub export_catalogue: &'a ExportCatalogue,
    pub import_limits: ImportResolutionLimits,
    pub iat_limits: IatPatchPlanLimits,
    pub relocation_limits: RelocationPlanLimits,
}

/// Fully prepared, still non-executable image data.
///
/// The image buffer is private and can only be observed through a read-only
/// slice. Import targets remain symbolic module/RVA pairs; no IAT values or
/// process addresses are written or exposed.
#[derive(Debug)]
pub struct PreparedImage {
    image: ImageBuffer,
    pub image_size: u64,
    pub preferred_image_base: u64,
    pub requested_image_base: u64,
    pub iat_patch_plan: IatPatchPlan,
}

impl PreparedImage {
    pub fn image_bytes(&self) -> &[u8] {
        self.image.as_slice()
    }

    pub fn len(&self) -> usize {
        self.image.len()
    }

    pub fn is_empty(&self) -> bool {
        self.image.is_empty()
    }
}

/// Construct a bounded, non-executable PE32+ AMD64 image as one failure-atomic
/// operation. No partially populated image escapes if any stage fails.
pub fn prepare_image(
    request: ImagePreparationRequest<'_>,
    allocator: &mut impl ImageAllocator,
) -> Result<PreparedImage, ImagePreparationError> {
    let report = inspect(request.input)
        .map_err(|error| stage_error(ImagePreparationStage::Inspect, error))?;

    let layout = plan_image_layout(
        &report,
        u64::try_from(request.input.len()).unwrap_or(u64::MAX),
        request.layout_limits,
    )
    .map_err(|error: ImageLayoutError| stage_error(ImagePreparationStage::Layout, error))?;

    let image = allocate_image_buffer(&layout, request.max_image_size, allocator).map_err(
        |error: ImageAllocationError| stage_error(ImagePreparationStage::Allocate, error),
    )?;

    let mut image = populate_image_buffer(&layout, request.input, image).map_err(
        |error: ImageAllocationError| stage_error(ImagePreparationStage::Populate, error),
    )?;

    let imports = resolve_imports(
        &report.import_metadata,
        request.export_catalogue,
        request.import_limits,
    )
    .map_err(|error| stage_error(ImagePreparationStage::ResolveImports, error))?;

    let iat_patch_plan =
        validate_iat_patch_plan(&imports, layout.image_size, request.iat_limits)
            .map_err(|error| stage_error(ImagePreparationStage::ValidateImports, error))?;

    let relocation_plan = plan_base_relocations(
        &report.format,
        report.machine_raw,
        report.image_base,
        request.requested_image_base,
        layout.image_size,
        report.relocation_metadata.as_ref(),
        image.as_slice(),
        request.relocation_limits,
    )
    .map_err(|error| stage_error(ImagePreparationStage::PlanRelocations, error))?;

    apply_base_relocations(&mut image, &relocation_plan).map_err(
        |error: RelocationApplyError| stage_error(ImagePreparationStage::ApplyRelocations, error),
    )?;

    Ok(PreparedImage {
        image,
        image_size: layout.image_size,
        preferred_image_base: report.image_base,
        requested_image_base: request.requested_image_base,
        iat_patch_plan,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExportCatalogueLimits, SystemImageAllocator};

    fn pe64() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x400];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        let coff = 0x84;
        bytes[coff..coff + 2].copy_from_slice(&0x8664u16.to_le_bytes());
        bytes[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes());
        bytes[coff + 16..coff + 18].copy_from_slice(&0xf0u16.to_le_bytes());
        let optional = coff + 20;
        bytes[optional..optional + 2].copy_from_slice(&0x20bu16.to_le_bytes());
        bytes[optional + 16..optional + 20].copy_from_slice(&0x1000u32.to_le_bytes());
        bytes[optional + 24..optional + 32].copy_from_slice(&0x140000000u64.to_le_bytes());
        bytes[optional + 32..optional + 36].copy_from_slice(&0x1000u32.to_le_bytes());
        bytes[optional + 36..optional + 40].copy_from_slice(&0x200u32.to_le_bytes());
        bytes[optional + 56..optional + 60].copy_from_slice(&0x2000u32.to_le_bytes());
        bytes[optional + 60..optional + 64].copy_from_slice(&0x200u32.to_le_bytes());
        bytes[optional + 68..optional + 70].copy_from_slice(&3u16.to_le_bytes());
        bytes[optional + 108..optional + 112].copy_from_slice(&16u32.to_le_bytes());
        let section = optional + 0xf0;
        bytes[section..section + 5].copy_from_slice(b".text");
        bytes[section + 8..section + 12].copy_from_slice(&0x100u32.to_le_bytes());
        bytes[section + 12..section + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        bytes[section + 16..section + 20].copy_from_slice(&0x200u32.to_le_bytes());
        bytes[section + 20..section + 24].copy_from_slice(&0x200u32.to_le_bytes());
        bytes[0x300..0x400].fill(0xA5);\n        bytes\n    }\n\n    fn request<'a>(
        input: &'a [u8],
        catalogue: &'a ExportCatalogue,
        requested_image_base: u64,
    ) -> ImagePreparationRequest<'a> {
        ImagePreparationRequest {
            input,
            requested_image_base,
            layout_limits: ImageLayoutLimits {
                max_image_size: 1024 * 1024,
                max_sections: 96,
            },
            max_image_size: 1024 * 1024,
            export_catalogue: catalogue,
            import_limits: ImportResolutionLimits::default(),
            iat_limits: IatPatchPlanLimits::default(),
            relocation_limits: RelocationPlanLimits::default(),
        }
    }

    #[test]
    fn prepares_non_executable_image_without_imports_or_relocations() {
        let input = pe64();
        let catalogue = ExportCatalogue::new(Vec::new(), ExportCatalogueLimits::default()).unwrap();
        let mut allocator = SystemImageAllocator;

        let prepared =\n            prepare_image(request(&input, &catalogue, 0x140000000), &mut allocator).unwrap();

        assert_eq!(prepared.image_size, 0x2000);
        assert_eq!(prepared.preferred_image_base, 0x140000000);
        assert_eq!(prepared.requested_image_base, 0x140000000);
        assert!(prepared.iat_patch_plan.patches.is_empty());
        assert_eq!(&prepared.image_bytes()[..0x200], &input[..0x200]);
        assert_eq!(\n            &prepared.image_bytes()[0x1000..0x1100],\n            &input[0x200..0x300]\n        );\n        assert!(prepared.image_bytes()[0x1100..0x2000]\n            .iter()\n            .all(|byte| *byte == 0));
    }

    #[derive(Default)]
    struct CountingAllocator {
        calls: usize,
    }

    impl ImageAllocator for CountingAllocator {
        fn allocate(&mut self, size: usize) -> Result<Vec<u8>, ImageAllocationError> {
            self.calls += 1;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(size)
                .map_err(|_| ImageAllocationError::AllocationFailed)?;
            bytes.resize(size, 0);
            Ok(bytes)
        }
    }

    #[test]
    fn malformed_input_is_rejected_before_image_allocation() {
        let catalogue =
            ExportCatalogue::new(Vec::new(), ExportCatalogueLimits::default()).unwrap();
        let mut allocator = CountingAllocator::default();

        let error = prepare_image(
            request(b"not a PE file", &catalogue, 0x140000000),
            &mut allocator,
        )
        .unwrap_err();

        assert_eq!(error.stage, ImagePreparationStage::Inspect);
        assert_eq!(allocator.calls, 0);
    }

    #[test]
    fn rebase_without_relocation_data_fails_without_returning_an_image() {
        let input = pe64();
        let catalogue =
            ExportCatalogue::new(Vec::new(), ExportCatalogueLimits::default()).unwrap();
        let mut allocator = CountingAllocator::default();

        let error =\n            prepare_image(request(&input, &catalogue, 0x150000000), &mut allocator).unwrap_err();

        assert_eq!(error.stage, ImagePreparationStage::PlanRelocations);
        assert_eq!(allocator.calls, 1);
    }
}
