//! umbriel's user docs, the source of the settings pages (see
//! [`super::schema::assemble_docs`]). A copy ships inside the app so the
//! pages work offline and on day one; a newer copy is downloaded from the
//! umbriel repository and kept in the state directory.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::discovery;

/// The docs as bundled at build time (`just refresh-umbriel-docs`).
pub const BUNDLED: &str = include_str!("../../assets/umbriel-docs.md");

/// Starts every page in the combined file.
const PAGE_MARKER: &str = "<!-- umbriel-config page: ";

const LISTING_URL: &str =
    "https://api.github.com/repos/noctalia-dev/umbriel/contents/docs/user?ref=main";
const USER_AGENT: &str = "umbriel-config";

/// Where the downloaded copy lives:
/// `$XDG_STATE_HOME/umbriel-config/umbriel-docs.md`.
pub fn cache_path(env: &discovery::Env) -> PathBuf {
    discovery::state_dir(env).join("umbriel-docs.md")
}

/// The newest docs available: the downloaded copy, else the bundled one.
pub fn load(env: &discovery::Env) -> String {
    std::fs::read_to_string(cache_path(env))
        .ok()
        .filter(|text| text.contains(PAGE_MARKER))
        .unwrap_or_else(|| BUNDLED.to_owned())
}

/// Whether the downloaded copy is missing or more than a day old.
pub fn is_stale(env: &discovery::Env) -> bool {
    let age = std::fs::metadata(cache_path(env))
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok());
    age.is_none_or(|age| age >= Duration::from_secs(24 * 60 * 60))
}

/// Download the docs and store them as the new copy. The old copy stays
/// when the download fails or holds no settings.
pub fn refresh(env: &discovery::Env) -> Result<(), String> {
    let docs = fetch()?;
    if super::schema::assemble_docs(&docs).is_empty() {
        return Err("the downloaded docs describe no settings".to_owned());
    }
    store(&cache_path(env), &docs).map_err(|err| err.to_string())
}

#[derive(serde::Deserialize)]
struct ListingItem {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    download_url: Option<String>,
}

/// Every page of `docs/user` on umbriel's main branch, combined the way
/// `just refresh-umbriel-docs` combines the bundled copy.
fn fetch() -> Result<String, String> {
    let mut pages: Vec<ListingItem> = get(LISTING_URL)?
        .read_json()
        .map_err(|err| err.to_string())?;
    pages.retain(|page| page.kind == "file" && page.name.ends_with(".md"));
    pages.sort_by(|a, b| a.name.cmp(&b.name));
    let mut docs = String::new();
    for page in pages {
        let Some(url) = page.download_url else {
            continue;
        };
        let text = get(&url)?
            .with_config()
            .limit(4 * 1024 * 1024)
            .read_to_string()
            .map_err(|err| err.to_string())?;
        docs += &format!("{PAGE_MARKER}{} -->\n{text}\n", page.name);
    }
    Ok(docs)
}

fn get(url: &str) -> Result<ureq::Body, String> {
    ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .config()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .call()
        .map(|response| response.into_body())
        .map_err(|err| err.to_string())
}

/// Write `text` to `path` through a temp file, so a reader never sees half.
fn store(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(state: &Path) -> discovery::Env {
        discovery::Env {
            xdg_state_home: Some(state.as_os_str().to_owned()),
            ..discovery::Env::default()
        }
    }

    #[test]
    fn load_prefers_a_downloaded_copy_and_falls_back_to_the_bundled_one() {
        let state = std::env::temp_dir().join(format!("umbriel-docs-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&state);
        let env = env(&state);
        assert_eq!(load(&env), BUNDLED);
        assert!(is_stale(&env));

        let downloaded = format!("{PAGE_MARKER}input.md -->\n# Input\n");
        store(&cache_path(&env), &downloaded).unwrap();
        assert_eq!(load(&env), downloaded);
        assert!(!is_stale(&env));

        // Something that isn't a docs file is ignored.
        store(&cache_path(&env), "<html>rate limited</html>").unwrap();
        assert_eq!(load(&env), BUNDLED);

        std::fs::remove_dir_all(&state).ok();
    }

    #[test]
    fn the_bundled_docs_describe_the_settings() {
        let entries = super::super::schema::assemble_docs(BUNDLED);
        let keys = super::super::schema::key_set(&entries);
        for key in [
            "input.keyboard.repeat_rate",
            "input.mouse.accel_profile",
            "input.touchpad.tap",
            "layout.gap",
            "animation.layers.duration_ms",
            "hot_corners.bottom_right.action",
        ] {
            assert!(keys.contains(key), "missing {key}");
        }
        // Examples in the docs aren't settings.
        assert!(!keys.iter().any(|key| key.starts_with("output.")));
        assert!(!keys.iter().any(|key| key.starts_with("keybinds.")));
        assert!(!keys.iter().any(|key| key.starts_with("animation.beziers")));
    }
}
