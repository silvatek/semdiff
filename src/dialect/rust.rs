/// Code identifiers in a Rust source.
///
/// Keywords are not identifiers, and neither are names written in comments or strings.
pub(super) fn identifiers(source: &str) -> Vec<String> {
    scan(source, None).0
}

/// Rust source with one code identifier renamed.
///
/// A keyword is left unchanged, as are comments and strings.
pub(super) fn rename_identifier(source: &str, from: &str, to: &str) -> String {
    scan(source, Some((from, to))).1
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

fn scan(source: &str, rename: Option<(&str, &str)>) -> (Vec<String>, String) {
    let mut index = 0;
    let mut found = Vec::new();
    let mut output = String::new();
    while index < source.len() {
        if source[index..].starts_with("//") {
            copy_line_comment(source, &mut index, &mut output);
            continue;
        }
        if source[index..].starts_with("/*") {
            copy_block_comment(source, &mut index, &mut output);
            continue;
        }
        if peek(source, index) == Some('\'') {
            copy_char_or_lifetime(source, &mut index, &mut output);
            continue;
        }
        if peek(source, index) == Some('"') {
            copy_cooked_string(source, &mut index, &mut output);
            continue;
        }
        if matches!(peek(source, index), Some(character) if is_ident_start(character)) {
            let start = index;
            let ident = read_ident(source, &mut index);
            if take_literal(source, &mut index, &mut output, &ident, start) {
                continue;
            }
            record_ident(&mut found, &mut output, &ident, rename);
            continue;
        }
        copy_one(source, &mut index, &mut output);
    }
    (found, output)
}

fn record_ident(
    identifiers: &mut Vec<String>,
    output: &mut String,
    ident: &str,
    rename: Option<(&str, &str)>,
) {
    if is_keyword(ident) {
        output.push_str(ident);
        return;
    }
    identifiers.push(ident.to_string());
    if let Some((from, to)) = rename {
        if ident == from {
            output.push_str(to);
            return;
        }
    }
    output.push_str(ident);
}

fn take_literal(
    source: &str,
    index: &mut usize,
    output: &mut String,
    ident: &str,
    ident_start: usize,
) -> bool {
    let after_ident = *index;
    if ident == "r" && matches!(peek(source, *index), Some('"' | '#')) {
        let mut cursor = *index;
        let hashes = count_hashes(source, &mut cursor);
        if peek(source, cursor) == Some('"') {
            output.push_str(&source[ident_start..cursor]);
            *index = cursor;
            copy_raw_string(source, index, output, hashes);
            return true;
        }
        return false;
    }
    if matches!(ident, "br" | "cr") && matches!(peek(source, *index), Some('"' | '#')) {
        let mut cursor = *index;
        let hashes = count_hashes(source, &mut cursor);
        if peek(source, cursor) == Some('"') {
            output.push_str(&source[ident_start..cursor]);
            *index = cursor;
            copy_raw_string(source, index, output, hashes);
            return true;
        }
        return false;
    }
    if matches!(ident, "b" | "c") {
        if peek(source, *index) == Some('"') {
            output.push_str(&source[ident_start..after_ident]);
            copy_cooked_string(source, index, output);
            return true;
        }
        if peek(source, *index) == Some('\'') {
            output.push_str(&source[ident_start..after_ident]);
            copy_char_or_lifetime(source, index, output);
            return true;
        }
        if source[*index..].starts_with("r\"") || source[*index..].starts_with("r#") {
            let mut cursor = *index + 1;
            let hashes = count_hashes(source, &mut cursor);
            if peek(source, cursor) == Some('"') {
                output.push_str(&source[ident_start..cursor]);
                *index = cursor;
                copy_raw_string(source, index, output, hashes);
                return true;
            }
        }
    }
    false
}

fn copy_line_comment(source: &str, index: &mut usize, output: &mut String) {
    while *index < source.len() {
        let character = peek(source, *index);
        copy_one(source, index, output);
        if character == Some('\n') {
            break;
        }
    }
}

fn copy_block_comment(source: &str, index: &mut usize, output: &mut String) {
    let start = *index;
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
    output.push_str(&source[start..*index]);
}

fn copy_cooked_string(source: &str, index: &mut usize, output: &mut String) {
    let start = *index;
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
    output.push_str(&source[start..*index]);
}

fn copy_raw_string(source: &str, index: &mut usize, output: &mut String, hashes: usize) {
    let start = *index;
    let _ = eat(source, index);
    let marker = "#".repeat(hashes);
    while *index < source.len() {
        if peek(source, *index) == Some('"') && source[*index + 1..].starts_with(&marker) {
            *index += 1 + hashes;
            break;
        }
        let _ = eat(source, index);
    }
    output.push_str(&source[start..*index]);
}

fn copy_char_or_lifetime(source: &str, index: &mut usize, output: &mut String) {
    let start = *index;
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
        output.push_str(&source[start..*index]);
        return;
    }
    if matches!(peek(source, *index), Some(character) if is_ident_start(character)) {
        let name = read_ident(source, index);
        if name.chars().count() == 1 && peek(source, *index) == Some('\'') {
            let _ = eat(source, index);
        }
    }
    output.push_str(&source[start..*index]);
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

fn copy_one(source: &str, index: &mut usize, output: &mut String) {
    let start = *index;
    let _ = eat(source, index);
    output.push_str(&source[start..*index]);
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
    use super::{identifiers, is_keyword, rename_identifier, KEYWORDS};

    #[test]
    fn keywords_are_not_identifiers() {
        let mut sorted = KEYWORDS.to_vec();
        sorted.sort_unstable();
        assert_eq!(KEYWORDS, sorted.as_slice());
        assert!(is_keyword("fn"));
        assert!(!is_keyword("fibonacci"));

        let names = identifiers("fn fibonacci() { let n = fibonacci; }\n");
        assert_eq!(names, ["fibonacci", "n", "fibonacci"]);
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
        let names: Vec<_> = identifiers(source)
            .into_iter()
            .filter(|name| name == "fibonacci")
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
        let renamed = rename_identifier(source, "fibonacci", "fib");
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
            rename_identifier("fn fibonacci() {}\n", "fn", "function"),
            "fn fibonacci() {}\n"
        );
    }
}
