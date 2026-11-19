#![expect(
    clippy::print_stdout,
    reason = "a task runner reports progress on stdout by definition"
)]

mod command;
mod e2e;
mod evidence;
mod report;

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use command::Cmd;
use report::Report;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = root();
    let result = dispatch(&root, &args);
    match result {
        Ok(report) => {
            let path = report::write(&root, &report);
            match path {
                Ok(path) => println!("report: {}", path.display()),
                Err(error) => eprintln!("report write failed: {error}"),
            }
            if report.failed.is_empty() {
                ExitCode::SUCCESS
            } else {
                eprintln!("failed steps: {}", report.failed.join(", "));
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("xtask error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Some(parent) = manifest.parent() else {
        eprintln!("xtask manifest has no parent directory");
        return PathBuf::from(".");
    };
    parent.to_path_buf()
}

fn dispatch(root: &Path, args: &[String]) -> Result<Report, String> {
    let mut report = Report::new(args);
    let command = args.first().map(String::as_str).unwrap_or("help");
    match command {
        "fmt" => {
            run(&mut report, root, &[Cmd::new("cargo fmt --all -- --check")])?;
            Ok(report)
        }
        "lint" => {
            run(
                &mut report,
                root,
                &[Cmd::new(
                    "cargo clippy --workspace --all-targets --locked -- -D warnings",
                )],
            )?;
            Ok(report)
        }
        "test" => {
            run(
                &mut report,
                root,
                &[
                    Cmd::new("cargo test --workspace --all-targets --locked"),
                    Cmd::new("cargo test --workspace --doc --locked"),
                ],
            )?;
            Ok(report)
        }
        "docs" => {
            run(
                &mut report,
                root,
                &[
                    Cmd::new("cargo doc --no-deps --workspace --locked").with_env(
                        "RUSTDOCFLAGS",
                        "-D warnings -D rustdoc::broken_intra_doc_links",
                    ),
                ],
            )?;
            Ok(report)
        }
        "style" => {
            run(&mut report, root, &style_commands())?;
            Ok(report)
        }
        "style-fix" => {
            run(
                &mut report,
                root,
                &[
                    Cmd::new("python3 -m pyink --config pyproject.toml scripts"),
                    Cmd::new("python3 scripts/check_style.py --fix"),
                ],
            )?;
            Ok(report)
        }
        "e2e" => {
            e2e::run(&mut report, root, &args[1..])?;
            Ok(report)
        }
        "verify" => {
            evidence::checks(&mut report, root)?;
            Ok(report)
        }
        "perf" => {
            let label = args.get(1).map(String::as_str).unwrap_or("local");
            let line = format!("python3 scripts/measure_compile.py --label {label}");
            run(&mut report, root, &[Cmd::new(&line)])?;
            Ok(report)
        }
        "fixtures" => {
            evidence::verify(&mut report, root)?;
            Ok(report)
        }
        "assure" => {
            evidence::assure(&mut report, root)?;
            Ok(report)
        }
        "check" => {
            run(&mut report, root, &style_commands())?;
            run(
                &mut report,
                root,
                &[
                    Cmd::new("cargo fmt --all -- --check"),
                    Cmd::new("cargo clippy --workspace --all-targets --locked -- -D warnings"),
                    Cmd::new("cargo test --workspace --all-targets --locked"),
                    Cmd::new("cargo test --workspace --doc --locked"),
                ],
            )?;
            evidence::checks(&mut report, root)?;
            e2e::run(&mut report, root, &[])?;
            Ok(report)
        }
        "ci" => {
            run(&mut report, root, &style_commands())?;
            run(
                &mut report,
                root,
                &[
                    Cmd::new("cargo fmt --all -- --check"),
                    Cmd::new("cargo clippy --workspace --all-targets --locked -- -D warnings"),
                    Cmd::new("cargo test --workspace --all-targets --locked"),
                    Cmd::new("cargo test --workspace --doc --locked"),
                    Cmd::new("cargo build --release --locked"),
                    Cmd::new("python3 scripts/check_sbe_generated.py"),
                    Cmd::new("cargo doc --no-deps --workspace --locked").with_env(
                        "RUSTDOCFLAGS",
                        "-D warnings -D rustdoc::broken_intra_doc_links",
                    ),
                ],
            )?;
            evidence::checks(&mut report, root)?;
            e2e::run(&mut report, root, &[])?;
            Ok(report)
        }
        "help" | "--help" | "-h" => {
            println!("{HELP}");
            Ok(report)
        }
        other => Err(format!("unknown command `{other}`; run `cargo xtask help`")),
    }
}

/// Every non-Rust language gate, in one place, so `style`, `check` and `ci`
/// cannot drift apart. Tool versions are pinned in requirements-dev.txt.
fn style_commands() -> Vec<Cmd> {
    vec![
        Cmd::new("python3 scripts/check_style.py"),
        Cmd::new("python3 -m ruff check --no-cache ."),
        Cmd::new("python3 -m pyink --check --config pyproject.toml scripts"),
        Cmd::new("python3 scripts/check_shellcheck.py"),
        Cmd::new("python3 scripts/check_no_cross_pollution.py"),
        Cmd::new("python3 scripts/check_rust_style.py --self-test"),
    ]
}

fn run(report: &mut Report, root: &Path, commands: &[Cmd]) -> Result<(), String> {
    for command in commands {
        let outcome = command.execute(root)?;
        let failed = outcome.status == "failed";
        println!("[{}] {}", outcome.status, outcome.command);
        if failed {
            // Keep the failure cause in CI logs, not only in the JSON report.
            eprintln!("--- failed step output ---\n{}", outcome.output_tail);
        }
        report.push(outcome);
        if failed {
            return Ok(());
        }
    }
    Ok(())
}

const HELP: &str = "\
xtask - repository tasks

Usage: cargo xtask <command> [args]

Commands:
  style     Python, shell, Makefile, JSON and Markdown style gates
  style-fix apply the safe Python and Markdown style fixes
  fmt       check formatting
  lint      clippy with warnings denied
  test      all-target and doc tests
  docs      rustdoc with warnings denied and broken links rejected
  e2e       run the curated e2e suites; --list-ignored lists opt-in tests
  verify    matrix, architecture and generated-path gates
  fixtures  verify checked-in journal fixtures
  assure    run the assurance scripts
  check     fmt, lint, tests, checks, e2e
  ci        check plus release build (the full gate)
  help      show this message
";
