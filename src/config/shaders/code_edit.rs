//! Code-editor keys the plain text box doesn't handle: Tab indents,
//! Shift+Tab outdents, Enter keeps the line's indentation (and indents
//! after `{`), Ctrl+/ comments lines, and brackets close themselves.
//! Offsets are UTF-8 byte offsets (what Slint's text input reports);
//! every result is `(text, anchor, cursor)` with the selection to
//! restore. A result equal to the input means the key did nothing here
//! and the text box should handle it as usual.

const INDENT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Indent,
    Outdent,
    Newline,
    ToggleComment,
    /// An opening bracket was typed: `(`, `[` or `{`.
    Open(char),
    /// A closing bracket was typed.
    Close(char),
    Backspace,
}

pub fn apply(text: &str, anchor: usize, cursor: usize, key: Key) -> (String, usize, usize) {
    let anchor = boundary(text, anchor);
    let cursor = boundary(text, cursor);
    let (start, end) = (anchor.min(cursor), anchor.max(cursor));
    let multi_line = text[start..end].contains('\n');
    match key {
        Key::Newline => newline(text, start, end),
        // A caret or a selection inside one line: pad to the next
        // tab stop, replacing any selected text.
        Key::Indent if !multi_line => {
            let column = text[line_start(text, start)..start].chars().count();
            replace(text, start, end, &" ".repeat(INDENT - column % INDENT))
        }
        Key::Indent | Key::Outdent => shift_lines(text, anchor, cursor, key == Key::Indent),
        Key::ToggleComment => toggle_comment(text, anchor, cursor),
        Key::Open(open) => open_bracket(text, anchor, cursor, open),
        Key::Close(close) => {
            if anchor == cursor && text[cursor..].starts_with(close) {
                // Typing over the bracket that closed itself.
                let at = cursor + close.len_utf8();
                (text.to_owned(), at, at)
            } else {
                (text.to_owned(), anchor, cursor)
            }
        }
        Key::Backspace => {
            let pair = text[..cursor].chars().next_back().and_then(closer);
            match pair {
                Some(close) if anchor == cursor && text[cursor..].starts_with(close) => {
                    let at = cursor - 1;
                    (format!("{}{}", &text[..at], &text[cursor + 1..]), at, at)
                }
                _ => (text.to_owned(), anchor, cursor),
            }
        }
    }
}

fn closer(open: char) -> Option<char> {
    match open {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    }
}

fn replace(text: &str, start: usize, end: usize, with: &str) -> (String, usize, usize) {
    let out = format!("{}{with}{}", &text[..start], &text[end..]);
    let caret = start + with.len();
    (out, caret, caret)
}

/// Enter: keep the line's indentation, one step deeper after a `{`; a
/// `}` right behind the caret drops to its own line.
fn newline(text: &str, start: usize, end: usize) -> (String, usize, usize) {
    let line = line_start(text, start);
    let indent: String = text[line..]
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect();
    if !text[line..start].trim_end().ends_with('{') {
        return replace(text, start, end, &format!("\n{indent}"));
    }
    let inner = format!("{indent}{}", " ".repeat(INDENT));
    if text[end..].starts_with('}') {
        let with = format!("\n{inner}\n{indent}");
        let out = format!("{}{with}{}", &text[..start], &text[end..]);
        let caret = start + 1 + inner.len();
        return (out, caret, caret);
    }
    replace(text, start, end, &format!("\n{inner}"))
}

/// A typed `(`, `[` or `{`: wraps a selection, and closes itself when
/// what follows isn't part of a word (so `(` before `x` stays alone).
fn open_bracket(text: &str, anchor: usize, cursor: usize, open: char) -> (String, usize, usize) {
    let (start, end) = (anchor.min(cursor), anchor.max(cursor));
    let Some(close) = closer(open) else {
        return (text.to_owned(), anchor, cursor);
    };
    if start < end {
        let out = format!(
            "{}{open}{}{close}{}",
            &text[..start],
            &text[start..end],
            &text[end..]
        );
        return (out, start + 1, end + 1);
    }
    let alone = text[cursor..]
        .chars()
        .next()
        .is_none_or(|next| next.is_whitespace() || ")]};,".contains(next));
    if alone {
        let out = format!("{}{open}{close}{}", &text[..cursor], &text[cursor..]);
        (out, cursor + 1, cursor + 1)
    } else {
        (text.to_owned(), anchor, cursor)
    }
}

