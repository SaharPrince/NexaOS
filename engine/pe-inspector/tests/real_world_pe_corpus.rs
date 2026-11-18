use nexa_inspect::{
    allocate_image_buffer, apply_base_relocations, inspect, plan_base_relocations,
    plan_image_layout, populate_image_buffer, ImageLayoutLimits, RelocationPlanLimits,
    SystemImageAllocator,
};
use std::{fs, path::PathBuf};

struct Expected {
    file: &'static str,
    blocks: usize,
    entries: usize,
    absolute: usize,
    dir64: usize,
}

const SAMPLES: [Expected; 4] = [
    Expected {
        file: "ripgrep.exe",
        blocks: 118,
        entries: 12_852,
        absolute: 53,
        dir64: 12_799,
    },
    Expected {
        file: "bat.exe",
        blocks: 124,
        entries: 11_374,
        absolute: 52,
        dir64: 11_322,
    },
    Expected {
        file: "jq.exe",
        blocks: 16,
        entries: 1_358,
        absolute: 4,
        dir64: 1_354,
    },
    Expected {
        file: "sqlite3.dll",
        blocks: 30,
        entries: 2_910,
        absolute: 16,
        dir64: 2_894,
    },
];

#[test]
fn independently_published_pe_samples_pass_bounded_layout_and_relocation_path() {
    let Some(directory) = std::env::var_os("NEXA_REAL_WORLD_PE_DIR") else {
        eprintln!(
            "SKIP: real-world PE corpus was NOT validated; NEXA_REAL_WORLD_PE_DIR is not set"
        );
        return;
    };
    let directory = PathBuf::from(directory);
    let mut total_entries = 0usize;
    let mut total_absolute = 0usize;
    let mut total_dir64 = 0usize;

    for expected in SAMPLES {
        let path = directory.join(expected.file);
        let input = fs::read(&path)
            .unwrap_or_else(|error| panic!("read verified sample {}: {error}", path.display()));
        let original_input = input.clone();
        let report =
            inspect(&input).unwrap_or_else(|error| panic!("inspect {}: {error}", expected.file));

        assert_eq!(report.format, "PE32+", "{} format", expected.file);
        assert_eq!(report.machine_raw, 0x8664, "{} machine", expected.file);
        assert_eq!(report.execution, "NOT_ATTEMPTED");
        assert_eq!(
            report.relocation_blocks, expected.blocks,
            "{} blocks",
            expected.file
        );
        assert_eq!(
            report.relocation_entries, expected.entries,
            "{} entries",
            expected.file
        );

        let metadata = report
            .relocation_metadata
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no relocation metadata", expected.file));
        let encoded_entries: Vec<_> = metadata
            .blocks
            .iter()
            .flat_map(|block| block.entries.iter())
            .collect();
        let absolute = encoded_entries
            .iter()
            .filter(|entry| entry.relocation_type == 0)
            .count();
        let dir64 = encoded_entries
            .iter()
            .filter(|entry| entry.relocation_type == 10)
            .count();
        assert_eq!(
            encoded_entries.len(),
            expected.entries,
            "{} encoded entries",
            expected.file
        );
        assert_eq!(absolute, expected.absolute, "{} ABSOLUTE", expected.file);
        assert_eq!(dir64, expected.dir64, "{} DIR64", expected.file);
        assert_eq!(
            absolute + dir64,
            encoded_entries.len(),
            "{} unsupported relocations",
            expected.file
        );

        let layout = plan_image_layout(
            &report,
            input.len() as u64,
            ImageLayoutLimits {
                max_image_size: 256 * 1024 * 1024,
                max_sections: 96,
            },
        )
        .unwrap_or_else(|error| panic!("plan layout for {}: {error}", expected.file));

        let requested_base = report
            .image_base
            .checked_add(0x10_0000)
            .expect("test relocation base must not overflow");
        let mut allocator = SystemImageAllocator;
        let image = allocate_image_buffer(&layout, 256 * 1024 * 1024, &mut allocator)
            .unwrap_or_else(|error| panic!("allocate {}: {error}", expected.file));
        let mut image = populate_image_buffer(&layout, &input, image)
            .unwrap_or_else(|error| panic!("populate {}: {error}", expected.file));

        let plan = plan_base_relocations(
            &report.format,
            report.machine_raw,
            report.image_base,
            requested_base,
            layout.image_size,
            Some(metadata),
            image.as_slice(),
            RelocationPlanLimits::default(),
        )
        .unwrap_or_else(|error| panic!("plan relocations for {}: {error}", expected.file));
        assert_eq!(
            plan.patches.len(),
            expected.dir64,
            "{} planned patches",
            expected.file
        );

        for patch in &plan.patches {
            let start = patch.target_rva as usize;
            let current = u64::from_le_bytes(
                image.as_slice()[start..start + 8]
                    .try_into()
                    .expect("validated patch range"),
            );
            assert_eq!(
                current, patch.original_value,
                "{} original patch value",
                expected.file
            );
        }

        apply_base_relocations(&mut image, &plan)
            .unwrap_or_else(|error| panic!("apply relocations for {}: {error}", expected.file));

        for patch in &plan.patches {
            let start = patch.target_rva as usize;
            let relocated = u64::from_le_bytes(
                image.as_slice()[start..start + 8]
                    .try_into()
                    .expect("validated patch range"),
            );
            assert_eq!(
                relocated, patch.relocated_value,
                "{} relocated patch value",
                expected.file
            );
        }

        assert_eq!(
            input, original_input,
            "{} input bytes changed",
            expected.file
        );
        println!(
            "PASS {}: {} blocks, {} entries ({} ABSOLUTE, {} DIR64); {} patches applied; execution NOT_ATTEMPTED",
            expected.file, expected.blocks, expected.entries, absolute, dir64, plan.patches.len()
        );
        total_entries += expected.entries;
        total_absolute += absolute;
        total_dir64 += plan.patches.len();
    }

    assert_eq!(total_entries, 28_494);
    assert_eq!(total_absolute, 125);
    assert_eq!(total_dir64, 28_369);
    println!(
        "REAL-WORLD PE CORPUS PASS: 4 samples; {total_entries} entries; {total_absolute} ABSOLUTE; {total_dir64} DIR64 patches applied"
    );
}
