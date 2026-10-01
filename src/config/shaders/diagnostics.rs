//! A GLSL compile log as problems with line numbers, for the editor to
//! mark. The preview compiles with `#line 1` ahead of the user's code,
//! so the lines in the log are the editor's own.

/// One problem from the driver's log. `line` is 1-based; 0 when the
/// driver named no line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub line: usize,
    pub message: String,
}

/// The problems in a driver log, one per line of it. Drivers differ:
/// `ERROR: 0:4: 'x' : message` (ANGLE, glslang), `0:4(2): error: message`
/// (Mesa) and `0(4) : error C1008: message` (NVIDIA). A line in none of
/// these keeps its text and no line number.
pub fn parse(log: &str) -> Vec<Diagnostic> {
    log.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("NOTE:"))
        .map(|line| {
            let (line_number, message) = split_position(line);
            Diagnostic {
                line: line_number,
                message: message.to_owned(),
            }
        })
        .collect()
}

/// `(line, message)` of one log line.
fn split_position(text: &str) -> (usize, &str) {
    let rest = text
        .strip_prefix("ERROR:")
        .or_else(|| text.strip_prefix("error:"))
        .unwrap_or(text)
        .trim_start();
    let Some((_, after_file)) = number(rest) else {
        return (0, text);
    };
    let Some(after_sep) = after_file
        .strip_prefix(':')
        .or_else(|| after_file.strip_prefix('('))
    else {
        return (0, text);
    };
    let Some((line, mut after)) = number(after_sep) else {
        return (0, text);
    };
    // The rest of the position: `)` after NVIDIA's line, `(column)`
    // after Mesa's.
    after = after.strip_prefix(')').unwrap_or(after);
    if let Some(column) = after.strip_prefix('(')
        && let Some((_, tail)) = number(column)
    {
        after = tail.strip_prefix(')').unwrap_or(tail);
    }
    let message = after.trim_start_matches([' ', ':']);
    // Mesa and NVIDIA add their own "error" (and "C1008") label.
    let message = message
        .strip_prefix("error")
        .map(|tail| {
            let tail = tail.trim_start();
            let tail = tail
                .strip_prefix('C')
                .and_then(number)
                .map_or(tail, |(_, after_code)| after_code);
            tail.trim_start_matches([' ', ':'])
        })
        .unwrap_or(message);
    (line, message)
}

/// A leading run of digits and what follows it.
fn number(text: &str) -> Option<(usize, &str)> {
    let end = text
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(text.len());
    let value = text[..end].parse().ok()?;
    Some((value, &text[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(log: &str) -> (usize, String) {
        let found = parse(log);
        assert_eq!(found.len(), 1, "{log:?}");
        (found[0].line, found[0].message.clone())
    }

    #[test]
    fn each_drivers_format_gives_a_line_and_a_message() {
        assert_eq!(
            one("ERROR: 0:4: 'foo' : undeclared identifier"),
            (4, "'foo' : undeclared identifier".into())
        );
        assert_eq!(
            one("0:12(7): error: 'foo' undeclared"),
            (12, "'foo' undeclared".into())
        );
        assert_eq!(
            one("0(9) : error C1008: undefined variable \"foo\""),
            (9, "undefined variable \"foo\"".into())
        );
    }

    #[test]
    fn lines_without_a_position_keep_their_text() {
        assert_eq!(
            one("ERROR: unresolved external symbol"),
            (0, "ERROR: unresolved external symbol".into())
        );
        assert_eq!(one("shader rejected by the driver").0, 0);
    }

    #[test]
    fn notes_and_blanks_are_dropped_and_every_error_kept() {
        let log = "NOTE: debug\n\nERROR: 0:4: 'a' : x\n  ERROR: 0:9: 'b' : y\n";
        let found = parse(log);
        assert_eq!(found.iter().map(|d| d.line).collect::<Vec<_>>(), vec![4, 9]);
        assert!(parse("").is_empty());
    }
}
