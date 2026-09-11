//! Bundled changelog: CHANGELOG.md is compiled into the binary, so the
//! What's-new overlay needs no network. The parser is a pure function
//! over the `## [version]` headers.

use crate::config::discovery;
use std::path::{Path, PathBuf};

/// The repository's CHANGELOG.md, baked in at compile time.
pub fn bundled() -> &'static str {
    include_str!("../CHANGELOG.md")
}

/// One `## [version] — date` section of the changelog.
pub struct Section {
    pub version: String,
    pub date: String,
    /// The section body verbatim — headings, bullets, blank lines. The
    /// renderer keeps it plain; there is no markdown engine in Slint.
    pub body: String,
}

/// Split on `## [` headers. `[Unreleased]` and anything unparseable is
/// skipped; sections appear in file order (newest first by convention).
pub fn parse(text: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("## [") {
            let Some((version, tail)) = rest.split_once(']') else {
                continue;
            };
            let date = tail.trim_start_matches(['—', '-', ' ']).trim().to_owned();
            sections.push(Section {
                version: version.trim().to_owned(),
                date,
                body: String::new(),
            });
        } else if let Some(section) = sections.last_mut()
            && section.version != "Unreleased"
            && (!line.is_empty() || !section.body.is_empty())
        {
            section.body.push_str(line);
            section.body.push('\n');
        }
    }
    sections.retain(|section| !section.version.is_empty() && section.version != "Unreleased");
    for section in &mut sections {
        section.body = section.body.trim().to_owned();
    }
    sections
}

/// The section for `version`; a dev build without its own section yet
/// falls back to the newest one so the overlay is never empty-handed.
pub fn for_version<'a>(sections: &'a [Section], version: &str) -> Option<&'a Section> {
    sections
        .iter()
        .find(|section| section.version == version)
        .or_else(|| sections.first())
}

/// Every section as display text, newest first: a header line per
/// version, then its body.
pub fn full_text(sections: &[Section]) -> String {
    let mut out = String::new();
    for section in sections {
        out.push_str(&format!(
            "Version {}{}\n{}\n\n",
            section.version,
            if section.date.is_empty() {
                String::new()
            } else {
                format!(" — {}", section.date)
            },
            section.body
        ));
    }
    out.trim().to_owned()
}

fn stamp_path(env: &discovery::Env) -> PathBuf {
    let base = if let Some(state_home) = env.xdg_state_home.as_deref() {
        PathBuf::from(state_home)
    } else {
        let home = env
            .home
            .as_deref()
            .unwrap_or_else(|| std::ffi::OsStr::new(""));
        Path::new(home).join(".local/state")
    };
    base.join("umbriel-config/last-whatsnew-version")
}

/// The overlay shows once per version: again only when the running
/// version differs from the stamp.
pub fn should_show(env: &discovery::Env, running: &str) -> bool {
    std::fs::read_to_string(stamp_path(env))
        .map(|stamped| stamped.trim() != running)
        .unwrap_or(true)
}

pub fn mark_shown(env: &discovery::Env, running: &str) {
    let path = stamp_path(env);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, format!("{running}\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "\
## [Unreleased]

### Changed
- something in the works

## [0.2.0] — 2026-09-20

### Features
- backups

## [0.1.0-alpha.1]

### Features
- first release
";

    #[test]
    fn parse_splits_sections_and_skips_unreleased() {
        let sections = parse(FIXTURE);
        let versions: Vec<&str> = sections.iter().map(|s| s.version.as_str()).collect();
        assert_eq!(versions, vec!["0.2.0", "0.1.0-alpha.1"]);
        assert_eq!(sections[0].date, "2026-09-20");
        assert_eq!(sections[1].date, "");
        assert!(sections[0].body.contains("### Features"));
        assert!(sections[0].body.contains("- backups"));
        assert!(!sections[0].body.contains("Unreleased"));
    }

    #[test]
    fn parse_handles_empty_and_headerless_text() {
        assert!(parse("").is_empty());
        assert!(parse("# Title\n\nplain text\n").is_empty());
    }

    #[test]
    fn for_version_matches_or_falls_back_to_newest() {
        let sections = parse(FIXTURE);
        assert_eq!(for_version(&sections, "0.2.0").unwrap().version, "0.2.0");
        assert_eq!(
            for_version(&sections, "0.3.0-dev").unwrap().version,
            "0.2.0"
        );
    }

    #[test]
    fn full_text_lists_every_section_with_headers() {
        let text = full_text(&parse(FIXTURE));
        assert!(text.contains("Version 0.2.0 — 2026-09-20"));
        assert!(text.contains("Version 0.1.0-alpha.1\n"));
        assert!(text.contains("- backups"));
        assert!(!text.contains("Unreleased"));
    }

    #[test]
    fn whatsnew_stamp_suppresses_until_the_version_changes() {
        let env = discovery::Env {
            xdg_config_home: None,
            xdg_config_dirs: None,
            xdg_data_dirs: None,
            xdg_state_home: Some(
                std::env::temp_dir()
                    .join(format!("umbriel-whatsnew-test-{}", std::process::id()))
                    .into_os_string(),
            ),
            home: None,
        };
        assert!(should_show(&env, "0.1.0"));
        mark_shown(&env, "0.1.0");
        assert!(!should_show(&env, "0.1.0"));
        assert!(should_show(&env, "0.2.0"));
        std::fs::remove_dir_all(env.xdg_state_home.unwrap()).ok();
    }
}
