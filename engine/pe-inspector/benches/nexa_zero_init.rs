use nexa_inspect::{ImageAllocationError, ImageAllocator, SystemImageAllocator};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

const DEFAULT_IMAGE_BYTES: usize = 0x2000;
const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;

fn setting(name: &str, default: usize, max: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
        .clamp(1, max)
}

fn summarize(label: &str, count: usize, elapsed: Duration) {
    let average_ns = elapsed.as_nanos() / count as u128;
    println!(
        "{label}: iterations={count}, total_ms={:.3}, ns_per_iteration={average_ns}",
        elapsed.as_secs_f64() * 1000.0
    );
}

fn main() -> Result<(), ImageAllocationError> {
    let count = setting("NEXA_ZERO_INIT_ITERS", 2_000, 1_000_000);
    let image_bytes = setting("NEXA_ZERO_INIT_BYTES", DEFAULT_IMAGE_BYTES, MAX_IMAGE_BYTES);

    println!("NexaOS duplicate-zero-initialization diagnostic");
    println!("fixture_kind: fresh initialized process-heap Vec<u8>");
    println!("execution: NOT_ATTEMPTED");
    println!("image_bytes: {image_bytes}");
    println!("iterations: {count}");
    println!("measurement_scope: allocator initialization and isolated subsequent fill");
    println!("not_measured: process-wide heap allocations, peak RSS, loader behavior, application startup");

    let mut allocator = SystemImageAllocator;

    // This phase includes the allocator's own zero initialization.
    let mut allocator_elapsed = Duration::ZERO;
    for _ in 0..count {
        let started = Instant::now();
        let bytes = allocator.allocate(image_bytes)?;
        allocator_elapsed += started.elapsed();
        black_box(bytes);
    }
    summarize("allocator_allocate_initialized", count, allocator_elapsed);

    // Each vector is freshly allocated and initialized before timing. Only the
    // additional fill that allocate_image_buffer currently performs is timed.
    let mut second_clear_elapsed = Duration::ZERO;
    for _ in 0..count {
        let mut bytes = allocator.allocate(image_bytes)?;
        let started = Instant::now();
        bytes.fill(0);
        second_clear_elapsed += started.elapsed();
        black_box(bytes);
    }
    summarize("isolated_second_clear", count, second_clear_elapsed);

    println!("allocator_calls_for_each_phase: {count}");
    println!("allocator_requested_bytes_for_each_phase: {}", image_bytes.saturating_mul(count));
    println!("interpretation: isolated_second_clear is a diagnostic of the extra fill only; it is not an end-to-end speedup estimate");
    Ok(())
}
