//! Code-editor keys the plain text box doesn't handle: Tab indents,
//! Shift+Tab outdents, Enter keeps the line's indentation. Offsets are
//! UTF-8 byte offsets (what Slint's text input reports); every result
//! is `(text, anchor, cursor)` with the selection to restore.

const INDENT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Indent,
    Outdent,
    Newline,
}

pub fn apply(text: &str, anchor: usize, cursor: usize, key: Key) -> (String, usize, usize) {
    let anchor = boundary(text, anchor);
    let cursor = boundary(text, cursor);
    let (start, end) = (anchor.min(cursor), anchor.max(cursor));
    let multi_line = text[start..end].contains('\n');
    match key {
        Key::Newline => {
            let indent: String = text[line_start(text, start)..]
                .chars()
                .take_while(|ch| *ch == ' ' || *ch == '\t')
                .collect();
            replace(text, start, end, &format!("\n{indent}"))
        }
        // A caret or a selection inside one line: pad to the next
        // tab stop, replacing any selected text.
        Key::Indent if !multi_line => {
            let column = text[line_start(text, start)..start].chars().count();
            replace(text, start, end, &" ".repeat(INDENT - column % INDENT))
        }
        Key::Indent | Key::Outdent => shift_lines(text, anchor, cursor, key == Key::Indent),
    }
}

fn replace(text: &str, start: usize, end: usize, with: &str) -> (String, usize, usize) {
    let out = format!("{}{with}{}", &text[..start], &text[end..]);
    let caret = start + with.len();
    (out, caret, caret)
}

/// Indent or outdent every line the selection touches. A selection
/// ending at column 0 leaves that last line alone, like editors do.
fn shift_lines(text: &str, anchor: usize, cursor: usize, indent: bool) -> (String, usize, usize) {
    let (start, end) = (anchor.min(cursor), anchor.max(cursor));
    let first = line_start(text, start);
    let last_end = if end > start && end == line_start(text, end) {
        end - 1
    } else {
        end
    };
    // (line start in the old text, bytes added, bytes removed)
    let mut edits: Vec<(usize, usize, usize)> = Vec::new();
    let mut at = first;
    loop {
        let line_end = text[at..].find('\n').map_or(text.len(), |i| at + i);
        if indent {
            edits.push((at, INDENT, 0));
        } else {
            let line = &text[at..line_end];
            let removed = if line.starts_with('\t') {
                1
            } else {
                line.bytes().take(INDENT).take_while(|b| *b == b' ').count()
            };
            edits.push((at, 0, removed));
        }
        if line_end >= last_end || line_end == text.len() {
            break;
        }
        at = line_end + 1;
    }
    let mut out = String::with_capacity(text.len() + edits.len() * INDENT);
    let mut copied = 0;
    for &(line, added, removed) in &edits {
        out.push_str(&text[copied..line]);
        out.push_str(&" ".repeat(added));
        copied = line + removed;
    }
    out.push_str(&text[copied..]);
    // Shift an old offset by every edit at or before it; a position
    // inside removed whitespace lands at its line's new start.
    let map = |pos: usize| {
        let shift: isize = edits
            .iter()
            .filter(|(line, _, _)| *line <= pos)
            .map(|&(line, added, removed)| added as isize - (pos - line).min(removed) as isize)
            .sum();
        (pos as isize + shift) as usize
    };
    (out, map(anchor), map(cursor))
}

/// A code state: text plus selection (byte offsets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub text: String,
    pub anchor: usize,
    pub cursor: usize,
}

/// Undo/redo for the code pane, kept here rather than in the text
/// box: the box's own history is byte positions that go stale (and
/// can crash) once the text is replaced from outside, which Tab,
/// Enter and every builder change do.
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    current: Option<Snapshot>,
    last_typed: Option<std::time::Instant>,
}

/// Keystrokes closer together than this undo as one step.
const GROUP: std::time::Duration = std::time::Duration::from_millis(800);

impl History {
    /// Start over from `text` (the editor opened on new code).
    pub fn reset(&mut self, text: &str) {
        *self = History::default();
        self.current = Some(Snapshot {
            text: text.to_owned(),
            anchor: 0,
            cursor: 0,
        });
    }

    /// The code changed to `next`. `typed` edits close together in
    /// time merge into one undo step; other edits (Tab, Enter, a
    /// builder change) are always their own step.
    pub fn record(&mut self, next: Snapshot, typed: bool, now: std::time::Instant) {
        let Some(current) = self.current.take() else {
            self.current = Some(next);
            return;
        };
        if current.text == next.text {
            self.current = Some(next);
            return;
        }
        let merge = typed
            && self
                .last_typed
                .is_some_and(|last| now.duration_since(last) < GROUP);
        if !merge {
            self.undo.push(current);
        }
        self.redo.clear();
        self.last_typed = typed.then_some(now);
        self.current = Some(next);
    }

    /// Step back; the state to show, if there was one.
    pub fn undo(&mut self) -> Option<Snapshot> {
        let previous = self.undo.pop()?;
        if let Some(current) = self.current.replace(previous.clone()) {
            self.redo.push(current);
        }
        self.last_typed = None;
        Some(previous)
    }

