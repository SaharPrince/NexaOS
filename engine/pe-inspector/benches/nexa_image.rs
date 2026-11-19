use nexa_inspect::{
    allocate_image_buffer, inspect, plan_image_layout, populate_image_buffer, prepare_image,
    ExportCatalogue, ExportCatalogueLimits, IatPatchPlanLimits, ImageAllocationError,
    ImageAllocator, ImageLayoutLimits, ImagePreparationRequest, ImportResolutionLimits,
    RelocationPlanLimits, SystemImageAllocator,
};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

const INPUT_SIZE: usize = 0x400;
const IMAGE_SIZE: u64 = 0x2000;
const IMAGE_BASE: u64 = 0x140000000;

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

// A deterministic PE32+ AMD64 fixture used only as input data.
// No entry point or other code in this fixture is executed.
fn fixture() -> Vec<u8> {
    let mut bytes = vec![0u8; INPUT_SIZE];

    bytes[0..2].copy_from_slice(b"MZ");
    put32(&mut bytes, 0x3c, 0x80);
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");

    let coff = 0x84;
    put16(&mut bytes, coff, 0x8664);
    put16(&mut bytes, coff + 2, 1);
    put16(&mut bytes, coff + 16, 0xf0);

    let optional = coff + 20;
    put16(&mut bytes, optional, 0x20b);
    put32(&mut bytes, optional + 16, 0x1000);
    put64(&mut bytes, optional + 24, IMAGE_BASE);
    put32(&mut bytes, optional + 32, 0x1000);
    put32(&mut bytes, optional + 36, 0x200);
    put32(&mut bytes, optional + 56, IMAGE_SIZE as u32);
    put32(&mut bytes, optional + 60, 0x200);
    put16(&mut bytes, optional + 68, 3);
    put32(&mut bytes, optional + 108, 16);

    let section = optional + 0xf0;
    bytes[section..section + 5].copy_from_slice(b".data");
    put32(&mut bytes, section + 8, 0x300);
    put32(&mut bytes, section + 12, 0x1000);
    put32(&mut bytes, section + 16, 0x200);
    put32(&mut bytes, section + 20, 0x200);
    put32(&mut bytes, section + 36, 0x40000040);

    bytes[0x200..0x400].fill(0x5a);
    bytes
}

#[derive(Default)]
struct CountingAllocator {
    calls: usize,
    requested_bytes: u64,
}

impl ImageAllocator for CountingAllocator {
    fn allocate(&mut self, size: usize) -> Result<Vec<u8>, ImageAllocationError> {
        self.calls += 1;
        self.requested_bytes = self
            .requested_bytes
            .saturating_add(u64::try_from(size).unwrap_or(u64::MAX));

        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .map_err(|_| ImageAllocationError::AllocationFailed)?;
        bytes.resize(size, 0);
        Ok(bytes)
    }
}

fn iterations(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
        .clamp(1, 1_000_000)
}

fn measure<F>(label: &str, count: u64, mut operation: F) -> Duration
where
    F: FnMut(),
{
    let started = Instant::now();

    for _ in 0..count {
        operation();
    }

    let elapsed = started.elapsed();
    let average_ns = elapsed.as_nanos() / u128::from(count);

    println!(
        "{label}: iterations={count}, total_ms={:.3}, ns_per_iteration={average_ns}",
        elapsed.as_secs_f64() * 1000.0
    );

    elapsed
}

