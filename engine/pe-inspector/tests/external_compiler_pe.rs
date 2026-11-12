use nexa_inspect::{inspect, plan_image_layout, ImageLayoutLimits};

#[test]
fn plans_independently_compiled_mingw_pe32_plus_when_configured() {
    let Some(path) = std::env::var_os("NEXA_MINGW_PE_FIXTURE") else {
        eprintln!("skipping external MinGW fixture: NEXA_MINGW_PE_FIXTURE is not set");
        return;
    };

    let bytes = std::fs::read(path).expect("read MinGW-produced PE fixture");
    let report = inspect(&bytes).expect("inspect MinGW-produced PE fixture");

    assert_eq!(report.format, "PE32+");
    assert_eq!(report.machine_raw, 0x8664);
    assert!(!report.sections.is_empty());

    let has_initialized_section = report
        .sections
        .iter()
        .any(|section| section.raw_size > 0 && section.virtual_size > 0);
    let has_virtual_only_storage = report
        .sections
        .iter()
        .any(|section| section.virtual_size > section.raw_size);
    assert!(has_initialized_section, "expected initialized section data");
    assert!(
        has_virtual_only_storage,
        "expected virtual-only section storage for zero-initialized data"
    );

    let plan = plan_image_layout(
        &report,
        bytes.len() as u64,
        ImageLayoutLimits {
            max_image_size: 256 * 1024 * 1024,
            max_sections: 96,
        },
    )
    .expect("plan MinGW-produced PE32+ image");

    assert_eq!(plan.image_size, u64::from(report.size_of_image));
    assert!(plan.total_copy_bytes >= plan.headers.length);
    assert!(
        plan.total_zero_fill_bytes > 0,
        "expected zero-fill mapping for virtual-only section storage"
    );
    assert!(plan.sections.iter().all(|section| section.mapped_size > 0));

    if let Ok(version) = std::env::var("NEXA_MINGW_COMPILER_VERSION") {
        println!("External PE fixture compiler: {version}");
    }
    println!(
        "MinGW PE32+ fixture: {} bytes, {} sections, {} copy bytes, {} zero-fill bytes",
        bytes.len(),
        report.sections.len(),
        plan.total_copy_bytes,
        plan.total_zero_fill_bytes
    );
}
