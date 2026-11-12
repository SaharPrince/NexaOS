#![cfg(windows)]

use nexa_inspect::{inspect, plan_image_layout, ImageLayoutLimits};

#[test]
fn plans_the_compiler_produced_nexa_inspect_executable() {
    let executable_path = env!("CARGO_BIN_EXE_nexa-inspect");
    let bytes = std::fs::read(executable_path).expect("read compiler-produced PE fixture");
    let report = inspect(&bytes).expect("inspect compiler-produced PE fixture");

    assert_eq!(report.format, "PE32+");
    assert_eq!(report.machine_raw, 0x8664);
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
    assert!(plan.total_copy_bytes >= plan.headers.length);
    assert!(plan.total_zero_fill_bytes <= plan.image_size);
    assert!(plan.sections.iter().all(|section| section.mapped_size > 0));
}