    /// Step forward again after an undo.
    pub fn redo(&mut self) -> Option<Snapshot> {
        let next = self.redo.pop()?;
        if let Some(current) = self.current.replace(next.clone()) {
            self.undo.push(current);
        }
        self.last_typed = None;
        Some(next)
    }
}

fn line_start(text: &str, pos: usize) -> usize {
    text[..pos].rfind('\n').map_or(0, |i| i + 1)
}

/// The byte offset where 1-based `line` starts; the last line's start
/// when `line` is past the end, the first's for 0.
pub fn line_offset(text: &str, line: usize) -> usize {
    text.split_inclusive('\n')
        .take(line.saturating_sub(1))
        .map(str::len)
        .sum::<usize>()
        .min(line_start(text, text.len()))
}

fn boundary(text: &str, pos: usize) -> usize {
    let mut pos = pos.min(text.len());
    while !text.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_line_starts_after_the_newlines_before_it() {
        use super::line_offset;
        let text = "ab\ncd\n\nef";
        assert_eq!(line_offset(text, 1), 0);
        assert_eq!(line_offset(text, 2), 3);
        assert_eq!(line_offset(text, 3), 6);
        assert_eq!(line_offset(text, 4), 7);
        // Past the end: the last line. Line 0: the first.
        assert_eq!(line_offset(text, 99), 7);
        assert_eq!(line_offset(text, 0), 0);
        assert_eq!(line_offset("ab\n", 5), 3);
        assert_eq!(line_offset("", 2), 0);
    }

    #[test]
    fn tab_pads_to_the_next_tab_stop() {
        use super::{Key, apply};
        // Caret at column 0 and column 2 of "ab".
        assert_eq!(apply("ab", 0, 0, Key::Indent), ("    ab".into(), 4, 4));
        assert_eq!(apply("ab", 2, 2, Key::Indent), ("ab  ".into(), 4, 4));
        // A one-line selection is replaced by the padding.
        assert_eq!(apply("abcd", 1, 3, Key::Indent), ("a   d".into(), 4, 4));
    }

    #[test]
    fn tab_and_shift_tab_shift_every_selected_line() {
        use super::{Key, apply};
        let text = "a\n  b\nc";
        // Select from inside line 1 to inside line 2.
        let (out, anchor, cursor) = apply(text, 0, 4, Key::Indent);
        assert_eq!(out, "    a\n      b\nc");
        assert_eq!((anchor, cursor), (4, 12));
        let (back, anchor, cursor) = apply(&out, anchor, cursor, Key::Outdent);
        assert_eq!(back, "a\n  b\nc");
        assert_eq!((anchor, cursor), (0, 4));
        // A selection ending at column 0 leaves that line alone.
        let (out, _, _) = apply("a\nb\n", 0, 2, Key::Indent);
        assert_eq!(out, "    a\nb\n");
        // Shift+Tab on a caret outdents its line; a tab counts as one step.
        assert_eq!(apply("      x", 7, 7, Key::Outdent), ("  x".into(), 3, 3));
        assert_eq!(apply("\tx", 1, 1, Key::Outdent), ("x".into(), 0, 0));
        // A caret inside the removed spaces lands at the line start.
        assert_eq!(apply("    x", 2, 2, Key::Outdent), ("x".into(), 0, 0));
    }

    #[test]
    fn code_history_undoes_typing_in_groups_and_other_edits_singly() {
        use super::{History, Snapshot};
        use std::time::{Duration, Instant};
        let snap = |text: &str| Snapshot {
            text: text.to_owned(),
            anchor: text.len(),
            cursor: text.len(),
        };
        let t0 = Instant::now();
        let mut history = History::default();
        history.reset("a");
        // Quick typing merges into one step...
        history.record(snap("ab"), true, t0);
        history.record(snap("abc"), true, t0 + Duration::from_millis(100));
        // ...an indent (or builder change) is a step of its own.
        history.record(snap("    abc"), false, t0 + Duration::from_millis(200));
        assert_eq!(history.undo().unwrap().text, "abc");
        assert_eq!(history.undo().unwrap().text, "a");
        assert!(history.undo().is_none(), "nothing before the opened code");
        assert_eq!(history.redo().unwrap().text, "abc");
        assert_eq!(history.redo().unwrap().text, "    abc");
        assert!(history.redo().is_none());
        // A new edit after an undo drops the redo branch.
        history.undo();
        history.record(snap("abcd"), true, t0 + Duration::from_secs(5));
        assert!(history.redo().is_none());
        // Typing after a pause starts a new step.
        history.record(snap("abcde"), true, t0 + Duration::from_secs(10));
        assert_eq!(history.undo().unwrap().text, "abcd");
    }

    #[test]
    fn enter_keeps_the_line_indentation() {
        use super::{Key, apply};
        assert_eq!(
            apply("    a = 1;", 10, 10, Key::Newline),
            ("    a = 1;\n    ".into(), 15, 15)
        );
        // Non-ASCII before the caret: offsets stay on char boundaries.
        let text = "  é";
        let (out, caret, _) = apply(text, text.len(), text.len(), Key::Newline);
        assert_eq!(out, "  é\n  ");
        assert_eq!(caret, out.len());
    }
}