/// `(start, end)` of every line the selection touches. A selection
/// ending at column 0 leaves that last line alone, like editors do.
fn touched_lines(text: &str, anchor: usize, cursor: usize) -> Vec<(usize, usize)> {
    let (start, end) = (anchor.min(cursor), anchor.max(cursor));
    let last_end = if end > start && end == line_start(text, end) {
        end - 1
    } else {
        end
    };
    let mut lines = Vec::new();
    let mut at = line_start(text, start);
    loop {
        let line_end = text[at..].find('\n').map_or(text.len(), |i| at + i);
        lines.push((at, line_end));
        if line_end >= last_end || line_end == text.len() {
            return lines;
        }
        at = line_end + 1;
    }
}

/// Indent or outdent every line the selection touches.
fn shift_lines(text: &str, anchor: usize, cursor: usize, indent: bool) -> (String, usize, usize) {
    let edits: Vec<Edit> = touched_lines(text, anchor, cursor)
        .into_iter()
        .map(|(at, end)| {
            if indent {
                (at, " ".repeat(INDENT), 0)
            } else {
                let line = &text[at..end];
                let removed = if line.starts_with('\t') {
                    1
                } else {
                    line.bytes().take(INDENT).take_while(|b| *b == b' ').count()
                };
                (at, String::new(), removed)
            }
        })
        .collect();
    splice(text, &edits, anchor, cursor)
}

/// Comment every code line the selection touches with `// ` (at the
/// shallowest indentation), or, when they all are already, uncomment
/// them. Blank lines are left alone.
fn toggle_comment(text: &str, anchor: usize, cursor: usize) -> (String, usize, usize) {
    let indent_of = |line: &str| line.len() - line.trim_start_matches([' ', '\t']).len();
    let code: Vec<(usize, &str)> = touched_lines(text, anchor, cursor)
        .into_iter()
        .map(|(at, end)| (at, &text[at..end]))
        .filter(|(_, line)| !line.trim().is_empty())
        .collect();
    let uncomment = !code.is_empty()
        && code
            .iter()
            .all(|(_, line)| line.trim_start().starts_with("//"));
    let column = code
        .iter()
        .map(|(_, line)| indent_of(line))
        .min()
        .unwrap_or(0);
    let edits: Vec<Edit> = code
        .iter()
        .map(|&(at, line)| {
            if uncomment {
                let at = at + indent_of(line);
                let space = text[at + 2..].starts_with(' ');
                (at, String::new(), 2 + usize::from(space))
            } else {
                (at + column, "// ".to_owned(), 0)
            }
        })
        .collect();
    splice(text, &edits, anchor, cursor)
}

/// `(where, what to insert, how many bytes to remove first)`.
type Edit = (usize, String, usize);

