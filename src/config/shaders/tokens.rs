//! Syntax highlighting for the shader editor. Slint's text input can't
//! color spans, so the editor stacks one transparent-text copy of the
//! code per [`Token`] class under the input: each copy keeps the
//! characters of its class and blanks the rest, so every copy lays out
//! exactly like the code itself. [`layer`] builds one such copy.

/// A class of GLSL text, in the order the editor stacks and colors the
/// copies (`ui/pages/shaders.slint`, `ShaderCodeEditor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Plain,
    Comment,
    Keyword,
    Type,
    Builtin,
    Umbriel,
    Number,
    Preproc,
}

/// How many copies the editor stacks.
pub const TOKENS: usize = 8;

/// The `index` that asks [`layer`] for the gutter's line numbers.
pub const LINE_NUMBERS: usize = TOKENS;

const ALL: [Token; TOKENS] = [
    Token::Plain,
    Token::Comment,
    Token::Keyword,
    Token::Type,
    Token::Builtin,
    Token::Umbriel,
    Token::Number,
    Token::Preproc,
];

const KEYWORDS: &[&str] = &[
    "attribute",
    "break",
    "const",
    "continue",
    "discard",
    "do",
    "else",
    "false",
    "for",
    "highp",
    "if",
    "in",
    "inout",
    "invariant",
    "lowp",
    "mediump",
    "out",
    "precision",
    "return",
    "struct",
    "true",
    "uniform",
    "varying",
    "while",
];

const TYPES: &[&str] = &[
    "bool",
    "bvec2",
    "bvec3",
    "bvec4",
    "float",
    "int",
    "ivec2",
    "ivec3",
    "ivec4",
    "mat2",
    "mat3",
    "mat4",
    "sampler2D",
    "samplerCube",
    "vec2",
    "vec3",
    "vec4",
    "void",
];

const BUILTINS: &[&str] = &[
    "abs",
    "acos",
    "all",
    "any",
    "asin",
    "atan",
    "ceil",
    "clamp",
    "cos",
    "cross",
    "degrees",
    "distance",
    "dot",
    "equal",
    "exp",
    "exp2",
    "faceforward",
    "floor",
    "fract",
    "gl_FragColor",
    "gl_FragCoord",
    "greaterThan",
    "greaterThanEqual",
    "inversesqrt",
    "length",
    "lessThan",
    "lessThanEqual",
    "log",
    "log2",
    "matrixCompMult",
    "max",
    "min",
    "mix",
    "mod",
    "normalize",
    "not",
    "notEqual",
    "pow",
    "radians",
    "reflect",
    "refract",
    "sign",
    "sin",
    "smoothstep",
    "sqrt",
    "step",
    "tan",
    "texture2D",
    "texture2DProj",
];

fn word_class(word: &str) -> Token {
    if word.starts_with("umbriel_") {
        Token::Umbriel
    } else if KEYWORDS.contains(&word) {
        Token::Keyword
    } else if TYPES.contains(&word) {
        Token::Type
    } else if BUILTINS.contains(&word) {
        Token::Builtin
    } else {
        Token::Plain
    }
}

/// The class of every character of `text`.
fn classify(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = vec![Token::Plain; chars.len()];
    let mut fill = |from: usize, to: usize, token: Token| out[from..to].fill(token);
    let at = |i: usize| chars.get(i).copied().unwrap_or('\0');
    let line_end = |from: usize| {
        chars[from..]
            .iter()
            .position(|ch| *ch == '\n')
            .map_or(chars.len(), |n| from + n)
    };
    // Only whitespace so far on this line: a `#` starts a directive.
    let mut line_start = true;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\n' {
            line_start = true;
            i += 1;
        } else if ch.is_whitespace() {
            i += 1;
        } else if ch == '/' && at(i + 1) == '/' {
            let end = line_end(i);
            fill(i, end, Token::Comment);
            i = end;
        } else if ch == '/' && at(i + 1) == '*' {
            let end = (i + 2..chars.len())
                .find(|&j| chars[j] == '*' && at(j + 1) == '/')
                .map_or(chars.len(), |j| j + 2);
            fill(i, end, Token::Comment);
            i = end;
            line_start = false;
        } else if ch == '#' && line_start {
            let mut end = line_end(i);
            if let Some(n) = (i..end).find(|&j| chars[j] == '/' && at(j + 1) == '/') {
                end = n;
            }
            fill(i, end, Token::Preproc);
            i = end;
        } else if ch.is_ascii_digit() || (ch == '.' && at(i + 1).is_ascii_digit()) {
            let mut end = i + 1;
            while end < chars.len() {
                let c = chars[end];
                let exponent_sign = (c == '+' || c == '-') && matches!(chars[end - 1], 'e' | 'E');
                if c.is_ascii_alphanumeric() || c == '.' || exponent_sign {
                    end += 1;
                } else {
                    break;
                }
            }
            fill(i, end, Token::Number);
            i = end;
            line_start = false;
        } else if ch.is_alphabetic() || ch == '_' {
            let mut end = i + 1;
            while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            let word: String = chars[i..end].iter().collect();
            fill(i, end, word_class(&word));
            i = end;
            line_start = false;
        } else {
            i += 1;
            line_start = false;
        }
    }
    out
}

