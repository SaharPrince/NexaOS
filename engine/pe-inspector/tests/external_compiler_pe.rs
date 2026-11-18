use nexa_inspect::{inspect, plan_image_layout, ImageLayoutLimits, PeReport};

fn print_section_transition_diagnostics(report: &PeReport) {
    let alignment = u64::from(report.section_alignment);
    if alignment == 0 || !alignment.is_power_of_two() {
        println!(
            "Section transition diagnostics unavailable: invalid SectionAlignment {alignment:#x}"
        );
        return;
    }

    for (index, pair) in report.sections.windows(2).enumerate() {
        let previous = &pair[0];
        let next = &pair[1];
        let mapped_size = if previous.virtual_size == 0 {
            u64::from(previous.raw_size)
        } else {
            u64::from(previous.virtual_size)
        };
        let Some(aligned_extent) = mapped_size
            .checked_add(alignment - 1)
            .map(|rounded| rounded & !(alignment - 1))
        else {
            println!("Section transition {index}: aligned extent overflow");
            continue;
        };
        let Some(expected_next_rva) =
            u64::from(previous.virtual_address).checked_add(aligned_extent)
        else {
            println!("Section transition {index}: expected RVA overflow");
            continue;
        };
        let actual_next_rva = u64::from(next.virtual_address);
        let classification = if actual_next_rva < u64::from(previous.virtual_address) {
            "out-of-order"
        } else if actual_next_rva < expected_next_rva {
            "overlap"
        } else if actual_next_rva > expected_next_rva {
            "gap"
        } else {
            "adjacent"
        };
        let extent_source = if previous.virtual_size == 0 {
            "raw-size-fallback"
        } else {
            "VirtualSize"
        };

        println!(
            "Section transition {index}: {previous_name} -> {next_name}; extent_source={extent_source}; previous_rva={previous_rva:#x}; aligned_extent={aligned_extent:#x}; expected_next_rva={expected_next_rva:#x}; actual_next_rva={actual_next_rva:#x}; classification={classification}",
            previous_name = previous.name,
            next_name = next.name,
            previous_rva = previous.virtual_address,
            actual_next_rva = next.virtual_address,
        );
    }
}

