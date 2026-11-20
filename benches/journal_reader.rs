//! Benchmarks for the journal vector reader and the streaming visitor.
//!
//! Run with `cargo bench --bench journal_reader`. The harness is `std`-only
//! (`harness = false`), so this works on stable Rust and prints timings that
//! can be compared across commits. Both workloads read the same checked-in
//! v2 golden run through the shared parser.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    reason = "benchmarks drive known fixtures and report timings on stdout"
)]
#![allow(
    clippy::allow_attributes,
    reason = "benchmark crates relax the production lint set wholesale"
)]

use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use fieldbook::journal::JournalEntry;
use fieldbook::journal::read_observations;
use fieldbook::journal::visit_observations;

const ITERATIONS: usize = 200;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/journal-v2-golden/golden")
}

fn vector_reader(directory: &Path) -> usize {
    let mut total = 0;
    for _ in 0..ITERATIONS {
        total += read_observations(directory)
            .expect("golden run must read")
            .len();
    }
    total
}

fn streaming_visitor(directory: &Path) -> usize {
    let mut total = 0;
    for _ in 0..ITERATIONS {
        let summary = visit_observations(directory, &mut |_entry: &JournalEntry<'_>| Ok(()))
            .expect("golden run must stream");
        total += summary.records as usize;
    }
    total
}

fn report(name: &str, rows: usize, started: Instant) {
    let elapsed = started.elapsed();
    let per_iteration = elapsed.as_nanos() / ITERATIONS as u128;
    println!(
        "{name}: {ITERATIONS} iterations, {rows} rows, total {:?}, per iteration {} ns",
        elapsed, per_iteration
    );
}

fn main() {
    let directory = fixture();
    let started = Instant::now();
    let rows = vector_reader(&directory);
    report("read_observations", rows, started);
    let started = Instant::now();
    let rows = streaming_visitor(&directory);
    report("visit_observations", rows, started);
}