/// `text` with `edits` (ascending, not overlapping) applied and the
/// selection carried along. An old offset moves by every edit at or
/// before it; one inside removed text lands at the edit's start.
fn splice(text: &str, edits: &[Edit], anchor: usize, cursor: usize) -> (String, usize, usize) {
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for (at, insert, removed) in edits {
        out.push_str(&text[copied..*at]);
        out.push_str(insert);
        copied = at + removed;
    }
    out.push_str(&text[copied..]);
    let map = |pos: usize| {
        let shift: isize = edits
            .iter()
            .filter(|(at, _, _)| *at <= pos)
            .map(|(at, insert, removed)| insert.len() as isize - (pos - at).min(*removed) as isize)
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

/// The 0-based line and character column of byte `offset`.
pub fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..boundary(text, offset)];
    let line = before.matches('\n').count();
    (
        line,
        before[line_start(before, before.len())..].chars().count(),
    )
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

    #[test]
    fn a_position_is_a_line_and_a_character_column() {
        use super::line_col;
        assert_eq!(line_col("ab\ncd", 0), (0, 0));
        assert_eq!(line_col("ab\ncd", 4), (1, 1));
        assert_eq!(line_col("ab\n", 3), (1, 0));
        // Columns count characters, not bytes.
        assert_eq!(line_col("é(x", 3), (0, 2));
    }

    #[test]
    fn enter_indents_after_an_open_brace_and_splits_a_pair() {
        use super::{Key, apply};
        assert_eq!(
            apply("  f() {", 7, 7, Key::Newline),
            ("  f() {\n      ".into(), 14, 14)
        );
        // Trailing spaces after the brace don't matter.
        assert_eq!(apply("{ ", 2, 2, Key::Newline).0, "{ \n    ");
        // `{|}` opens a block with the closing brace on its own line.
        assert_eq!(
            apply("  {}", 3, 3, Key::Newline),
            ("  {\n      \n  }".into(), 10, 10)
        );
        // A brace elsewhere on the line isn't an open block.
        assert_eq!(apply("a { b", 5, 5, Key::Newline).0, "a { b\n");
    }

    #[test]
    fn ctrl_slash_comments_and_uncomments_whole_lines() {
        use super::{Key, apply};
        let text = "  a;\n\n    b;\n";
        // Both code lines, at the shallowest indent; the blank is skipped.
        let (out, anchor, cursor) = apply(text, 0, 9, Key::ToggleComment);
        assert_eq!(out, "  // a;\n\n  //   b;\n");
        assert_eq!((anchor, cursor), (0, 15));
        // Toggling again restores it, selection included.
        assert_eq!(
            apply(&out, anchor, cursor, Key::ToggleComment),
            (text.into(), 0, 9)
        );
        // A single line from a caret; a lone `//` without a space.
        assert_eq!(
            apply("x;", 1, 1, Key::ToggleComment),
            ("// x;".into(), 4, 4)
        );
        assert_eq!(apply("//x;", 3, 3, Key::ToggleComment), ("x;".into(), 1, 1));
        // Mixed lines get commented, not uncommented.
        assert_eq!(
            apply("// a\nb", 0, 7, Key::ToggleComment).0,
            "// // a\n// b"
        );
        // Nothing but blanks: nothing to do.
        assert_eq!(apply("\n", 0, 0, Key::ToggleComment).0, "\n");
    }

    #[test]
    fn brackets_close_themselves_and_are_typed_over() {
        use super::{Key, apply};
        // At the end, or before a space or a closer, the pair appears.
        assert_eq!(apply("f", 1, 1, Key::Open('(')), ("f()".into(), 2, 2));
        assert_eq!(apply("a b", 1, 1, Key::Open('[')), ("a[] b".into(), 2, 2));
        assert_eq!(apply("{)", 1, 1, Key::Open('(')), ("{())".into(), 2, 2));
        // Before a word it types alone (unchanged: the box handles it).
        assert_eq!(apply("x", 0, 0, Key::Open('(')), ("x".into(), 0, 0));
        // A selection is wrapped and stays selected.
        assert_eq!(apply("abc", 1, 2, Key::Open('(')), ("a(b)c".into(), 2, 3));
        // The closer is typed over; elsewhere it is left to the box.
        assert_eq!(apply("f()", 2, 2, Key::Close(')')), ("f()".into(), 3, 3));
        assert_eq!(apply("f(", 2, 2, Key::Close(')')), ("f(".into(), 2, 2));
        // Backspace between a pair removes both.
        assert_eq!(apply("f()", 2, 2, Key::Backspace), ("f".into(), 1, 1));
        assert_eq!(apply("f(x)", 2, 2, Key::Backspace), ("f(x)".into(), 2, 2));
        assert_eq!(apply("f()", 1, 2, Key::Backspace), ("f()".into(), 1, 2));
    }
}
