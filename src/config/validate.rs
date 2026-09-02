//! Post-save validation through the compositor's own checker.
//!
//! Wraps `umbriel validate -c <path>`: exit 0 means accepted (warnings
//! allowed); `error: ` / `warning: ` lines on stderr become diagnostics
//! (compositor: `src/main.cpp`, `validateConfig`).

use std::path::Path;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum ValidateError {
    #[error("`umbriel` not found on PATH: {source}")]
    Launch { source: std::io::Error },
}

/// One entry from `umbriel validate -c <path>`'s stderr, e.g. `error: ...` or `warning: ...`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Diagnostic {
    Error(String),
    Warning(String),
}

impl Diagnostic {
    pub fn message(&self) -> &str {
        match self {
            Diagnostic::Error(msg) => msg,
            Diagnostic::Warning(msg) => msg,
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Diagnostic::Error(_))
    }
}

#[derive(Debug, Clone)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    /// Accepted by umbriel: no errors (warnings allowed), mirroring its
    /// warnings-only exit-success behavior.
    pub fn is_ok(&self) -> bool {
        !self.diagnostics.iter().any(Diagnostic::is_error)
    }
}

/// Run `umbriel validate -c <path>` and capture its output.
pub fn validate(path: &Path) -> Result<Report, ValidateError> {
    let output = Command::new("umbriel")
        .arg("validate")
        .arg("-c")
        .arg(path)
        .output()
        .map_err(|source| ValidateError::Launch { source })?;
    Ok(parse(
        &String::from_utf8_lossy(&output.stderr),
        output.status.success(),
    ))
}

// Turn `error: ` / `warning: ` stderr lines into diagnostics; other lines
// are noise and dropped. A failing run that reported no errors is treated
// as an error so crashes cannot read as "config ok".
fn parse(stderr: &str, exited_cleanly: bool) -> Report {
    let mut diagnostics: Vec<_> = stderr
        .lines()
        .filter_map(|line| {
            if let Some(message) = line.strip_prefix("error: ") {
                Some(Diagnostic::Error(message.to_owned()))
            } else {
                line.strip_prefix("warning: ")
                    .map(|message| Diagnostic::Warning(message.to_owned()))
            }
        })
        .collect();
    if !exited_cleanly && !diagnostics.iter().any(Diagnostic::is_error) {
        diagnostics.push(Diagnostic::Error(
            "umbriel validate exited unsuccessfully without reporting diagnostics".to_owned(),
        ));
    }
    Report { diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_error_and_warning_lines_and_drops_noise() {
        let report = parse(
            "some unrelated line\n\
             error: config.toml:12:3: unknown key 'nope'\n\
             warning: config.toml:40:1: value out of range\n",
            true,
        );
        assert_eq!(
            report.diagnostics,
            vec![
                Diagnostic::Error("config.toml:12:3: unknown key 'nope'".into()),
                Diagnostic::Warning("config.toml:40:1: value out of range".into()),
            ]
        );
        assert!(!report.is_ok());
    }

    #[test]
    fn empty_stderr_is_clean() {
        let report = parse("", true);
        assert!(report.diagnostics.is_empty());
        assert!(report.is_ok());
    }

    #[test]
    fn warnings_only_is_ok() {
        let report = parse("warning: something mild\n", true);
        assert!(report.is_ok());
        assert_eq!(report.diagnostics.len(), 1);
    }

    #[test]
    fn failed_run_without_diagnostics_is_an_error() {
        let report = parse("", false);
        assert!(!report.is_ok());
        assert_eq!(report.diagnostics.len(), 1);
    }
}
