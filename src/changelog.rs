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
    /// The section body verbatim — headings, bullets, blank lines;
    /// `blocks` splits it for the notes view.
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

/// The bundled fonts are subsets — Latin plus a little punctuation — so
/// emoji headings, arrows and geometric shapes draw as blanks. Map the
/// few that carry meaning and drop the rest before anything reaches the
/// overlay; a dropped character takes one following space with it, so
/// "### <emoji> Features" reads "### Features".
pub fn renderable(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut drop_space = false;
    for ch in text.chars() {
        if drop_space && ch == ' ' {
            drop_space = false;
            continue;
        }
        drop_space = false;
        match ch {
            '→' => out.push_str("->"),
            '←' => out.push_str("<-"),
            _ if in_bundled_fonts(ch) => out.push(ch),
            _ => drop_space = out.ends_with(' '),
        }
    }
    out
}

/// Latin-1 and Latin Extended plus the punctuation Inter and JetBrains
/// Mono actually ship in this build (checked against their cmaps).
fn in_bundled_fonts(ch: char) -> bool {
    ch.is_ascii()
        || matches!(
            ch,
            '\u{00a0}'..='\u{024f}' | '–' | '—' | '‘' | '’' | '“' | '”' | '…' | '•'
        )
}

/// What a line of release notes is, for the notes view. The order is
/// the `kind` numbering of the UI's NoteLine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlockKind {
    Heading,
    Scope,
    Bullet,
    SubBullet,
    Detail,
    Paragraph,
}

/// One line of release notes as the notes view draws it.
#[derive(Debug, PartialEq)]
pub struct Block {
    pub kind: BlockKind,
    /// The group's color: 0 neutral, 1 features, 2 fixes, 3 highlights,
    /// 4 removals.
    pub tone: i32,
    pub text: String,
    /// A canary entry's date and commit, split off its bullet.
    pub meta: String,
}

/// Split release notes into lines: `##`/`###` group headings, `####`
/// scopes, bullets and sub-bullets, a commit's body under its bullet,
/// and paragraphs. Wrapped lines join up; comments, code fences and
/// emphasis marks drop.
pub fn blocks(notes: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut tone = 0;
    // Whether the next plain line continues the last block: no blank
    // line in between.
    let mut open = false;
    for line in notes.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("<!--") || trimmed.starts_with("```") {
            open = false;
            continue;
        }
        let indented = line.starts_with(' ');
        let (kind, text) = if let Some(rest) = trimmed.strip_prefix('#') {
            let text = rest.trim_start_matches('#').trim();
            if rest.starts_with("###") {
                (BlockKind::Scope, text)
            } else {
                tone = tone_of(text);
                (BlockKind::Heading, text)
            }
        } else if let Some(text) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            let kind = if indented {
                BlockKind::SubBullet
            } else {
                BlockKind::Bullet
            };
            (kind, text)
        } else if open
            && let Some(last) = blocks.last_mut()
            // A commit's bullet is one line; what follows is its body.
            && !(last.kind == BlockKind::Bullet && !last.meta.is_empty())
            && !matches!(last.kind, BlockKind::Heading | BlockKind::Scope)
        {
            last.text.push(' ');
            last.text.push_str(&plain(trimmed));
            continue;
        } else if indented {
            (BlockKind::Detail, trimmed)
        } else {
            (BlockKind::Paragraph, trimmed)
        };
        let (text, meta) = match kind {
            BlockKind::Bullet => split_meta(text),
            _ => (text, String::new()),
        };
        blocks.push(Block {
            kind,
            tone,
            text: plain(text),
            meta,
        });
        open = true;
    }
    blocks
}

/// A group heading's color (see `Block::tone`).
fn tone_of(heading: &str) -> i32 {
    let heading = heading.to_lowercase();
    if heading.contains("feature") || heading.contains("added") {
        1
    } else if heading.contains("fix") {
        2
    } else if heading.contains("highlight") || heading.contains("coming") {
        3
    } else if heading.contains("remov") || heading.contains("break") {
        4
    } else {
        0
    }
}

/// Split a canary bullet's `· 2026-09-27 01:06 (a4264c8)` tail off as
/// its meta line.
fn split_meta(text: &str) -> (&str, String) {
    let commit = text
        .strip_suffix(')')
        .and_then(|head| head.rsplit_once(" ("))
        .filter(|(_, sha)| sha.len() >= 7 && sha.chars().all(|ch| ch.is_ascii_hexdigit()));
    match commit {
        Some((head, sha)) => match head.rsplit_once(" · ") {
            Some((title, date)) => (title, format!("{date} · {sha}")),
            None => (head, sha.to_owned()),
        },
        None => (text, String::new()),
    }
}

/// Markdown marks the view can't draw: emphasis and code marks drop, a
/// link keeps its text.
fn plain(text: &str) -> String {
    let text = text.replace("**", "").replace('`', "");
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(start) = rest.find('[') {
        let Some((label, tail)) = rest[start + 1..].split_once("](") else {
            break;
        };
        let Some(end) = tail.find(')') else { break };
        out.push_str(&rest[..start]);
        out.push_str(label);
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
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
    fn renderable_drops_what_the_fonts_cannot_draw() {
        // The emoji goes, and so does the space it left behind.
        assert_eq!(renderable("### 🚀 Features"), "### Features");
        assert_eq!(renderable("### 🐛 Fixed"), "### Fixed");
        // Arrows carry meaning, so they become ASCII instead.
        assert_eq!(renderable("Settings → Updates"), "Settings -> Updates");
        // Typography the fonts do have survives untouched, and so does
        // the indentation of a continuation line.
        assert_eq!(
            renderable("- (ui) “quoted” — em-dash…\n  indented detail"),
            "- (ui) “quoted” — em-dash…\n  indented detail"
        );
    }

    #[test]
    fn blocks_split_canary_notes() {
        let notes = "Untested build of `a4264c8` from dev.\n\
            \n\
            <!-- commits: a4264c8 -->\n\
            ### Fixed\n\
            \n\
            #### Schema\n\
            \n\
            - Read the docs · 2026-09-27 01:06 (a4264c8)\n\
            \x20 The bundled docs now cover\n\
            \x20 effect presets.\n\
            \n\
            \x20 - Animations: a Window\n\
            \x20   drag card.\n\
            \n\
            ### Features\n\
            - Plain **bold** bullet that\n\
            \x20 wraps, see [the docs](https://x).\n";
        let parsed = blocks(notes);
        let got: Vec<(BlockKind, i32, &str, &str)> = parsed
            .iter()
            .map(|block| {
                (
                    block.kind,
                    block.tone,
                    block.text.as_str(),
                    block.meta.as_str(),
                )
            })
            .collect();
        use BlockKind::*;
        assert_eq!(
            got,
            [
                (Paragraph, 0, "Untested build of a4264c8 from dev.", ""),
                (Heading, 2, "Fixed", ""),
                (Scope, 2, "Schema", ""),
                (Bullet, 2, "Read the docs", "2026-09-27 01:06 · a4264c8"),
                (Detail, 2, "The bundled docs now cover effect presets.", ""),
                (SubBullet, 2, "Animations: a Window drag card.", ""),
                (Heading, 1, "Features", ""),
                (Bullet, 1, "Plain bold bullet that wraps, see the docs.", ""),
            ]
        );
    }

    #[test]
    fn whatsnew_stamp_suppresses_until_the_version_changes() {
        let env = discovery::Env {
            xdg_config_home: None,
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
