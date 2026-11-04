use super::{Span, SpanKind};

/// Classified spans in a Rust source.
///
/// A keyword is not an identifier, and a name inside a comment or string is not one either.
pub(super) fn spans(source: &str) -> Vec<Span> {
    scan(source)
}

const KEYWORDS: &[&str] = &[
    "Self", "abstract", "as", "async", "await", "become", "box", "break", "const", "continue",
    "crate", "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if",
    "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

fn is_keyword(name: &str) -> bool {
    KEYWORDS.binary_search(&name).is_ok()
}

fn scan(source: &str) -> Vec<Span> {
    let mut index = 0;
    let mut spans = Vec::new();
    while index < source.len() {
        if source[index..].starts_with("//") {
            let start = index;
            skip_line_comment(source, &mut index);
            spans.push(Span {
                start,
                end: index,
                kind: SpanKind::Comment,
            });
            continue;
        }
        if source[index..].starts_with("/*") {
            let start = index;
            skip_block_comment(source, &mut index);
            spans.push(Span {
                start,
                end: index,
                kind: SpanKind::Comment,
            });
            continue;
        }
        if peek(source, index) == Some('\'') {
            let start = index;
            if skip_char_or_lifetime(source, &mut index) {
                spans.push(Span {
                    start,
                    end: index,
                    kind: SpanKind::String,
                });
            }
            continue;
        }
        if peek(source, index) == Some('"') {
            let start = index;
            skip_cooked_string(source, &mut index);
            spans.push(Span {
                start,
                end: index,
                kind: SpanKind::String,
            });
            continue;
        }
        if matches!(peek(source, index), Some(character) if is_ident_start(character)) {
            let start = index;
            let ident = read_ident(source, &mut index);
            if skip_literal(source, &mut index, &ident) {
                spans.push(Span {
                    start,
                    end: index,
                    kind: SpanKind::String,
                });
                continue;
            }
            spans.push(Span {
                start,
                end: index,
                kind: if is_keyword(&ident) {
                    SpanKind::Keyword
                } else {
                    SpanKind::Identifier
                },
            });
            continue;
        }
        let _ = eat(source, &mut index);
    }
    spans
}

fn skip_literal(source: &str, index: &mut usize, ident: &str) -> bool {
    if ident == "r" && matches!(peek(source, *index), Some('"' | '#')) {
        let mut cursor = *index;
        let hashes = count_hashes(source, &mut cursor);
        if peek(source, cursor) == Some('"') {
            *index = cursor;
            skip_raw_string(source, index, hashes);
            return true;
        }
        return false;
    }
    if matches!(ident, "br" | "cr") && matches!(peek(source, *index), Some('"' | '#')) {
        let mut cursor = *index;
        let hashes = count_hashes(source, &mut cursor);
        if peek(source, cursor) == Some('"') {
            *index = cursor;
            skip_raw_string(source, index, hashes);
            return true;
        }
        return false;
    }
    if matches!(ident, "b" | "c") {
        if peek(source, *index) == Some('"') {
            skip_cooked_string(source, index);
            return true;
        }
        if peek(source, *index) == Some('\'') {
            let _ = skip_char_or_lifetime(source, index);
            return true;
        }
        if source[*index..].starts_with("r\"") || source[*index..].starts_with("r#") {
            let mut cursor = *index + 1;
            let hashes = count_hashes(source, &mut cursor);
            if peek(source, cursor) == Some('"') {
                *index = cursor;
                skip_raw_string(source, index, hashes);
                return true;
            }
        }
    }
    false
}

fn skip_line_comment(source: &str, index: &mut usize) {
    while *index < source.len() {
        let character = eat(source, index);
        if character == Some('\n') {
            break;
        }
    }
}

fn skip_block_comment(source: &str, index: &mut usize) {
    *index += 2;
    let mut depth = 1;
    while *index < source.len() && depth > 0 {
        if source[*index..].starts_with("/*") {
            depth += 1;
            *index += 2;
        } else if source[*index..].starts_with("*/") {
            depth -= 1;
            *index += 2;
        } else {
            let _ = eat(source, index);
        }
    }
}

fn skip_cooked_string(source: &str, index: &mut usize) {
    let _ = eat(source, index);
    while *index < source.len() {
        match peek(source, *index) {
            Some('\\') => {
                let _ = eat(source, index);
                let _ = eat(source, index);
            }
            Some('"') => {
                let _ = eat(source, index);
                break;
            }
            Some(_) => {
                let _ = eat(source, index);
            }
            None => break,
        }
    }
}

fn skip_raw_string(source: &str, index: &mut usize, hashes: usize) {
    let _ = eat(source, index);
    let marker = "#".repeat(hashes);
    while *index < source.len() {
        if peek(source, *index) == Some('"') && source[*index + 1..].starts_with(&marker) {
            *index += 1 + hashes;
            break;
        }
        let _ = eat(source, index);
    }
}

fn skip_char_or_lifetime(source: &str, index: &mut usize) -> bool {
    let _ = eat(source, index);
    if peek(source, *index) == Some('\\') {
        let _ = eat(source, index);
        if peek(source, *index) == Some('u') {
            let _ = eat(source, index);
            if peek(source, *index) == Some('{') {
                while *index < source.len() && peek(source, *index) != Some('}') {
                    let _ = eat(source, index);
                }
                if peek(source, *index) == Some('}') {
                    let _ = eat(source, index);
                }
            }
        } else {
            let _ = eat(source, index);
        }
        if peek(source, *index) == Some('\'') {
            let _ = eat(source, index);
        }
        return true;
    }
    if matches!(peek(source, *index), Some(character) if is_ident_start(character)) {
        let name = read_ident(source, index);
        if name.chars().count() == 1 && peek(source, *index) == Some('\'') {
            let _ = eat(source, index);
            return true;
        }
    }
    false
}

fn count_hashes(source: &str, index: &mut usize) -> usize {
    let mut count = 0;
    while peek(source, *index) == Some('#') {
        count += 1;
        *index += 1;
    }
    count
}

fn read_ident(source: &str, index: &mut usize) -> String {
    let start = *index;
    if matches!(peek(source, *index), Some(character) if is_ident_start(character)) {
        let _ = eat(source, index);
        while matches!(peek(source, *index), Some(character) if is_ident_continue(character)) {
            let _ = eat(source, index);
        }
    }
    source[start..*index].to_string()
}

fn eat(source: &str, index: &mut usize) -> Option<char> {
    let character = peek(source, *index)?;
    *index += character.len_utf8();
    Some(character)
}

fn peek(source: &str, index: usize) -> Option<char> {
    source[index..].chars().next()
}

fn is_ident_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

fn is_ident_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

#[cfg(test)]
mod tests {
    use super::super::SpanKind;
    use super::{is_keyword, spans, KEYWORDS};

    fn identifier_names(source: &str) -> Vec<&str> {
        spans(source)
            .into_iter()
            .filter(|span| span.kind == SpanKind::Identifier)
            .map(|span| &source[span.start..span.end])
            .collect()
    }

    #[test]
    fn keywords_are_not_identifiers() {
        let mut sorted = KEYWORDS.to_vec();
        sorted.sort_unstable();
        assert_eq!(KEYWORDS, sorted.as_slice());
        assert!(is_keyword("fn"));
        assert!(!is_keyword("fibonacci"));
        assert_eq!(
            identifier_names("fn fibonacci() { let n = fibonacci; }\n"),
            ["fibonacci", "n", "fibonacci"]
        );
    }

    #[test]
    fn identifiers_skip_comments_and_strings() {
        let source = concat!(
            "fn fibonacci() {\n",
            "    /* fibonacci /* still */ fibonacci */\n",
            "    \"fibonacci\"\n",
            "    r#\"fibonacci\"#\n",
            "    // fibonacci\n",
            "    fibonacci()\n",
            "}\n",
        );
        let names: Vec<_> = identifier_names(source)
            .into_iter()
            .filter(|name| *name == "fibonacci")
            .collect();
        assert_eq!(names, ["fibonacci", "fibonacci"]);
    }

    #[test]
    fn rename_keeps_keywords_comments_and_strings() {
        let source = concat!(
            "fn fibonacci() {\n",
            "    // fibonacci\n",
            "    let s = \"fibonacci\";\n",
            "    println!(\"{n}: {}\", fibonacci(n));\n",
            "}\n",
        );
        let renamed = replace_identifiers(source, "fibonacci", "fib");
        assert_eq!(
            renamed,
            concat!(
                "fn fib() {\n",
                "    // fibonacci\n",
                "    let s = \"fibonacci\";\n",
                "    println!(\"{n}: {}\", fib(n));\n",
                "}\n",
            )
        );
        assert_eq!(
            replace_identifiers("fn fibonacci() {}\n", "fn", "function"),
            "fn fibonacci() {}\n"
        );
    }

    fn replace_identifiers(source: &str, from: &str, to: &str) -> String {
        let mut renamed = String::new();
        let mut cursor = 0;
        for span in spans(source) {
            if span.kind == SpanKind::Identifier && &source[span.start..span.end] == from {
                renamed.push_str(&source[cursor..span.start]);
                renamed.push_str(to);
                cursor = span.end;
            }
        }
        renamed.push_str(&source[cursor..]);
        renamed
    }
}
