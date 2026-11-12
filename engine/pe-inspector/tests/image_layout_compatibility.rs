use nexa_inspect::{plan_image_layout, ImageLayoutLimits, PeReport, Section};

fn report(sections: Vec<Section>) -> PeReport {
    PeReport {
        format: "PE32+".into(),
        execution: "NOT_ATTEMPTED",
        machine: "AMD64".into(),
        machine_raw: 0x8664,
        sections,
        image_base: 0x1_4000_0000,
        size_of_image: 0x3000,
        size_of_headers: 0x200,
        section_alignment: 0x1000,
        file_alignment: 0x200,
        entry_point_rva: 0x1000,
        subsystem: "Windows Console".into(),
        directories: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
        relocation_blocks: 0,
        relocation_entries: 0,
    }
}

fn section(
    name: &str,
    virtual_size: u32,
    virtual_address: u32,
    raw_size: u32,
    raw_offset: u32,
) -> Section {
    Section {
        name: name.into(),
        virtual_size,
        virtual_address,
        raw_size,
        raw_offset,
        characteristics: 0x4000_0040,
    }
}

fn limits() -> ImageLayoutLimits {
    ImageLayoutLimits {
        max_image_size: 16 * 1024 * 1024,
        max_sections: 96,
    }
}

#[test]
fn plans_bss_only_section_as_zero_fill_without_file_source() {
    let input = report(vec![section(".bss", 0x600, 0x1000, 0, 0)]);
    let plan = plan_image_layout(&input, 0x200, limits()).unwrap();

    assert_eq!(plan.sections.len(), 1);
    assert_eq!(plan.sections[0].copy, None);
    assert_eq!(
        plan.sections[0].zero_fill.as_ref().unwrap().image_rva,
        0x1000
    );
    assert_eq!(plan.sections[0].zero_fill.as_ref().unwrap().length, 0x600);
    assert_eq!(plan.total_copy_bytes, 0x200);
    assert_eq!(plan.total_zero_fill_bytes, 0x600);
}

#[test]
fn plans_initialized_section_without_unnecessary_zero_fill() {
    let input = report(vec![section(".text", 0x200, 0x1000, 0x200, 0x200)]);
    let plan = plan_image_layout(&input, 0x400, limits()).unwrap();

    assert_eq!(plan.sections.len(), 1);
    assert_eq!(plan.sections[0].copy.as_ref().unwrap().file_offset, 0x200);
    assert_eq!(plan.sections[0].copy.as_ref().unwrap().length, 0x200);
    assert_eq!(plan.sections[0].zero_fill, None);
    assert_eq!(plan.total_copy_bytes, 0x400);
    assert_eq!(plan.total_zero_fill_bytes, 0);
}

#[test]
fn plans_multiple_independent_initialized_sections() {
    let input = report(vec![
        section(".text", 0x200, 0x1000, 0x200, 0x200),
        section(".data", 0x200, 0x2000, 0x200, 0x400),
    ]);
    let plan = plan_image_layout(&input, 0x600, limits()).unwrap();

    assert_eq!(plan.sections.len(), 2);
    assert_eq!(plan.sections[0].copy.as_ref().unwrap().image_rva, 0x1000);
    assert_eq!(plan.sections[1].copy.as_ref().unwrap().image_rva, 0x2000);
    assert_eq!(plan.total_copy_bytes, 0x600);
    assert_eq!(plan.total_zero_fill_bytes, 0);
}

#[test]
fn accepts_low_section_alignment_when_file_alignment_matches() {
    let mut input = report(vec![section(".text", 0x200, 0x200, 0x200, 0x200)]);
    input.size_of_image = 0x1000;
    input.size_of_headers = 0x200;
    input.section_alignment = 0x200;
    input.file_alignment = 0x200;
    input.entry_point_rva = 0x200;

    let plan = plan_image_layout(&input, 0x400, limits()).unwrap();

    assert_eq!(plan.image_size, 0x1000);
    assert_eq!(plan.sections[0].image_rva, 0x200);
    assert_eq!(plan.sections[0].copy.as_ref().unwrap().length, 0x200);
}

#[test]
fn rejects_low_section_alignment_when_raw_offset_differs_from_rva() {
    let mut input = report(vec![section(".text", 0x200, 0x200, 0x200, 0x400)]);
    input.size_of_image = 0x1000;
    input.section_alignment = 0x200;
    input.file_alignment = 0x200;

    assert!(matches!(
        plan_image_layout(&input, 0x600, limits()),
        Err(nexa_inspect::ImageLayoutError::Invalid(
            "low-alignment section raw offset must equal its RVA"
        ))
    ));
}

#[test]
fn rejects_low_section_alignment_when_file_alignment_differs() {
    let mut input = report(vec![section(".text", 0x200, 0x200, 0x200, 0x200)]);
    input.size_of_image = 0x1000;
    input.section_alignment = 0x200;
    input.file_alignment = 0x100;

    assert!(matches!(
        plan_image_layout(&input, 0x400, limits()),
        Err(nexa_inspect::ImageLayoutError::Invalid(
            "low SectionAlignment requires matching FileAlignment"
        ))
    ));
}