/// One copy of the code for the editor's layer `index` (a [`Token`]'s
/// position, or [`LINE_NUMBERS`]): the characters of that class in
/// place, every other character a space. Newlines and tabs stay, so
/// the copy lays out exactly like `text`.
pub fn layer(text: &str, index: usize) -> String {
    if index == LINE_NUMBERS {
        let lines = text.split('\n').count();
        return (1..=lines)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("\n");
    }
    let Some(&token) = ALL.get(index) else {
        return String::new();
    };
    text.chars()
        .zip(classify(text))
        .map(|(ch, class)| match ch {
            '\n' | '\t' => ch,
            _ if class == token => ch,
            _ => ' ',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class_of(text: &str, needle: &str) -> Token {
        let at = text.find(needle).unwrap();
        classify(text)[text[..at].chars().count()]
    }

    #[test]
    fn words_get_their_class() {
        let code =
            "uniform float k;\nvec4 border(vec2 uv) { return vec4(sin(umbriel_time) * 2.5e-1); }";
        assert_eq!(class_of(code, "uniform"), Token::Keyword);
        assert_eq!(class_of(code, "float"), Token::Type);
        assert_eq!(class_of(code, "border"), Token::Plain);
        assert_eq!(class_of(code, "sin"), Token::Builtin);
        assert_eq!(class_of(code, "umbriel_time"), Token::Umbriel);
        assert_eq!(class_of(code, "2.5e-1"), Token::Number);
        assert_eq!(class_of(code, "e-1"), Token::Number);
        assert_eq!(class_of(code, "return"), Token::Keyword);
    }

    #[test]
    fn comments_and_directives_cover_their_extent() {
        let code = "#define A 1 // note\nx; /* a\nb */ y; // float\n  #if Z";
        assert_eq!(class_of(code, "#define"), Token::Preproc);
        assert_eq!(class_of(code, "A 1"), Token::Preproc);
        assert_eq!(class_of(code, "// note"), Token::Comment);
        assert_eq!(class_of(code, "a\nb"), Token::Comment);
        assert_eq!(class_of(code, "b */"), Token::Comment);
        assert_eq!(class_of(code, "y;"), Token::Plain);
        assert_eq!(class_of(code, "float"), Token::Comment);
        assert_eq!(class_of(code, "#if"), Token::Preproc);
    }

    #[test]
    fn a_hash_after_code_is_not_a_directive() {
        assert_eq!(class_of("x # y", "#"), Token::Plain);
    }

    #[test]
    fn layers_lay_out_like_the_code_and_split_it_between_them() {
        let code = "// hi\n\tvec4 f(vec2 uv) {\n    return vec4(1.0);\n}\n";
        let layers: Vec<Vec<char>> = (0..TOKENS)
            .map(|index| layer(code, index).chars().collect())
            .collect();
        for (at, ch) in code.chars().enumerate() {
            let holders = layers.iter().filter(|copy| copy[at] == ch).count();
            for copy in &layers {
                assert!(copy[at] == ch || copy[at] == ' ');
            }
            // Whitespace shows in every copy; any other character in one.
            let expected = if ch.is_whitespace() { TOKENS } else { 1 };
            assert_eq!(holders, expected, "{ch:?} at {at}");
        }
        assert!(layers.iter().all(|copy| copy.len() == code.chars().count()));
    }

    #[test]
    fn the_gutter_numbers_every_line() {
        assert_eq!(layer("a\nb\n", LINE_NUMBERS), "1\n2\n3");
        assert_eq!(layer("", LINE_NUMBERS), "1");
        assert_eq!(layer("x", LINE_NUMBERS + 1), "");
    }
}
