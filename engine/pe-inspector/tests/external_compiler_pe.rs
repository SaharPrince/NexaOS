use nexa_inspect::{inspect, plan_image_layout, ImageLayoutLimits};

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("PE header field is in bounds"),
    )
}

#[test]
fn plans_independently_compiled_mingw_pe32_plus_when_configured() {
    let Some(path) = std::env::var_os("NEXA_MINGW_PE_FIXTURE") else {
        eprintln!("skipping external MinGW PE32+ fixture: NEXA_MINGW_PE_FIXTURE is not set");
        return;
    };

    let bytes = std::fs::read(path).expect("read MinGW-produced PE32+ fixture");
    let pe_offset = read_u32(&bytes, 0x3c) as usize;
    let optional_header = pe_offset + 24;
    let import_directory = optional_header + 112 + 8;
    let import_rva = read_u32(&bytes, import_directory);
    let import_size = read_u32(&bytes, import_directory + 4);
    println!(
        "Raw PE32+ import directory: RVA={import_rva:#x}, size={import_size:#x}, size_mod_descriptor={}",
        import_size % 20
    );

    let report = inspect(&bytes).expect("inspect MinGW-produced PE32+ fixture");

    assert_eq!(report.format, "PE32+");
    assert_eq!(report.machine_raw, 0x8664);
    assert_eq!(report.machine, "AMD64");
    assert_eq!(report.execution, "NOT_ATTEMPTED");
    assert!(!report.sections.is_empty());
    for section in &report.sections {
        println!(
            "PE32+ section {}: RVA={:#x}, VirtualSize={:#x}, SizeOfRawData={:#x}",
            section.name, section.virtual_address, section.virtual_size, section.raw_size
        );
    }

    let has_initialized_section = report
        .sections
        .iter()
        .any(|section| section.raw_size > 0 && section.virtual_size > 0);
    let has_virtual_only_storage = report
        .sections
        .iter()
        .any(|section| section.virtual_size > section.raw_size);
    let has_raw_alignment_padding = report
        .sections
        .iter()
        .any(|section| section.virtual_size > 0 && section.raw_size > section.virtual_size);
    assert!(has_initialized_section, "expected initialized section data");
    assert!(
        has_virtual_only_storage,
        "expected virtual-only section storage for zero-initialized data"
    );
    assert!(
        has_raw_alignment_padding,
        "expected a file-aligned raw section larger than its logical virtual size"
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

    for section in report
        .sections
        .iter()
        .filter(|section| section.virtual_size > 0)
    {
        let layout = plan
            .sections
            .iter()
            .find(|layout| {
                layout.name == section.name
                    && layout.image_rva == u64::from(section.virtual_address)
            })
            .expect("every nonempty virtual section has a layout");
        assert_eq!(
            layout.mapped_size,
            u64::from(section.virtual_size),
            "mapped extent must follow VirtualSize for section {}",
            section.name
        );
        if section.raw_size > 0 {
            let expected_copy = section.raw_size.min(section.virtual_size);
            assert_eq!(
                layout.copy.as_ref().map(|copy| copy.length),
                Some(u64::from(expected_copy)),
                "copy must exclude file-alignment padding for section {}",
                section.name
            );
        }
    }

    if let Ok(version) = std::env::var("NEXA_MINGW_COMPILER_VERSION") {
        println!("External PE32+ fixture compiler: {version}");
    }
    println!(
        "MinGW PE32+ fixture: {} bytes, {} sections, {} copy bytes, {} zero-fill bytes",
        bytes.len(),
        report.sections.len(),
        plan.total_copy_bytes,
        plan.total_zero_fill_bytes
    );
}

#[test]
fn inspects_independently_compiled_mingw_pe32_x86_when_configured() {
    let Some(path) = std::env::var_os("NEXA_MINGW_PE32_FIXTURE") else {
        eprintln!("skipping external MinGW PE32 fixture: NEXA_MINGW_PE32_FIXTURE is not set");
        return;
    };

    let bytes = std::fs::read(path).expect("read MinGW-produced PE32 fixture");
    let report = inspect(&bytes).expect("inspect MinGW-produced PE32 fixture");

    assert_eq!(report.format, "PE32");
    assert_eq!(report.machine_raw, 0x014c);
    assert_eq!(report.machine, "x86");
    assert!(!report.sections.is_empty());
    assert_eq!(report.execution, "NOT_ATTEMPTED");

    if let Ok(version) = std::env::var("NEXA_MINGW_PE32_COMPILER_VERSION") {
        println!("External PE32 fixture compiler: {version}");
    }
    println!(
        "MinGW PE32 fixture: {} bytes, {} sections; inspection only (planner remains AMD64-only)",
        bytes.len(),
        report.sections.len()
    );
}


#[test]
fn plans_independently_compiled_llvm_lld_pe32_plus_when_configured() {
    let Some(path) = std::env::var_os("NEXA_LLVM_PE_FIXTURE") else {
        eprintln!("skipping external LLVM/LLD PE32+ fixture: NEXA_LLVM_PE_FIXTURE is not set");
        return;
    };

    let bytes = std::fs::read(path).expect("read LLVM/LLD-produced PE32+ fixture");
    let report = inspect(&bytes).expect("inspect LLVM/LLD-produced PE32+ fixture");

    assert_eq!(report.format, "PE32+");
    assert_eq!(report.machine_raw, 0x8664);
    assert_eq!(report.machine, "AMD64");
    assert_eq!(report.execution, "NOT_ATTEMPTED");
    assert!(!report.sections.is_empty());

    assert!(
        report.sections.iter().any(|section| section.raw_size > 0 && section.virtual_size > 0),
        "expected initialized data from LLVM/LLD fixture"
    );
    assert!(
        report.sections.iter().any(|section| section.virtual_size > section.raw_size),
        "expected virtual-only storage from LLVM/LLD fixture"
    );
    assert!(
        report.sections.iter().any(|section| section.virtual_size > 0 && section.raw_size > section.virtual_size),
        "expected file-alignment padding from LLVM/LLD fixture"
    );

    let plan = plan_image_layout(
        &report,
        bytes.len() as u64,
        ImageLayoutLimits {
            max_image_size: 256 * 1024 * 1024,
            max_sections: 96,
        },
    )
    .expect("plan LLVM/LLD-produced PE32+ image");

    assert_eq!(plan.image_size, u64::from(report.size_of_image));
    assert!(plan.total_copy_bytes >= plan.headers.length);
    assert!(plan.total_zero_fill_bytes > 0);
    assert!(plan.sections.iter().all(|section| section.mapped_size > 0));

    for section in report.sections.iter().filter(|section| section.virtual_size > 0) {
        let layout = plan.sections.iter().find(|layout| {
            layout.name == section.name
                && layout.image_rva == u64::from(section.virtual_address)
        }).expect("every nonempty virtual section has a layout");
        assert_eq!(layout.mapped_size, u64::from(section.virtual_size));
        if section.raw_size > 0 {
            assert_eq!(
                layout.copy.as_ref().map(|copy| copy.length),
                Some(u64::from(section.raw_size.min(section.virtual_size))),
                "copy length must exclude raw alignment padding for {}",
                section.name
            );
        }
    }

    if let Ok(version) = std::env::var("NEXA_LLVM_COMPILER_VERSION") {
        println!("External LLVM compiler: {version}");
    }
    if let Ok(version) = std::env::var("NEXA_LLD_LINKER_VERSION") {
        println!("External LLD linker: {version}");
    }
    println!(
        "LLVM/LLD PE32+ fixture: {} bytes, {} sections, {} copy bytes, {} zero-fill bytes",
        bytes.len(),
        report.sections.len(),
        plan.total_copy_bytes,
        plan.total_zero_fill_bytes
    );
}
