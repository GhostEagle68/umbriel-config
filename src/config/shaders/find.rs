//! Find and replace for the shader editor. Matching ignores ASCII case
//! (shader code is ASCII; this keeps byte offsets intact). Every step
//! returns `(text, anchor, cursor)` like the editor keys in
//! [`super::code_edit`]; the match found is selected, and a result equal
//! to the input means there was nothing to do.

/// What a find-bar button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The first match at or after the selection's start: as the query
    /// is typed, the current match stays put while it still matches.
    From,
    Next,
    Previous,
    /// Replace the selected match, then select the next; with nothing
    /// matching selected, just select the next.
    Replace,
    ReplaceAll,
}

/// Byte ranges of every match of `query`, in order, not overlapping.
pub fn matches(text: &str, query: &str) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return Vec::new();
    }
    let (text, query) = (text.to_ascii_lowercase(), query.to_ascii_lowercase());
    text.match_indices(&query)
        .map(|(at, found)| (at, at + found.len()))
        .collect()
}

/// The find bar's count: which match the selection start is on ("2 of
/// 4"), else how many there are; empty with no query.
pub fn status(text: &str, query: &str, selection_start: usize) -> String {
    let found = matches(text, query);
    match (found.len(), query.is_empty()) {
        (_, true) => String::new(),
        (0, _) => "No matches".to_owned(),
        (1, _) if found[0].0 != selection_start => "1 match".to_owned(),
        (total, _) => match found.iter().position(|m| m.0 == selection_start) {
            Some(at) => format!("{} of {total}", at + 1),
            None => format!("{total} matches"),
        },
    }
}

pub fn step(
    text: &str,
    anchor: usize,
    cursor: usize,
    query: &str,
    replacement: &str,
    step: Step,
) -> (String, usize, usize) {
    let unchanged = (text.to_owned(), anchor, cursor);
    let found = matches(text, query);
    let (start, end) = (anchor.min(cursor), anchor.max(cursor));
    let select = |range: Option<&(usize, usize)>| match range {
        Some(&(from, to)) => (text.to_owned(), from, to),
        None => unchanged.clone(),
    };
    match step {
        Step::From => select(
            found
                .iter()
                .find(|m| m.0 >= start)
                .or_else(|| found.first()),
        ),
        Step::Next => select(found.iter().find(|m| m.0 >= end).or_else(|| found.first())),
        Step::Previous => select(found.iter().rfind(|m| m.0 < start).or_else(|| found.last())),
        Step::Replace => {
            if !found.contains(&(start, end)) {
                return step_next(text, anchor, cursor, query, replacement);
            }
            let out = format!("{}{replacement}{}", &text[..start], &text[end..]);
            let caret = start + replacement.len();
            step_next(&out, caret, caret, query, replacement)
        }
        Step::ReplaceAll => {
            let Some(&(first, _)) = found.first() else {
                return unchanged;
            };
            let mut out = String::with_capacity(text.len());
            let mut copied = 0;
            for &(from, to) in &found {
                out.push_str(&text[copied..from]);
                out.push_str(replacement);
                copied = to;
            }
            out.push_str(&text[copied..]);
            (out, first, first)
        }
    }
}

fn step_next(
    text: &str,
    anchor: usize,
    cursor: usize,
    query: &str,
    replacement: &str,
) -> (String, usize, usize) {
    step(text, anchor, cursor, query, replacement, Step::Next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn go(
        text: &str,
        sel: (usize, usize),
        query: &str,
        with: &str,
        how: Step,
    ) -> (String, usize, usize) {
        step(text, sel.0, sel.1, query, with, how)
    }

    #[test]
    fn matches_ignore_case_and_never_overlap() {
        assert_eq!(matches("Ab ab AB", "ab"), [(0, 2), (3, 5), (6, 8)]);
        assert_eq!(matches("aaaa", "aa"), [(0, 2), (2, 4)]);
        assert!(matches("abc", "").is_empty());
        assert!(matches("abc", "x").is_empty());
        // Offsets stay right after non-ASCII text.
        let text = "é ab";
        let found = matches(text, "AB");
        assert_eq!(&text[found[0].0..found[0].1], "ab");
    }

    #[test]
    fn next_and_previous_wrap_around() {
        let text = "ab cd ab cd ab";
        // From a caret: the match at or after it.
        assert_eq!(go(text, (0, 0), "ab", "", Step::Next), (text.into(), 0, 2));
        // From a selected match: the following one.
        assert_eq!(go(text, (0, 2), "ab", "", Step::Next), (text.into(), 6, 8));
        // Past the last: back to the first.
        assert_eq!(
            go(text, (12, 14), "ab", "", Step::Next),
            (text.into(), 0, 2)
        );
        assert_eq!(
            go(text, (6, 8), "ab", "", Step::Previous),
            (text.into(), 0, 2)
        );
        assert_eq!(
            go(text, (0, 2), "ab", "", Step::Previous),
            (text.into(), 12, 14)
        );
        // The current match stays while typing continues to match it.
        assert_eq!(go(text, (6, 8), "ab", "", Step::From), (text.into(), 6, 8));
        assert_eq!(
            go(text, (7, 7), "ab", "", Step::From),
            (text.into(), 12, 14)
        );
        // No match: nothing changes.
        assert_eq!(go(text, (3, 4), "zz", "", Step::Next), (text.into(), 3, 4));
    }

    #[test]
    fn replace_swaps_the_selected_match_then_moves_on() {
        let text = "a1 a2 a3";
        // Nothing selected yet: it only finds the first.
        assert_eq!(
            go(text, (0, 0), "a", "XY", Step::Replace),
            (text.into(), 0, 1)
        );
        // A selected match is replaced and the next one selected.
        assert_eq!(
            go(text, (0, 1), "a", "XY", Step::Replace),
            ("XY1 a2 a3".into(), 4, 5)
        );
        // With nothing left to match, the caret stays after the text.
        assert_eq!(
            go("XY a", (3, 4), "a", "b", Step::Replace),
            ("XY b".into(), 4, 4)
        );
        // Otherwise the search wraps round to an earlier match.
        assert_eq!(
            go("a XY a", (5, 6), "a", "b", Step::Replace),
            ("a XY b".into(), 0, 1)
        );
    }

    #[test]
    fn replace_all_replaces_every_match_in_one_pass() {
        assert_eq!(
            go("a b A", (0, 0), "a", "aa", Step::ReplaceAll),
            ("aa b aa".into(), 0, 0)
        );
        assert_eq!(
            go("abc", (1, 2), "x", "y", Step::ReplaceAll),
            ("abc".into(), 1, 2)
        );
        assert_eq!(go("a-a", (0, 0), "a", "", Step::ReplaceAll).0, "-");
    }

    #[test]
    fn the_status_names_the_current_match_or_counts() {
        let text = "ab cd ab cd ab";
        assert_eq!(status(text, "ab", 6), "2 of 3");
        assert_eq!(status(text, "ab", 0), "1 of 3");
        // The caret isn't on a match: just how many.
        assert_eq!(status(text, "ab", 3), "3 matches");
        assert_eq!(status(text, "cd", 0), "2 matches");
        assert_eq!(status("xab", "ab", 0), "1 match");
        assert_eq!(status("xab", "ab", 1), "1 of 1");
        assert_eq!(status(text, "zz", 0), "No matches");
        assert_eq!(status(text, "", 0), "");
    }
}
