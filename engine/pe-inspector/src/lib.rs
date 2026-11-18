use serde::Serialize;
use std::fmt;

mod iat_patch;
mod image_buffer;
mod image_layout;
mod relocation_metadata;
mod relocation_plan;
mod resolver;
mod typed_metadata;
pub use iat_patch::{
    validate_iat_patch_plan, IatPatch, IatPatchPlan, IatPatchPlanError, IatPatchPlanFailure,
    IatPatchPlanLimits,
};
pub use image_buffer::{
    allocate_image_buffer, apply_base_relocations, populate_image_buffer, ImageAllocationError,
    ImageAllocator, ImageBuffer, RelocationApplyError, SystemImageAllocator,
};
pub use image_layout::{
    plan_image_layout, ImageCopyRange, ImageLayoutError, ImageLayoutLimits, ImageLayoutPlan,
    ImageZeroFillRange, SectionImageLayout,
};
pub use relocation_metadata::{
    ParsedRelocationBlock, ParsedRelocationDirectory, ParsedRelocationEntry,
};
pub use relocation_plan::{
    plan_base_relocations, RelocationPatch, RelocationPlan, RelocationPlanError,
    RelocationPlanFailure, RelocationPlanLimits,
};
pub use resolver::{
    resolve_imports, ExportCatalogue, ExportCatalogueError, ExportCatalogueLimits, ExportLookup,
    ExportLookupResult, ExportModule, ImportResolutionError, ImportResolutionFailure,
    ImportResolutionLimits, ImportResolutionPlan, ResolvedImport,
};
pub use typed_metadata::{