fn assert_section_order_and_adjacency(report: &PeReport) {
    let alignment = u64::from(report.section_alignment);
    assert!(
        alignment != 0 && alignment.is_power_of_two(),
        "fixture has invalid SectionAlignment: {alignment:#x}"
    );

    for (index, pair) in report.sections.windows(2).enumerate() {
        let previous = &pair[0];
        let next = &pair[1];
        let mapped_size = if previous.virtual_size == 0 {
            u64::from(previous.raw_size)
        } else {
            u64::from(previous.virtual_size)
        };
        let aligned_extent = mapped_size
            .checked_add(alignment - 1)
            .map(|rounded| rounded & !(alignment - 1))
            .expect("aligned section extent must not overflow");
        let expected_next_rva = u64::from(previous.virtual_address)
            .checked_add(aligned_extent)
            .expect("expected next section RVA must not overflow");
        let actual_next_rva = u64::from(next.virtual_address);

        assert!(
            actual_next_rva > u64::from(previous.virtual_address),
            "section table is not in strictly ascending RVA order at transition {index}: {} -> {}",
            previous.name,
            next.name
        );
        assert_eq!(
            actual_next_rva, expected_next_rva,
            "compiler-produced fixture has a non-adjacent aligned section transition {index}: {} -> {}",
            previous.name, next.name
        );
    }
}

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
    print_section_transition_diagnostics(&report);
    assert_section_order_and_adjacency(&report);

    assert_eq!(report.format, "PE32+");
    assert_eq!(report.machine_raw, 0x8664);
    assert_eq!(report.machine, "AMD64");
    assert_eq!(report.execution, "NOT_ATTEMPTED");
    assert!(!report.sections.is_empty());
    assert!(
        !report.imports.is_empty(),
        "MinGW fixture should expose at least one parsed import library"
    );
    assert!(
        !report.import_metadata.is_empty(),
        "MinGW fixture should preserve typed import metadata"
    );
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
    print_section_transition_diagnostics(&report);
    assert_section_order_and_adjacency(&report);

    assert_eq!(report.format, "PE32+");
    assert_eq!(report.machine_raw, 0x8664);
    assert_eq!(report.machine, "AMD64");
    assert_eq!(report.execution, "NOT_ATTEMPTED");
    assert!(!report.sections.is_empty());

    assert!(
        report
            .sections
            .iter()
            .any(|section| section.raw_size > 0 && section.virtual_size > 0),
        "expected initialized data from LLVM/LLD fixture"
    );
    assert!(
        report
            .sections
            .iter()
            .any(|section| section.virtual_size > section.raw_size),
        "expected virtual-only storage from LLVM/LLD fixture"
    );
    assert!(
        report
            .sections
            .iter()
            .any(|section| section.virtual_size > 0 && section.raw_size > section.virtual_size),
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

#[test]
fn inspects_llvm_relocation_fixture_when_configured() {
    let Some(path) = std::env::var_os("NEXA_LLVM_RELOC_FIXTURE") else {
        eprintln!("skipping LLVM relocation fixture: NEXA_LLVM_RELOC_FIXTURE is not set");
        return;
    };

    let bytes = std::fs::read(path).expect("read LLVM relocation fixture");
    let report = inspect(&bytes).expect("inspect LLVM relocation fixture");
    print_section_transition_diagnostics(&report);

    assert_eq!(report.format, "PE32+");
    assert_eq!(report.machine_raw, 0x8664);
    assert_eq!(report.machine, "AMD64");
    assert_eq!(report.execution, "NOT_ATTEMPTED");

    let metadata = report
        .relocation_metadata
        .as_ref()
        .expect("LLVM relocation fixture should contain a base-relocation directory");
    assert!(!metadata.blocks.is_empty(), "expected relocation blocks");
    let encoded_entries: usize = metadata
        .blocks
        .iter()
        .map(|block| block.entries.len())
        .sum();
    assert!(encoded_entries > 0, "expected encoded relocation entries");
    assert_eq!(report.relocation_blocks, metadata.blocks.len());
    assert_eq!(report.relocation_entries, encoded_entries);

    let plan = plan_image_layout(
        &report,
        bytes.len() as u64,
        ImageLayoutLimits {
            max_image_size: 256 * 1024 * 1024,
            max_sections: 96,
        },
    )
    .expect("plan LLVM relocation fixture");
    assert_eq!(plan.image_size, u64::from(report.size_of_image));

    println!(
        "LLVM relocation fixture: {} bytes, {} sections, {} relocation blocks, {} encoded entries",
        bytes.len(),
        report.sections.len(),
        report.relocation_blocks,
        report.relocation_entries
    );
}

#[test]
fn inspects_llvm_custom_image_base_fixture_when_configured() {
    let Some(path) = std::env::var_os("NEXA_LLVM_BASE_FIXTURE") else {
        eprintln!("skipping LLVM custom image-base fixture: NEXA_LLVM_BASE_FIXTURE is not set");
        return;
    };

    let bytes = std::fs::read(path).expect("read LLVM custom image-base fixture");
    let report = inspect(&bytes).expect("inspect LLVM custom image-base fixture");
    assert_eq!(report.format, "PE32+");
    assert_eq!(report.machine_raw, 0x8664);
    assert_eq!(report.machine, "AMD64");
    assert_eq!(report.execution, "NOT_ATTEMPTED");
    assert_eq!(report.image_base, 0x280000000);

    let plan = plan_image_layout(
        &report,
        bytes.len() as u64,
        ImageLayoutLimits {
            max_image_size: 256 * 1024 * 1024,
            max_sections: 96,
        },
    )
    .expect("plan LLVM custom image-base fixture");
    assert_eq!(plan.image_size, u64::from(report.size_of_image));

    println!(
        "LLVM custom image-base fixture: {} bytes, image base={:#x}, {} sections",
        bytes.len(),
        report.image_base,
        report.sections.len()
    );
}

#[test]
fn inspects_llvm_shared_bss_fixture_when_configured() {
    let Some(path) = std::env::var_os("NEXA_LLVM_BSS_FIXTURE") else {
        eprintln!("skipping LLVM shared-BSS fixture: NEXA_LLVM_BSS_FIXTURE is not set");
        return;
    };

    let bytes = std::fs::read(path).expect("read LLVM shared-BSS fixture");
    let report = inspect(&bytes).expect("inspect LLVM shared-BSS fixture");
    print_section_transition_diagnostics(&report);
    assert_eq!(report.format, "PE32+");
    assert_eq!(report.machine_raw, 0x8664);
    assert_eq!(report.machine, "AMD64");
    assert_eq!(report.execution, "NOT_ATTEMPTED");

    let bss = report
        .sections
        .iter()
        .find(|section| section.name == ".bss")
        .expect("shared-data fixture should retain a separate .bss section");
    assert_eq!(
        bss.raw_size, 0,
        "BSS section should have no file-backed data"
    );
    assert!(
        bss.virtual_size > 0,
        "BSS section should retain virtual-only storage"
    );

    let plan = plan_image_layout(
        &report,
        bytes.len() as u64,
        ImageLayoutLimits {
            max_image_size: 256 * 1024 * 1024,
            max_sections: 96,
        },
    )
    .expect("plan LLVM shared-BSS fixture");
    let bss_layout = plan
        .sections
        .iter()
        .find(|section| {
            section.name == bss.name && section.image_rva == u64::from(bss.virtual_address)
        })
        .expect("BSS section should have an image layout");
    assert_eq!(bss_layout.mapped_size, u64::from(bss.virtual_size));
    assert!(bss_layout.copy.is_none());
    assert!(
        plan.total_zero_fill_bytes >= u64::from(bss.virtual_size),
        "BSS storage should contribute to zero-fill accounting"
    );

    println!(
        "LLVM shared-BSS fixture: {} bytes, .bss VirtualSize={:#x}, raw size=0, zero-fill bytes={}",
        bytes.len(),
        bss.virtual_size,
        plan.total_zero_fill_bytes
    );
}

#[test]
fn plans_windows_system_pe_when_configured() {
    let Some(path) = std::env::var_os("NEXA_WINDOWS_SYSTEM_PE_FIXTURE") else {
        eprintln!("skipping Windows system PE fixture: NEXA_WINDOWS_SYSTEM_PE_FIXTURE is not set");
        return;
    };

    let bytes = std::fs::read(path).expect("read Windows system PE fixture");
    let report = inspect(&bytes).expect("inspect Windows system PE fixture");
    print_section_transition_diagnostics(&report);
    assert_section_order_and_adjacency(&report);

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
    .expect("plan Windows system PE image");

    assert_eq!(plan.image_size, u64::from(report.size_of_image));
    assert!(plan.total_copy_bytes >= plan.headers.length);
    assert!(plan.sections.iter().all(|section| section.mapped_size > 0));

    println!(
        "Windows system PE fixture: {} bytes, {} sections, {} copy bytes, {} zero-fill bytes",
        bytes.len(),
        report.sections.len(),
        plan.total_copy_bytes,
        plan.total_zero_fill_bytes
    );
}

#[test]
fn inspects_llvm_file_and_default_alignment_fixtures_when_configured() {
    let limits = ImageLayoutLimits {
        max_image_size: 256 * 1024 * 1024,
        max_sections: 96,
    };

    for (variable, expected_alignment) in [
        ("NEXA_LLVM_FILEALIGN_DEFAULT_FIXTURE", 512),
        ("NEXA_LLVM_FILEALIGN_4096_FIXTURE", 4096),
    ] {
        let Some(path) = std::env::var_os(variable) else {
            eprintln!("skipping {variable}: environment variable is not set");
            continue;
        };
        let bytes = std::fs::read(path).expect("read LLVM file-alignment fixture");
        let report = inspect(&bytes).expect("inspect LLVM file-alignment fixture");
        assert_eq!(report.format, "PE32+");
        assert_eq!(report.machine, "AMD64");
        assert_eq!(report.file_alignment, expected_alignment);
        assert_eq!(report.execution, "NOT_ATTEMPTED");

        let plan = plan_image_layout(&report, bytes.len() as u64, limits)
            .expect("plan LLVM file-alignment fixture");
        assert_eq!(plan.image_size, u64::from(report.size_of_image));
        println!(
            "LLVM file-alignment fixture: alignment={}, {} bytes, {} sections",
            report.file_alignment,
            bytes.len(),
            report.sections.len()
        );
    }

    if let Some(path) = std::env::var_os("NEXA_LLVM_DEFAULT_ALIGNMENT_FIXTURE") {
        let bytes = std::fs::read(path).expect("read LLVM default-alignment fixture");
        let report = inspect(&bytes).expect("inspect LLVM default-alignment fixture");
        assert_eq!(report.format, "PE32+");
        assert_eq!(report.machine, "AMD64");
        assert_eq!(report.execution, "NOT_ATTEMPTED");

        let rdata = report
            .sections
            .iter()
            .find(|section| section.name == ".rdata")
            .expect("default-alignment fixture contains merged .rdata section");
        let start = rdata.raw_offset as usize;
        let end = start
            .checked_add(rdata.raw_size as usize)
            .expect("raw .rdata range must not overflow");
        let contents = bytes
            .get(start..end)
            .expect("raw .rdata range must be inside the input");
        assert!(
            contents.len() >= 17,
            "merged .rdata section must contain both aligned input contributions"
        );
        assert_eq!(contents[0], 1);
        assert!(
            contents[1..16].iter().all(|byte| *byte == 0),
            "default input-section alignment padding must be zero-filled"
        );
        assert_eq!(contents[16], 2);

        let plan = plan_image_layout(&report, bytes.len() as u64, limits)
            .expect("plan LLVM default-alignment fixture");
        assert_eq!(plan.image_size, u64::from(report.size_of_image));
        println!(
            "LLVM default-alignment fixture: {} bytes, .rdata raw size={}, merged input spacing=16",
            bytes.len(),
            rdata.raw_size
        );
    } else {
        eprintln!("skipping LLVM default-alignment fixture: NEXA_LLVM_DEFAULT_ALIGNMENT_FIXTURE is not set");
    }
}
