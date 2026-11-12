#![cfg(windows)]

use nexa_inspect::{inspect, plan_image_layout, ImageLayoutLimits};

#[test]
fn plans_the_compiler_produced_nexa_inspect_executable() {
    let executable_path = env!("CARGO_BIN_EXE_nexa-inspect");
    let bytes = std::fs::read(executable_path).expect("read compiler-produced PE fixture");
    let report = inspect(&bytes).expect("inspect compiler-produced PE fixture");

    assert_eq!(report.format, "PE32+");
    assert_eq!(report.machine_raw, 0x8664);
    assert_eq!(report.machine, "AMD64");
    assert_eq!(report.execution, "NOT_ATTEMPTED");
    assert!(!report.sections.is_empty());

    let plan = plan_image_layout(
        &report,
        bytes.len() as u64,
        ImageLayoutLimits {
            max_image_size: 256 * 1024 * 1024,
            max_sections: 96,
        },
    )
    .expect("plan compiler-produced PE32+ image");

    assert_eq!(plan.image_size, u64::from(report.size_of_image));
    assert_eq!(plan.headers.file_offset, 0);
    assert_eq!(plan.headers.image_rva, 0);
    assert_eq!(plan.headers.length, u64::from(report.size_of_headers));

    let mut expected_copy_bytes = plan.headers.length;
    let mut expected_zero_fill_bytes = 0_u64;
    for section in report
        .sections
        .iter()
        .filter(|section| section.virtual_size > 0 || section.raw_size > 0)
    {
        let mapped_size = if section.virtual_size == 0 {
            u64::from(section.raw_size)
        } else {
            u64::from(section.virtual_size)
        };
        let expected_copy = u64::from(section.raw_size.min(if section.virtual_size == 0 {
            section.raw_size
        } else {
            section.virtual_size
        }));
        let layout = plan
            .sections
            .iter()
            .find(|layout| {
                layout.name == section.name
                    && layout.image_rva == u64::from(section.virtual_address)
            })
            .expect("every nonempty compiler-produced section has a layout");

        assert_eq!(layout.mapped_size, mapped_size);
        assert_eq!(
            layout.copy.as_ref().map(|copy| copy.length),
            (expected_copy > 0).then_some(expected_copy)
        );
        let expected_zero_fill = mapped_size - expected_copy;
        assert_eq!(
            layout.zero_fill.as_ref().map(|range| range.length),
            (expected_zero_fill > 0).then_some(expected_zero_fill)
        );
        if let Some(copy) = &layout.copy {
            assert_eq!(copy.image_rva, layout.image_rva);
            expected_copy_bytes += copy.length;
        }
        if let Some(zero_fill) = &layout.zero_fill {
            assert_eq!(zero_fill.image_rva, layout.image_rva + expected_copy);
            expected_zero_fill_bytes += zero_fill.length;
        }
    }

    assert_eq!(plan.total_copy_bytes, expected_copy_bytes);
    assert_eq!(plan.total_zero_fill_bytes, expected_zero_fill_bytes);
    assert!(plan.total_copy_bytes >= plan.headers.length);
    assert!(plan.total_zero_fill_bytes <= plan.image_size);
}
