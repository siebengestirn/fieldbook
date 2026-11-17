use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::command::Outcome;

#[derive(Debug)]
pub struct Report {
    pub invocation: Vec<String>,
    pub finished_at_ms: u128,
    pub steps: Vec<Outcome>,
    pub failed: Vec<String>,
}

impl Report {
    pub fn new(invocation: &[String]) -> Self {
        let finished_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());
        Self {
            invocation: invocation.to_vec(),
            finished_at_ms,
            steps: Vec::new(),
            failed: Vec::new(),
        }
    }

    pub fn push(&mut self, outcome: Outcome) {
        if outcome.status == "failed" {
            self.failed.push(outcome.command.clone());
        }
        self.steps.push(outcome);
    }
}

pub fn write(root: &Path, report: &Report) -> Result<PathBuf, String> {
    let dir = root.join("target/xtask");
    fs::create_dir_all(&dir).map_err(|error| format!("creating {}: {error}", dir.display()))?;
    let path = dir.join("last-run.json");
    let mut out = String::from("{\n  \"finished_at_ms\": ");
    out.push_str(&report.finished_at_ms.to_string());
    out.push_str(",\n  \"invocation\": [");
    for (index, arg) in report.invocation.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('\n');
        out.push_str("    ");
        out.push_str(&quote(arg));
    }
    out.push_str("\n  ],\n  \"steps\": [");
    for (index, step) in report.steps.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str("\n    {");
        out.push_str("\n      \"command\": ");
        out.push_str(&quote(&step.command));
        out.push_str(",\n      \"status\": ");
        out.push_str(&quote(&step.status));
        out.push_str(",\n      \"exit_code\": ");
        out.push_str(
            &step
                .exit_code
                .map_or_else(|| "null".to_owned(), |code| code.to_string()),
        );
        out.push_str(",\n      \"duration_ms\": ");
        out.push_str(&step.duration_ms.to_string());
        out.push_str(",\n      \"output_tail\": ");
        out.push_str(&quote(&step.output_tail));
        out.push_str("\n    }");
    }
    out.push_str("\n  ],\n  \"failed\": [");
    for (index, failed) in report.failed.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('\n');
        out.push_str("    ");
        out.push_str(&quote(failed));
    }
    out.push_str("\n  ]\n}\n");
    fs::write(&path, out).map_err(|error| format!("writing {}: {error}", path.display()))?;
    Ok(path)
}

fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if control.is_control() => {
                out.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_escapes_json_controls() {
        assert_eq!(quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(quote("line\nnext"), "\"line\\nnext\"");
    }

    #[test]
    fn report_records_failures() {
        let mut report = Report::new(&["e2e".to_owned()]);
        report.push(Outcome {
            command: "cargo test".to_owned(),
            status: "failed".to_owned(),
            exit_code: Some(101),
            duration_ms: 1,
            output_tail: String::new(),
        });
        assert_eq!(report.failed, vec!["cargo test".to_owned()]);
    }
}
