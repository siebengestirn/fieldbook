use std::{
    path::Path,
    process::{Command, ExitStatus},
    time::Instant,
};

#[derive(Debug, Clone)]
pub struct Cmd {
    pub line: String,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub command: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    pub output_tail: String,
}

impl Cmd {
    pub fn new(line: &str) -> Self {
        Self {
            line: line.to_owned(),
            env: Vec::new(),
        }
    }

    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.push((key.to_owned(), value.to_owned()));
        self
    }

    pub fn execute(&self, root: &Path) -> Result<Outcome, String> {
        let mut parts = self.line.split_whitespace();
        let program = parts.next().ok_or_else(|| "empty command".to_owned())?;
        let args: Vec<&str> = parts.collect();
        let started = Instant::now();
        let output = Command::new(program)
            .args(&args)
            .envs(self.env.iter().map(|(key, value)| (key, value)))
            .current_dir(root)
            .output()
            .map_err(|error| format!("running `{program}`: {error}"))?;
        let duration_ms = started.elapsed().as_millis();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail = format!("{stdout}{stderr}");
        // Retain enough output to name a failing test and its assertion.
        let tail = tail
            .lines()
            .rev()
            .take(120)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        Ok(Outcome {
            command: self.line.clone(),
            status: status_of(output.status),
            exit_code: output.status.code(),
            duration_ms,
            output_tail: tail,
        })
    }
}

fn status_of(status: ExitStatus) -> String {
    if status.success() {
        "passed".to_owned()
    } else {
        "failed".to_owned()
    }
}
