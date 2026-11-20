//! Build-time dependency tracking only.
//!
//! The SBE codec is checked in under `src/generated` and regenerated on demand
//! by `tools/sbe-gen`; generation is not part of every build. This build script
//! exists solely so commits baked into the binary via `option_env!` are
//! refreshed when the corresponding environment changes.

fn main() {
    println!("cargo::rerun-if-env-changed=GIT_COMMIT");
    println!("cargo::rerun-if-env-changed=GITHUB_SHA");
}
