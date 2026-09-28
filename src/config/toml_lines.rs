//! TOML read line by line, for the file viewer: which line sets a key,
//! and a light highlighting of each line. A reading aid only: the
//! document itself is parsed by `toml_edit`.

/// What a piece of a line is, for its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Plain,
    Comment,
    Header,
    Key,
    Text,
    Number,
    Punct,
}

/// Split a line into colored pieces; joined back they give the line.
pub fn highlight(line: &str) -> Vec<(String, Token)> {
    let mut out: Vec<(String, Token)> = Vec::new();
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    if !indent.is_empty() {
        out.push((indent.to_owned(), Token::Plain));
    }
    if trimmed.starts_with('#') {
        out.push((trimmed.to_owned(), Token::Comment));
        return out;
    }
    let rest = if trimmed.starts_with('[') && !value_line(trimmed) {
        let end = trimmed.rfind(']').map_or(trimmed.len(), |i| i + 1);
        out.push((trimmed[..end].to_owned(), Token::Header));
        &trimmed[end..]
    } else if let Some(eq) = key_end(trimmed) {
        out.push((trimmed[..eq].to_owned(), Token::Key));
        out.push(("=".to_owned(), Token::Punct));
        &trimmed[eq + 1..]
    } else {
        trimmed
    };
    value_tokens(rest, &mut out);
    out
}

/// A `[` that opens a value (an array continuation line), not a header.
fn value_line(trimmed: &str) -> bool {
    let inner = trimmed.trim_start_matches('[').trim_start();
    inner.starts_with(['"', '\'', '{', '[']) || inner.starts_with(|c: char| c.is_ascii_digit())
}

/// Where a `key = value` line's `=` is: before any quote, comment or
/// bracket.
fn key_end(line: &str) -> Option<usize> {
    for (i, c) in line.char_indices() {
        match c {
            '=' => return Some(i),
            '"' | '\'' | '#' | '[' | '{' | ',' => return None,
            _ => {}
        }
    }
    None
}

fn value_tokens(text: &str, out: &mut Vec<(String, Token)>) {
    let mut plain = String::new();
    let mut chars = text.char_indices().peekable();
    let flush = |plain: &mut String, out: &mut Vec<(String, Token)>| {
        if !plain.is_empty() {
            let token = match plain.trim() {
                "true" | "false" => Token::Number,
                word if word.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+') => {
                    Token::Number
                }
                _ => Token::Plain,
            };
            out.push((std::mem::take(plain), token));
        }
    };
    while let Some((i, c)) = chars.next() {
        match c {
            '#' => {
                flush(&mut plain, out);
                out.push((text[i..].to_owned(), Token::Comment));
                return;
            }
            '"' | '\'' => {
                flush(&mut plain, out);
                let mut end = text.len();
                let mut escaped = false;
                for (j, d) in chars.by_ref() {
                    if d == c && !(escaped && c == '"') {
                        end = j + 1;
                        break;
                    }
                    escaped = d == '\\' && !escaped;
                }
                out.push((text[i..end].to_owned(), Token::Text));
            }
            '[' | ']' | '{' | '}' | ',' | '=' => {
                flush(&mut plain, out);
                out.push((c.to_string(), Token::Punct));
            }
            ' ' | '\t' => {
                // Spaces end a word: `1 # note` colors the 1 alone.
                flush(&mut plain, out);
                out.push((c.to_string(), Token::Plain));
            }
            _ => plain.push(c),
        }
    }
    flush(&mut plain, out);
}

/// The line (0-based) that sets `dotted`: the key itself, else the
/// closest table or key above it (an inline table, an array of tables).
pub fn key_line(text: &str, dotted: &str) -> Option<usize> {
    let want: Vec<&str> = dotted.split('.').collect();
    let mut table: Vec<String> = Vec::new();
    let mut best: Option<(usize, usize)> = None;
    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        let path: Vec<String> = if trimmed.starts_with('[') && !value_line(trimmed) {
            table = parts(trimmed.trim_matches(['[', ']']));
            table.clone()
        } else if let Some(eq) = key_end(trimmed) {
            let mut path = table.clone();
            path.extend(parts(&trimmed[..eq]));
            path
        } else {
            continue;
        };
        let depth = path.len();
        if depth > 0
            && depth <= want.len()
            && path.iter().zip(&want).all(|(have, want)| have == want)
            && best.is_none_or(|(_, best)| depth > best)
        {
            best = Some((number, depth));
        }
    }
    best.map(|(number, _)| number)
}

/// `a."b c".d` → its parts, unquoted.
fn parts(key: &str) -> Vec<String> {
    key.split('.')
        .map(|part| part.trim().trim_matches(['"', '\'']).to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "\
# top
[general]
xwayland = true
mod_key = \"super\"

[animation.windows_in]
duration_ms = 300
curve.kind = \"spring\"

[[window_rule]]
match = { app_id = \"foot\" }
";

    #[test]
    fn key_line_finds_plain_dotted_and_nested_keys() {
        assert_eq!(key_line(TEXT, "general.mod_key"), Some(3));
        assert_eq!(key_line(TEXT, "animation.windows_in.duration_ms"), Some(6));
        assert_eq!(key_line(TEXT, "animation.windows_in.curve.kind"), Some(7));
        // Inside an inline table: the line that holds it.
        assert_eq!(key_line(TEXT, "window_rule.match.app_id"), Some(10));
        // Unset key: its table's header; unknown table: nothing.
        assert_eq!(key_line(TEXT, "general.autostart"), Some(1));
        assert_eq!(key_line(TEXT, "input.keyboard.layout"), None);
    }

    #[test]
    fn highlight_colors_each_piece_and_keeps_the_text() {
        let kinds = |line: &str| -> Vec<Token> {
            highlight(line)
                .into_iter()
                .filter(|(text, _)| !text.trim().is_empty())
                .map(|(_, token)| token)
                .collect()
        };
        assert_eq!(kinds("# note"), vec![Token::Comment]);
        assert_eq!(kinds("[general]"), vec![Token::Header]);
        assert_eq!(
            kinds("gap = 5 # px"),
            vec![Token::Key, Token::Punct, Token::Number, Token::Comment]
        );
        assert_eq!(
            kinds("files = [\"a # b\", 'c']"),
            vec![
                Token::Key,
                Token::Punct,
                Token::Punct,
                Token::Text,
                Token::Punct,
                Token::Text,
                Token::Punct
            ]
        );
        // An array's continuation line isn't a header.
        assert_eq!(
            kinds("  [\"x\"],"),
            vec![Token::Punct, Token::Text, Token::Punct, Token::Punct]
        );
        for line in TEXT.lines() {
            let joined: String = highlight(line).into_iter().map(|(text, _)| text).collect();
            assert_eq!(joined, line);
        }
    }
}
