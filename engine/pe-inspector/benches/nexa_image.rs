use nexa_inspect::{
    allocate_image_buffer, plan_image_layout, populate_image_buffer, DataDirectory,
    ImageLayoutLimits, ImportLibrary, PeReport, Section, SystemImageAllocator,
};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

const SECTION_COUNT: usize = 4;
const SECTION_RAW_SIZE: u32 = 0x1_0000;
const SECTION_VIRTUAL_SIZE: u32 = 0x1_8000;
const IMAGE_SIZE: u32 = 0x8_0000;
const HEADER_SIZE: u32 = 0x200;

fn iterations_from_env(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
        .clamp(1, 1_000_000)
}

fn fixture() -> (PeReport, Vec<u8>) {
    let mut sections = Vec::with_capacity(SECTION_COUNT);
    for index in 0..SECTION_COUNT {
        sections.push(Section {
            name: format!(".s{index}"),
            virtual_size: SECTION_VIRTUAL_SIZE,
            virtual_address: 0x1000 + index as u32 * 0x20_000,
            raw_size: SECTION_RAW_SIZE,
            raw_offset: HEADER_SIZE + index as u32 * SECTION_RAW_SIZE,
            characteristics: 0x4000_0040,
        });
    }

    let file_size = (HEADER_SIZE + SECTION_COUNT as u32 * SECTION_RAW_SIZE) as usize;
    let input = vec![0x5a; file_size];
    let report = PeReport {
        format: "PE32+".into(),
        execution: "NOT_ATTEMPTED",
        machine: "AMD64".into(),
        machine_raw: 0x8664,
        sections,
        image_base: 0x1_4000_0000,
        size_of_image: IMAGE_SIZE,
        size_of_headers: HEADER_SIZE,
        section_alignment: 0x1000,
        file_alignment: 0x200,
        entry_point_rva: 0x1000,
        subsystem: "Windows Console".into(),
        directories: Vec::<DataDirectory>::new(),
        imports: Vec::<ImportLibrary>::new(),
        exports: Vec::new(),
        relocation_blocks: 0,
        relocation_entries: 0,
    };
    (report, input)
}

fn report_rate(label: &str, elapsed: Duration, iterations: u64) {
    let nanos_per_iteration = elapsed.as_nanos() / u128::from(iterations);
    println!(
        "{label}: iterations={iterations}, total_ms={:.3}, ns_per_iteration={nanos_per_iteration}",
        elapsed.as_secs_f64() * 1_000.0
    );
}

fn main() {
    let planning_iterations = iterations_from_env("NEXA_BENCH_PLAN_ITERS", 1_000);
    let buffer_iterations = iterations_from_env("NEXA_BENCH_BUFFER_ITERS", 100);
    let (report, input) = fixture();
    let limits = ImageLayoutLimits {
        max_image_size: u64::from(IMAGE_SIZE),
        max_sections: SECTION_COUNT,
    };
    let file_len = input.len() as u64;

    let planning_started = Instant::now();
    for _ in 0..planning_iterations {
        let plan = plan_image_layout(black_box(&report), file_len, limits)
            .expect("benchmark PE fixture must produce a valid plan");
        black_box(plan);
    }
    let planning_elapsed = planning_started.elapsed();

    let plan = plan_image_layout(&report, file_len, limits)
        .expect("benchmark PE fixture must produce a valid plan");

    let mut allocator = SystemImageAllocator;
    let allocation_started = Instant::now();
    for _ in 0..buffer_iterations {
        let image = allocate_image_buffer(&plan, u64::from(IMAGE_SIZE), &mut allocator)
            .expect("benchmark image allocation must succeed");
        black_box(image);
    }
    let allocation_elapsed = allocation_started.elapsed();

    let mut population_elapsed = Duration::ZERO;
    for _ in 0..buffer_iterations {
        let image = allocate_image_buffer(&plan, u64::from(IMAGE_SIZE), &mut allocator)
            .expect("benchmark image allocation must succeed");
        let population_started = Instant::now();
        let populated = populate_image_buffer(black_box(&plan), black_box(&input), image)
            .expect("benchmark image population must succeed");
        population_elapsed += population_started.elapsed();
        black_box(populated);
    }

    let image_bytes = u64::from(IMAGE_SIZE);
    let copy_bytes = plan.total_copy_bytes;
    let zero_fill_bytes = plan.total_zero_fill_bytes;
    println!("NexaOS NCE image-layout microbenchmark (diagnostic; not a statistical benchmark)");
    println!(
        "fixture: sections={}, image_bytes={image_bytes}, input_bytes={}, copy_bytes={copy_bytes}, zero_fill_bytes={zero_fill_bytes}",
        report.sections.len(),
        input.len()
    );
    report_rate("planning", planning_elapsed, planning_iterations);
    report_rate(
        "image_buffer_allocation",
        allocation_elapsed,
        buffer_iterations,
    );
    report_rate("population_only", population_elapsed, buffer_iterations);
    println!(
        "logical_image_buffer_allocations: allocation_only={}, population_runs={}",
        buffer_iterations, buffer_iterations
    );
    println!(
        "configuration: NEXA_BENCH_PLAN_ITERS (default 1000), NEXA_BENCH_BUFFER_ITERS (default 100); each is clamped to 1..=1000000"
    );
    println!(
        "note: counts are calls to the image-buffer allocator, not process-wide heap allocation counts; peak RSS is not measured"
    );

    // Optional hold keeps the process alive long enough for an external
    // process monitor to read its peak working set. It is outside all timed
    // benchmark phases and is disabled during normal benchmark runs.
    let hold_ms = std::env::var("NEXA_BENCH_HOLD_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
        .clamp(0, 30_000);
    if hold_ms > 0 {
        println!("external_measurement_hold_ms={hold_ms}");
        std::thread::sleep(Duration::from_millis(hold_ms));
    }
}
