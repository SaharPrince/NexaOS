use crate::PeReport;
use std::fmt;

/// Caller-supplied image budget. Product defaults are deferred until low-end
/// hardware measurements establish an appropriate resource ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageLayoutLimits {
    pub max_image_size: u64,
    /// Maximum number of sections the planner may process and retain.
    pub max_sections: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageCopyRange {
    pub file_offset: u64,
    pub image_rva: u64,
    pub length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageZeroFillRange {
    pub image_rva: u64,
    pub length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionImageLayout {
    pub name: String,
    pub image_rva: u64,
    pub mapped_size: u64,
    pub copy: Option<ImageCopyRange>,
    pub zero_fill: Option<ImageZeroFillRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageLayoutPlan {
    pub image_size: u64,
    pub headers: ImageCopyRange,
    pub sections: Vec<SectionImageLayout>,
    pub total_copy_bytes: u64,
    pub total_zero_fill_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageLayoutError {
    Unsupported(&'static str),
    Invalid(&'static str),
    OutOfBounds(&'static str),
    ResourceLimit(&'static str),
}

impl fmt::Display for ImageLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(message) => write!(f, "unsupported image: {message}"),
            Self::Invalid(message) => write!(f, "invalid image layout: {message}"),
            Self::OutOfBounds(message) => write!(f, "image range out of bounds: {message}"),
            Self::ResourceLimit(message) => write!(f, "image resource limit exceeded: {message}"),
        }
    }
}

impl std::error::Error for ImageLayoutError {}

fn checked_end(start: u64, length: u64, what: &'static str) -> Result<u64, ImageLayoutError> {
    start
        .checked_add(length)
        .ok_or(ImageLayoutError::OutOfBounds(what))
}

fn overlaps(a_start: u64, a_end: u64, b_start: u64, b_end: u64) -> bool {
    a_start < b_end && b_start < a_end
}

/// Produce a pure PE32+ AMD64 image-layout plan. This performs no image
/// allocation and writes no bytes. file_len must describe the exact input
/// whose bytes were used to create report.
pub fn plan_image_layout(
    report: &PeReport,
    file_len: u64,
    limits: ImageLayoutLimits,
) -> Result<ImageLayoutPlan, ImageLayoutError> {
    if report.format != "PE32+" || report.machine_raw != 0x8664 {
        return Err(ImageLayoutError::Unsupported(
            "the initial loader planner supports PE32+ AMD64 images only",
        ));
    }
    if limits.max_image_size == 0 {
        return Err(ImageLayoutError::Invalid(
            "maximum image size must be nonzero",
        ));
    }
    if report.sections.len() > limits.max_sections {
        return Err(ImageLayoutError::ResourceLimit(
            "section count exceeds the configured maximum",
        ));
    }

    let image_size = report.size_of_image as u64;
    let headers_size = report.size_of_headers as u64;
    let section_alignment = report.section_alignment as u64;
    let file_alignment = report.file_alignment as u64;

    if image_size == 0 || image_size > limits.max_image_size {
        return Err(ImageLayoutError::ResourceLimit(
            "SizeOfImage is zero or exceeds the configured maximum",
        ));
    }
    if headers_size == 0 || headers_size > image_size || headers_size > file_len {
        return Err(ImageLayoutError::OutOfBounds(
            "SizeOfHeaders must fit in both the file and image",
        ));
    }
    if section_alignment == 0 || !section_alignment.is_power_of_two() {
        return Err(ImageLayoutError::Invalid(
            "SectionAlignment must be a nonzero power of two",
        ));
    }
    if file_alignment == 0 || !file_alignment.is_power_of_two() {
        return Err(ImageLayoutError::Invalid(
            "FileAlignment must be a nonzero power of two",
        ));
    }
    if section_alignment < file_alignment {
        return Err(ImageLayoutError::Invalid(
            "SectionAlignment is smaller than FileAlignment",
        ));
    }
    if section_alignment < 0x1000 {
        if file_alignment != section_alignment {
            return Err(ImageLayoutError::Invalid(
                "low SectionAlignment requires matching FileAlignment",
            ));
        }
    } else if !(0x200..=0x1_0000).contains(&file_alignment) {
        return Err(ImageLayoutError::Invalid(
            "FileAlignment must be between 512 bytes and 64 KiB",
        ));
    }
    if !image_size.is_multiple_of(section_alignment) {
        return Err(ImageLayoutError::Invalid(
            "SizeOfImage is not a multiple of SectionAlignment",
        ));
    }
    if !headers_size.is_multiple_of(file_alignment) {
        return Err(ImageLayoutError::Invalid(
            "SizeOfHeaders is not a multiple of FileAlignment",
        ));
    }
    if report.entry_point_rva != 0 && report.entry_point_rva as u64 >= image_size {
        return Err(ImageLayoutError::OutOfBounds(
            "entry point RVA is outside SizeOfImage",
        ));
    }

    let headers = ImageCopyRange {
        file_offset: 0,
        image_rva: 0,
        length: headers_size,
    };
    let mut layouts = Vec::with_capacity(report.sections.len());
    let mut total_copy_bytes = headers_size;
    let mut total_zero_fill_bytes = 0u64;
    let mut ranges = Vec::with_capacity(report.sections.len());
    let mut source_ranges = Vec::with_capacity(report.sections.len());

    for section in &report.sections {
        let raw_size = section.raw_size as u64;
        let virtual_size = section.virtual_size as u64;
        // VirtualSize is the logical in-memory extent. SizeOfRawData is
        // file-aligned and can include trailing padding beyond that extent.
        // Some images use a zero VirtualSize; retain the raw-size fallback
        // for that ambiguous legacy case rather than silently dropping data.
        let mapped_size = if virtual_size == 0 {
            raw_size
        } else {
            virtual_size
        };
        if mapped_size == 0 {
            continue;
        }
        let copy_size = raw_size.min(mapped_size);

        let image_rva = section.virtual_address as u64;
        if !image_rva.is_multiple_of(section_alignment) {
            return Err(ImageLayoutError::Invalid(
                "section VirtualAddress is not aligned to SectionAlignment",
            ));
        }
        let image_end = checked_end(image_rva, mapped_size, "section image range")?;
        if image_end > image_size {
            return Err(ImageLayoutError::OutOfBounds(
                "section image range exceeds SizeOfImage",
            ));
        }
        if overlaps(image_rva, image_end, 0, headers_size) {
            return Err(ImageLayoutError::Invalid(
                "section image range overlaps image headers",
            ));
        }
        for &(other_start, other_end) in &ranges {
            if overlaps(image_rva, image_end, other_start, other_end) {
                return Err(ImageLayoutError::Invalid("section image ranges overlap"));
            }
        }
        ranges.push((image_rva, image_end));

        let copy = if copy_size == 0 {
            None
        } else {
            let file_offset = section.raw_offset as u64;
            if file_offset < headers_size {
                return Err(ImageLayoutError::Invalid(
                    "section raw data overlaps image headers",
                ));
            }
            if !file_offset.is_multiple_of(file_alignment)
                || !raw_size.is_multiple_of(file_alignment)
            {
                return Err(ImageLayoutError::Invalid(
                    "section raw range is not aligned to FileAlignment",
                ));
            }
            let file_end = checked_end(file_offset, raw_size, "section raw file range")?;
            if file_end > file_len {
                return Err(ImageLayoutError::OutOfBounds(
                    "section raw file range exceeds input file",
                ));
            }
            for &(other_start, other_end) in &source_ranges {
                if overlaps(file_offset, file_end, other_start, other_end) {
                    return Err(ImageLayoutError::Invalid("section raw file ranges overlap"));
                }
            }
            source_ranges.push((file_offset, file_end));
            total_copy_bytes = total_copy_bytes
                .checked_add(copy_size)
                .ok_or(ImageLayoutError::OutOfBounds("total copy byte count"))?;
            Some(ImageCopyRange {
                file_offset,
                image_rva,
                length: copy_size,
            })
        };

        let zero_fill_size = mapped_size - copy_size;
        let zero_fill = if zero_fill_size == 0 {
            None
        } else {
            total_zero_fill_bytes = total_zero_fill_bytes
                .checked_add(zero_fill_size)
                .ok_or(ImageLayoutError::OutOfBounds("total zero-fill byte count"))?;
            Some(ImageZeroFillRange {
                image_rva: image_rva + raw_size,
                length: zero_fill_size,
            })
        };

        layouts.push(SectionImageLayout {
            name: section.name.clone(),
            image_rva,
            mapped_size,
            copy,
            zero_fill,
        });
    }

    Ok(ImageLayoutPlan {
        image_size,
        headers,
        sections: layouts,
        total_copy_bytes,
        total_zero_fill_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataDirectory, ImportLibrary, Section};

    fn report() -> PeReport {
        PeReport {
            format: "PE32+".into(),
            execution: "NOT_ATTEMPTED",
            machine: "AMD64".into(),
            machine_raw: 0x8664,
            sections: vec![Section {
                name: ".text".into(),
                virtual_size: 0x300,
                virtual_address: 0x1000,
                raw_size: 0x200,
                raw_offset: 0x200,
                characteristics: 0x6000_0020,
            }],
            image_base: 0x1400_0000,
            size_of_image: 0x2000,
            size_of_headers: 0x200,
            section_alignment: 0x1000,
            file_alignment: 0x200,
            entry_point_rva: 0x1000,
            subsystem: "Windows Console".into(),
            directories: Vec::<DataDirectory>::new(),
            imports: Vec::<ImportLibrary>::new(),
            exports: Vec::new(),
            relocation_blocks: 0,
            relocation_entries: 0,
        }
    }

    fn limits() -> ImageLayoutLimits {
        ImageLayoutLimits {
            max_image_size: 16 * 1024 * 1024,
            max_sections: 96,
        }
    }

    #[test]
    fn plans_headers_section_copy_and_zero_fill() {
        let plan = plan_image_layout(&report(), 0x400, limits()).unwrap();
        assert_eq!(plan.image_size, 0x2000);
        assert_eq!(plan.headers.length, 0x200);
        assert_eq!(plan.sections.len(), 1);
        assert_eq!(plan.sections[0].copy.as_ref().unwrap().file_offset, 0x200);
        assert_eq!(plan.sections[0].copy.as_ref().unwrap().length, 0x200);
        assert_eq!(
            plan.sections[0].zero_fill,
            Some(ImageZeroFillRange {
                image_rva: 0x1200,
                length: 0x100,
            })
        );
        assert_eq!(plan.total_copy_bytes, 0x400);
        assert_eq!(plan.total_zero_fill_bytes, 0x100);
    }

    #[test]
    fn excludes_file_alignment_padding_when_raw_size_exceeds_virtual_size() {
        let mut input = report();
        input.sections[0].virtual_size = 0x100;
        input.sections[0].raw_size = 0x400;
        let plan = plan_image_layout(&input, 0x600, limits()).unwrap();
        assert_eq!(plan.sections[0].mapped_size, 0x100);
        assert_eq!(plan.sections[0].copy.as_ref().unwrap().length, 0x100);
        assert_eq!(plan.sections[0].zero_fill, None);
        assert_eq!(plan.total_copy_bytes, 0x300);
        assert_eq!(plan.total_zero_fill_bytes, 0);
    }

    #[test]
    fn uses_raw_size_when_virtual_size_is_zero() {
        let mut input = report();
        input.sections[0].virtual_size = 0;
        input.sections[0].raw_size = 0x400;
        let plan = plan_image_layout(&input, 0x600, limits()).unwrap();
        assert_eq!(plan.sections[0].mapped_size, 0x400);
        assert_eq!(plan.sections[0].copy.as_ref().unwrap().length, 0x400);
        assert_eq!(plan.sections[0].zero_fill, None);
    }

    #[test]
    fn validates_full_raw_range_even_when_padding_is_not_copied() {
        let mut input = report();
        input.sections[0].virtual_size = 0x100;
        input.sections[0].raw_size = 0x400;
        assert!(matches!(
            plan_image_layout(&input, 0x5ff, limits()),
            Err(ImageLayoutError::OutOfBounds(_))
        ));
    }

    #[test]
    fn rejects_raw_range_overlap_in_alignment_padding() {
        let mut input = report();
        input.sections[0].virtual_size = 0x100;
        input.sections[0].raw_size = 0x400;

        let mut second = input.sections[0].clone();
        second.name = ".data".into();
        second.virtual_address = 0x2000;
        second.raw_offset = 0x400;
        second.raw_size = 0x200;
        second.virtual_size = 0x100;
        input.size_of_image = 0x3000;
        input.sections.push(second);

        assert!(matches!(
            plan_image_layout(&input, 0x800, limits()),
            Err(ImageLayoutError::Invalid("section raw file ranges overlap"))
        ));
    }

    #[test]
    fn rejects_non_amd64_or_pe32_input() {
        let mut input = report();
        input.format = "PE32".into();
        assert!(matches!(
            plan_image_layout(&input, 0x400, limits()),
            Err(ImageLayoutError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_image_over_configured_limit_before_planning() {
        let mut input = report();
        input.size_of_image = 0x4000;
        assert!(matches!(
            plan_image_layout(
                &input,
                0x400,
                ImageLayoutLimits {
                    max_image_size: 0x2000,
                    max_sections: 96,
                }
            ),
            Err(ImageLayoutError::ResourceLimit(_))
        ));
    }

    #[test]
    fn rejects_section_count_over_configured_limit_before_planning() {
        let mut input = report();
        input.sections.push(input.sections[0].clone());
        assert!(matches!(
            plan_image_layout(
                &input,
                0x400,
                ImageLayoutLimits {
                    max_image_size: 0x4000,
                    max_sections: 1,
                }
            ),
            Err(ImageLayoutError::ResourceLimit(
                "section count exceeds the configured maximum"
            ))
        ));
    }

    #[test]
    fn rejects_section_destination_outside_image() {
        let mut input = report();
        input.size_of_image = 0x1000;
        assert!(matches!(
            plan_image_layout(&input, 0x400, limits()),
            Err(ImageLayoutError::OutOfBounds(_))
        ));
    }

    #[test]
    fn rejects_truncated_section_source_range() {
        assert!(matches!(
            plan_image_layout(&report(), 0x3ff, limits()),
            Err(ImageLayoutError::OutOfBounds(_))
        ));
    }

    #[test]
    fn rejects_invalid_alignment() {
        let mut input = report();
        input.file_alignment = 0x300;
        assert!(matches!(
            plan_image_layout(&input, 0x400, limits()),
            Err(ImageLayoutError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_overlapping_section_destinations() {
        let mut input = report();
        let mut second = input.sections[0].clone();
        second.name = ".data".into();
        input.sections.push(second);
        assert!(matches!(
            plan_image_layout(&input, 0x400, limits()),
            Err(ImageLayoutError::Invalid("section image ranges overlap"))
        ));
    }

    #[test]
    fn rejects_overlapping_section_source_ranges() {
        let mut input = report();
        let mut second = input.sections[0].clone();
        second.name = ".data".into();
        second.virtual_address = 0x2000;
        second.raw_offset = 0x200;
        input.size_of_image = 0x3000;
        input.sections.push(second);
        assert!(matches!(
            plan_image_layout(&input, 0x600, limits()),
            Err(ImageLayoutError::Invalid("section raw file ranges overlap"))
        ));
    }

    #[test]
    fn rejects_section_destination_overlapping_headers() {
        let mut input = report();
        input.section_alignment = 0x100;
        input.file_alignment = 0x100;
        input.sections[0].virtual_address = 0x100;
        assert!(matches!(
            plan_image_layout(&input, 0x400, limits()),
            Err(ImageLayoutError::Invalid(
                "section image range overlaps image headers"
            ))
        ));
    }
}
