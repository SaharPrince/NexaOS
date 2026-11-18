//! Exercises image preparation through the crate's public API, as an
//! external consumer would. The fixture is inspected and copied only; it is
//! never executed.
use nexa_inspect::{
    prepare_image, ExportCatalogue, ImageAllocator, ImageAllocationError, ImageLayoutLimits,
    ImagePreparationRequest, ImportResolutionLimits, IatPatchPlanLimits, RelocationPlanLimits,
};

fn minimal_pe64() -> Vec<u8> {
    let mut bytes = vec![0u8; 0x400];
    bytes[0..2].copy_from_slice(b"MZ");
    bytes[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    bytes[0x80..0x84].copy_from_slice(b"PE\\0\\0");

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
    bytes[0x200..0x400].fill(0xa5);
    bytes
}

#[test]
fn prepared_image_is_observable_through_read_only_public_accessors() {
    let input = minimal_pe64();
    // Type inference keeps this test independent of the limits type's name.
    let catalogue = ExportCatalogue::new(Vec::new(), Default::default()).unwrap();
    let request = ImagePreparationRequest {
        input: &input,
        requested_image_base: 0x140000000,
        layout_limits: ImageLayoutLimits {
            max_image_size: 1024 * 1024,
            max_sections: 96,
        },
        max_image_size: 1024 * 1024,
        export_catalogue: &catalogue,
        import_limits: ImportResolutionLimits::default(),
        iat_limits: IatPatchPlanLimits::default(),
        relocation_limits: RelocationPlanLimits::default(),
    };
    let mut allocator = ExternalAllocator { calls: 0 };

    let prepared = prepare_image(request, &mut allocator).unwrap();
    assert_eq!(allocator.calls, 1);

    assert_eq!(prepared.image_size(), 0x2000);
    assert_eq!(prepared.preferred_image_base(), 0x140000000);
    assert_eq!(prepared.requested_image_base(), 0x140000000);
    assert_eq!(prepared.len(), 0x2000);
    assert!(!prepared.is_empty());

    let image = prepared.image_bytes();
    assert_eq!(&image[..0x200], &input[..0x200]);
    assert_eq!(&image[0x1000..0x1100], &input[0x200..0x300]);
    assert!(image[0x1100..0x2000].iter().all(|byte| *byte == 0));

    // The public API exposes the validated plan by shared reference only.
    assert!(prepared.iat_patch_plan().patches.is_empty());
}

// Keep the allocator trait in scope as a compile-time check that consumers
// can also provide their own allocator implementation.
#[allow(dead_code)]
struct ExternalAllocator {
    calls: usize,
}

impl ImageAllocator for ExternalAllocator {
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
