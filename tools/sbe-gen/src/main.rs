//! Regenerate the checked-in SBE codec from the schema sources.
//!
//! This tool is intentionally outside the workspace: generation is an explicit
//! maintenance operation, not a cost paid by every build. Run it from the
//! repository root:
//!
//! ```text
//! cargo run --locked --manifest-path tools/sbe-gen/Cargo.toml -- \
//!     --schema-root . --output src/generated
//! ```

#![expect(
    clippy::print_stdout,
    reason = "a generator reports its output path on stdout by definition"
)]

mod sbe_yaml;

use std::{
    fs, //
    path::PathBuf,
};

use ergo_sbe::{
    ConversionSelector, DomainVarData, GenerationConfig, generate_str_to_dir,
    miette::{IntoDiagnostic, Result, miette},
};

const SCHEMA: &str = "sbe_schemas/fieldbook/challenge_orderbook.yaml";
const COMMON_TYPES: &str = "sbe_schemas/includes/common-types.xml";
const MODULE: &str = "challenge_orderbook";

fn main() -> Result<()> {
    let mut schema_root = PathBuf::from(".");
    let mut output = PathBuf::from("src/generated");
    let mut arguments = std::env::args().skip(1);
    while let Some(flag) = arguments.next() {
        match flag.as_str() {
            "--schema-root" => {
                schema_root = PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| miette!("--schema-root needs a path"))?,
                )
            }
            "--output" => {
                output = PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| miette!("--output needs a path"))?,
                )
            }
            other => return Err(miette!("unknown argument `{other}`")),
        }
    }
    let yaml = fs::read_to_string(schema_root.join(SCHEMA))
        .into_diagnostic()
        .map_err(|error| miette!("schema read: {error}"))?;
    let schema = sbe_yaml::parse(&yaml).map_err(|error| miette!("schema parse: {error}"))?;
    let common_types = fs::read_to_string(schema_root.join(COMMON_TYPES))
        .into_diagnostic()
        .map_err(|error| miette!("common types read: {error}"))?;
    let xml = schema
        .to_xml("challenge_orderbook_messages", &common_types)
        .map_err(|error| miette!("to_xml: {error}"))?;
    let config = GenerationConfig::new(MODULE)
        .with_domain_objects(DomainVarData::Strings)
        .with_domain_type(
            ConversionSelector::named_type("UTCTimestampMicrosEncoding"),
            "chrono::DateTime<chrono::Utc>",
        )
        .with_domain_type(
            ConversionSelector::named_type("SideEncoding"),
            "crate::Side",
        );
    generate_str_to_dir(&xml, config, &output)
        .map_err(|error| miette!("generation failed: {error:?}"))?;
    println!(
        "generated {}",
        output.join(format!("{MODULE}.rs")).display()
    );
    Ok(())
}
