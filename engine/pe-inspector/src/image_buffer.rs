use crate::ImageLayoutPlan;
use std::fmt;

/// Errors returned while creating a bounded, non-executable image buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageAllocationError {
    InvalidSize,
    ResourceLimit(&'static str),
    AllocationFailed,
    InvalidAllocatorOutput,
}

impl fmt::Display for ImageAllocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize => write!(f, "image allocation size is invalid"),
            Self::ResourceLimit(message) => write!(f, "image allocation limit exceeded: {message}"),
            Self::AllocationFailed => write!(f, "image buffer allocation failed"),
            Self::InvalidAllocatorOutput => write!(f, "allocator returned an invalid image buffer"),
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

/// Owned image storage. Its bytes are exposed read-only in NCE-002b; population
/// is intentionally deferred to NCE-002c.
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

/// Allocate exactly the image size described by a previously validated plan.
///
/// The caller supplies the allocation ceiling; no product-wide default is
/// assumed. The buffer is not executable and is not populated from the PE file.
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
}