fn main() {
    let parse_count = iterations("NEXA_BENCH_PARSE_ITERS", 1_000);
    let layout_count = iterations("NEXA_BENCH_LAYOUT_ITERS", 1_000);
    let buffer_count = iterations("NEXA_BENCH_BUFFER_ITERS", 200);
    let prepare_count = iterations("NEXA_BENCH_PREPARE_ITERS", 200);

    let input = fixture();

    // Validate the fixture before timing any phase.
    let report = inspect(&input).expect("benchmark fixture must parse");
    assert_eq!(report.format, "PE32+");
    assert_eq!(report.machine_raw, 0x8664);
    assert_eq!(report.sections.len(), 1);
    assert_eq!(report.image_base, IMAGE_BASE);

    let limits = ImageLayoutLimits {
        max_image_size: IMAGE_SIZE,
        max_sections: 96,
    };

    let layout = plan_image_layout(&report, input.len() as u64, limits)
        .expect("benchmark fixture must have a valid image layout");

    assert_eq!(layout.image_size, IMAGE_SIZE);
    assert_eq!(layout.total_copy_bytes, 0x400);
    assert_eq!(layout.total_zero_fill_bytes, 0x100);

    let catalogue = ExportCatalogue::new(Vec::new(), ExportCatalogueLimits::default())
        .expect("empty export catalogue must be valid");

    let request = || ImagePreparationRequest {
        input: &input,
        requested_image_base: IMAGE_BASE,
        layout_limits: limits,
        max_image_size: IMAGE_SIZE,
        export_catalogue: &catalogue,
        import_limits: ImportResolutionLimits::default(),
        iat_limits: IatPatchPlanLimits::default(),
        relocation_limits: RelocationPlanLimits::default(),
    };

    println!("NexaOS NCE-005b end-to-end preparation benchmark");
    println!("fixture_kind: deterministic synthetic PE32+ AMD64");
    println!("execution: NOT_ATTEMPTED");
    println!("input_bytes: {}", input.len());
    println!("image_bytes: {}", layout.image_size);
    println!("sections: {}", report.sections.len());
    println!("copy_bytes: {}", layout.total_copy_bytes);
    println!("zero_fill_bytes: {}", layout.total_zero_fill_bytes);
    println!("imports: {}", report.import_metadata.len());
    println!("relocation_entries: {}", report.relocation_entries);

    measure("parse_inspect", parse_count, || {
        black_box(inspect(black_box(&input)).expect("fixture remains valid"));
    });

    measure("layout_planning", layout_count, || {
        black_box(
            plan_image_layout(black_box(&report), input.len() as u64, limits)
                .expect("fixture layout remains valid"),
        );
    });

    let mut allocation_counter = CountingAllocator::default();
    measure("image_buffer_allocation", buffer_count, || {
        black_box(
            allocate_image_buffer(&layout, IMAGE_SIZE, &mut allocation_counter)
                .expect("bounded image allocation must succeed"),
        );
    });

    println!(
        "allocation_phase: calls={}, requested_bytes={}",
        allocation_counter.calls, allocation_counter.requested_bytes
    );

    let mut combined_allocator = CountingAllocator::default();
    measure("image_allocation_and_population", buffer_count, || {
        let image = allocate_image_buffer(&layout, IMAGE_SIZE, &mut combined_allocator)
            .expect("bounded image allocation must succeed");

        black_box(
            populate_image_buffer(black_box(&layout), black_box(&input), image)
                .expect("fixture population must succeed"),
        );
    });

    println!(
        "allocation_and_population_allocator_calls: {}",
        combined_allocator.calls
    );
    println!(
        "allocation_and_population_allocator_requested_bytes: {}",
        combined_allocator.requested_bytes
    );

    // Allocate each destination before starting its timer. This isolates the
    // population operation for the same fixture used by the end-to-end phases.
    // A fresh zeroed buffer is prepared between samples, outside the timed span.
    let mut population_only_elapsed = Duration::ZERO;
    let mut population_allocator = CountingAllocator::default();
    for _ in 0..buffer_count {
        let image = allocate_image_buffer(&layout, IMAGE_SIZE, &mut population_allocator)
            .expect("bounded image allocation must succeed");
        let started = Instant::now();
        black_box(
            populate_image_buffer(black_box(&layout), black_box(&input), image)
                .expect("fixture population must succeed"),
        );
        population_only_elapsed += started.elapsed();
    }
    let population_average_ns =
        population_only_elapsed.as_nanos() / u128::from(buffer_count);
    println!(
        "population_only: iterations={buffer_count}, total_ms={:.3}, ns_per_iteration={population_average_ns}",
        population_only_elapsed.as_secs_f64() * 1000.0
    );
    println!(
        "population_only_setup_allocator_calls: {}",
        population_allocator.calls
    );
    println!(
        "population_only_setup_allocator_requested_bytes: {}",
        population_allocator.requested_bytes
    );

    let mut preparation_allocator = CountingAllocator::default();
    measure(
        "prepare_image_end_to_end_counting_allocator",
        prepare_count,
        || {
            let prepared = prepare_image(request(), &mut preparation_allocator)
                .expect("complete bounded preparation must succeed");

            assert_eq!(prepared.image_size(), IMAGE_SIZE);
            assert_eq!(prepared.requested_image_base(), IMAGE_BASE);
            black_box(prepared.image_bytes());
        },
    );

    println!(
        "prepare_counting_allocator_calls: {}",
        preparation_allocator.calls
    );
    println!(
        "prepare_counting_allocator_requested_bytes: {}",
        preparation_allocator.requested_bytes
    );

    let mut system_allocator = SystemImageAllocator;
    measure(
        "prepare_image_end_to_end_system_allocator",
        prepare_count,
        || {
            let prepared = prepare_image(request(), &mut system_allocator)
                .expect("complete bounded preparation with system allocator must succeed");

            assert_eq!(prepared.image_size(), IMAGE_SIZE);
            assert_eq!(prepared.requested_image_base(), IMAGE_BASE);
            black_box(prepared.image_bytes());
        },
    );

    println!("measurement_scope: wall-clock elapsed time and logical image allocator calls");
    println!("not_measured: process-wide heap allocations, peak RSS, cold-start effects, loader compatibility");
    println!("fixture_limit: no imports or relocations; this is a baseline, not real-world corpus coverage");

    // Keep the process alive for an external working-set sampler if requested.
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
