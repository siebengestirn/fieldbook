# Compile-time optimisation evidence

This record reports measured clean-build cost on this checkout. It is a local
engineering measurement, not a certificate of throughput on other hosts.
All measurements used an isolated target directory inside this repository.

## Baseline

`cargo build --locked --timings` on the unmodified tree: **66 s** wall time,
356 compilation units, 360.3 s summed unit time. The timing report showed two
`ergo-sbe`-driven costs: the generator crate itself (13.4 s) and a second
`syn` toolchain (about 19 s) present only to build it.

## Changes

1. **Generated SBE codec is checked in.** The 7,391-line module lives at
   `src/generated/challenge_orderbook.rs` and is included directly by
   `src/util/dtos.rs`. Generation moved to `tools/sbe-gen`, a standalone crate
   excluded from the workspace. The main build script now only tracks the two
   commit environment variables used by `option_env!`. No build-dependency
   remains in the workspace graph.
2. **Reproducibility is enforced.** `scripts/check_sbe_generated.py` regenerates
   the codec from the schema into a repository-local scratch path and compares
   bytes. `cargo xtask ci` runs it; `scripts/check_compile_policy.py` runs in
   `cargo xtask verify` and rejects any reintroduction of build-dependencies,
   unused features or `OUT_DIR` inclusion. The Rust style checker excludes
   `src/generated/` because those suppressions are generator output, not
   hand-written style.
3. **Dependency features pruned.** `chrono` no longer requests its default
   clock/timezone stack: wall-clock reads come from `std::time::SystemTime`.
   `clap` no longer requests the unused `env` feature. The unused direct
   `signature` dependency was removed; RSA signing uses `rsa::signature`.
4. **Measurement tooling added.** `scripts/measure_compile.py` runs a clean
   timed build and records the slowest units under `.local/perf`, which is now
   gitignored together with all other local scratch output. `cargo xtask perf`
   exposes the same measurement.

## Results

| Configuration | Wall time | Units | Summed unit time |
|---|---:|---:|---:|
| Baseline | 66 s | 356 | 360.3 s |
| Checked-in codec | 62 s | 295 | 351.2 s |
| Codec plus feature pruning | **53 s** | 293 | 261.5 s |

Wall time fell by 20%; summed compilation work fell by 27%. The remaining
largest units are the application crate, `tokio`, `rustls`, `syn` and
`zerocopy` (the last is required by the RSA dependency chain).

## Rejected experiments

These were measured and deliberately **not** committed:

| Experiment | Result | Decision |
|---|---|---|
| `codegen-units = 16` | 86 s (vs 62 s reference) | rejected: slower |
| `-C link-arg=-fuse-ld=lld` | build failed; Apple clang rejects the flag | rejected: unsupported |
| `CARGO_INCREMENTAL = 0` | 80 s (vs 53 s reference) | rejected: slower |

Existing debug settings were retained: line tables for workspace code, no
debug information for dependencies. Thin LTO remains in the release profile.

## Reproduce

```sh
python3 scripts/measure_compile.py --label baseline-check
python3 scripts/check_sbe_generated.py
python3 scripts/check_compile_policy.py
cargo xtask ci
```

## Boundaries

No runtime behaviour, feature surface or safety gate was removed. The
generated codec is byte-identical to the previous build output (SHA-256
`6eab1a9de5826bd5372b396330f190213fa9ed0bfbe758dee92f21f9784e15ea`).
Measurements describe this host and toolchain only.
