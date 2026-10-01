//! Suggestions for the word under the caret in the shader editor, and
//! the reference list of what umbriel gives a kind of shader. The
//! `umbriel_*` names and each kind's entry point come from [`ShaderApi`]
//! (umbriel's own docs), so a newer umbriel's additions are offered
//! without an app release; GLSL's own words come from the highlighter.

use super::api::ShaderApi;
use super::tokens;

/// One suggestion: what to insert, how it's called, what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub label: String,
    /// A function's parameter list, or what kind of GLSL word it is.
    pub detail: String,
    pub doc: String,
}

/// Fewest typed characters before suggesting (`ve` offers `vec4`).
const MIN_PREFIX: usize = 2;
/// Most suggestions shown.
const MAX: usize = 8;

fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn boundary(text: &str, mut at: usize) -> usize {
    at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// Where the word before `cursor` starts, and the word so far.
fn word_before(text: &str, cursor: usize) -> (usize, &str) {
    let cursor = boundary(text, cursor);
    let start = text[..cursor]
        .char_indices()
        .rev()
        .find(|(_, ch)| !is_word(*ch))
        .map_or(0, |(at, ch)| at + ch.len_utf8());
    (start, &text[start..cursor])
}

/// The entry point's name in `vec4 border(vec2 uv)`.
fn entry_name(signature: &str) -> &str {
    let head = signature.split('(').next().unwrap_or(signature);
    head.rsplit(char::is_whitespace).next().unwrap_or(head)
}

fn entry_hint(signature: &str) -> Hint {
    Hint {
        label: entry_name(signature).to_owned(),
        detail: signature
            .find('(')
            .map_or_else(String::new, |at| signature[at..].to_owned()),
        doc: format!("The entry point umbriel calls: {signature}."),
    }
}

/// What to offer for the word ending at `cursor` in a shader of `kind`:
/// names that start with it and aren't it already. Nothing inside a
/// `//` comment, after a `.` (a swizzle or member), or in the middle of
/// a word.
pub fn suggest(text: &str, cursor: usize, kind: &str, api: &ShaderApi) -> Vec<Hint> {
    let (start, prefix) = word_before(text, cursor);
    let cursor = boundary(text, cursor);
    let before = &text[..start];
    let line = &before[before.rfind('\n').map_or(0, |at| at + 1)..];
    if prefix.chars().count() < MIN_PREFIX
        || prefix.starts_with(|ch: char| ch.is_ascii_digit())
        || before.ends_with('.')
        || line.contains("//")
        || text[cursor..].starts_with(is_word)
    {
        return Vec::new();
    }
    let mut hints = reference(kind, api);
    hints.extend(tokens::glsl_words().map(|(word, what)| Hint {
        label: word.to_owned(),
        detail: what.to_owned(),
        doc: String::new(),
    }));
    hints.retain(|hint| hint.label.starts_with(prefix) && hint.label != prefix);
    hints.dedup_by(|a, b| a.label == b.label);
    hints.truncate(MAX);
    hints
}

/// What umbriel gives a shader of `kind`: its entry point, then every
/// `umbriel_*` name it sees.
pub fn reference(kind: &str, api: &ShaderApi) -> Vec<Hint> {
    let entries = api
        .entry_points
        .iter()
        .filter(|(entry_kind, _)| entry_kind == kind)
        .map(|(_, signature)| entry_hint(signature));
    let names = api.names_for(kind).map(|item| Hint {
        label: item.name.clone(),
        detail: item.signature.clone(),
        doc: item.description.clone(),
    });
    entries.chain(names).collect()
}

/// `text` with the word before `cursor` replaced by `label`, and the
/// caret after it.
pub fn accept(text: &str, cursor: usize, label: &str) -> (String, usize) {
    let cursor = boundary(text, cursor);
    let (start, _) = word_before(text, cursor);
    let out = format!("{}{label}{}", &text[..start], &text[cursor..]);
    (out, start + label.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::umbriel_docs;

    fn api() -> ShaderApi {
        ShaderApi::build(umbriel_docs::BUNDLED, None)
    }

    fn labels(text: &str, kind: &str) -> Vec<String> {
        suggest(text, text.len(), kind, &api())
            .into_iter()
            .map(|hint| hint.label)
            .collect()
    }

    #[test]
    fn umbriel_names_and_glsl_words_complete_by_prefix() {
        let found = labels("    vec4 c = umbriel_sam", "animation");
        assert_eq!(found, ["umbriel_sample", "umbriel_sample_previous"]);
        let found = labels("ve", "animation");
        assert!(found.contains(&"vec2".to_owned()) && found.contains(&"vec4".to_owned()));
        assert!(labels("smooth", "border").contains(&"smoothstep".to_owned()));
    }

    #[test]
    fn a_kind_is_offered_only_what_it_sees() {
        assert!(
            labels("umbriel_border_d", "border").contains(&"umbriel_border_distance".to_owned())
        );
        assert!(labels("umbriel_border_d", "animation").is_empty());
        // Its own entry point comes first, the others' never.
        assert_eq!(labels("bor", "border")[0], "border");
        assert!(!labels("bor", "window").contains(&"border".to_owned()));
        let hint = &suggest("bor", 3, "border", &api())[0];
        assert_eq!(hint.detail, "(vec2 uv)");
    }

    #[test]
    fn nothing_is_offered_where_it_would_be_noise() {
        // Too short, finished, in a comment, a member, mid-word, a number.
        assert!(labels("v", "animation").is_empty());
        assert!(labels("umbriel_time", "animation").is_empty());
        assert!(labels("// umbriel_sam", "animation").is_empty());
        assert!(labels("uv.xy", "animation").is_empty());
        assert!(labels("1.0e", "animation").is_empty());
        let text = "umbriel_sample";
        assert!(suggest(text, 5, "animation", &api()).is_empty());
        // A comment on an earlier line doesn't count.
        assert!(!labels("// a\numbriel_sam", "animation").is_empty());
    }

    #[test]
    fn the_list_is_capped() {
        assert!(labels("in", "animation").len() <= MAX);
    }

    #[test]
    fn accepting_replaces_the_word_before_the_caret() {
        let (text, caret) = accept("x = umbriel_sa(uv);", 14, "umbriel_sample");
        assert_eq!(text, "x = umbriel_sample(uv);");
        assert_eq!(caret, 18);
        assert_eq!(accept("ve", 2, "vec4"), ("vec4".into(), 4));
        // A multi-byte character before the word is no trouble.
        assert_eq!(accept("é ve", 5, "vec4"), ("é vec4".into(), 7));
    }

    #[test]
    fn the_reference_starts_with_the_entry_point() {
        let list = reference("cursor", &api());
        assert_eq!(list[0].label, "cursor");
        assert!(list.iter().any(|hint| hint.label == "umbriel_pointer"));
        assert!(!list.iter().any(|hint| hint.label == "umbriel_progress"));
    }
}
