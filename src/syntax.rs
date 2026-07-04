use tree_sitter::Language;
use tree_sitter::Node;
use tree_sitter::Parser;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HighlightClass {
    Comment,
    Function,
    Keyword,
    Number,
    String,
    Type,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HighlightSpan {
    pub text: String,
    pub class: Option<HighlightClass>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionRange {
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
}

pub fn function_for_line(path: &std::path::Path, source: &str, line: usize) -> Option<String> {
    let language = language_for_path(path)?;
    functions(source, language.language)?
        .into_iter()
        .find(|range| range.start_line <= line && line <= range.end_line)
        .map(|range| range.name)
}

pub fn highlight_lines(
    path: &std::path::Path,
    lines: &[String],
) -> Option<Vec<Vec<HighlightSpan>>> {
    let language = language_for_path(path)?;
    let source = lines.join("\n");
    let mut parser = parser(language.language)?;
    let tree = parser.parse(&source, None)?;
    let mut ranges = lexical_highlight_ranges(lines, language.keywords);
    ranges.extend(highlight_ranges(tree.root_node()));
    Some(split_highlight_lines(lines, &ranges))
}

pub fn language_name_for_path(path: &std::path::Path) -> Option<&'static str> {
    Some(language_for_path(path)?.name)
}

pub fn supported_languages() -> &'static [&'static str] {
    &[
        "c",
        "cpp",
        "go",
        "javascript",
        "json",
        "python",
        "rust",
        "tsx",
        "typescript",
    ]
}

fn functions(source: &str, language: fn() -> Language) -> Option<Vec<FunctionRange>> {
    let mut parser = parser(language)?;
    let tree = parser.parse(source, None)?;
    let mut ranges = Vec::new();
    collect_functions(tree.root_node(), source.as_bytes(), &mut ranges);
    Some(ranges)
}

fn collect_functions(node: Node<'_>, source: &[u8], ranges: &mut Vec<FunctionRange>) {
    if is_function_like(node.kind())
        && let Some(name) = node.child_by_field_name("name")
        && let Ok(name) = name.utf8_text(source)
    {
        ranges.push(FunctionRange {
            name: name.to_owned(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_functions(child, source, ranges);
    }
}

fn parser(language: fn() -> Language) -> Option<Parser> {
    let mut parser = Parser::new();
    parser.set_language(&language()).ok()?;
    Some(parser)
}

#[derive(Clone, Copy)]
struct LanguageSpec {
    name: &'static str,
    language: fn() -> Language,
    extensions: &'static [&'static str],
    filenames: &'static [&'static str],
    keywords: &'static [&'static str],
}

const LANGUAGES: &[LanguageSpec] = &[
    LanguageSpec {
        name: "rust",
        language: rust_language,
        extensions: &["rs"],
        filenames: &[],
        keywords: RUST_KEYWORDS,
    },
    LanguageSpec {
        name: "python",
        language: python_language,
        extensions: &["py", "pyw"],
        filenames: &[],
        keywords: PYTHON_KEYWORDS,
    },
    LanguageSpec {
        name: "go",
        language: go_language,
        extensions: &["go"],
        filenames: &[],
        keywords: GO_KEYWORDS,
    },
    LanguageSpec {
        name: "javascript",
        language: javascript_language,
        extensions: &["js", "mjs", "cjs"],
        filenames: &[],
        keywords: JAVASCRIPT_KEYWORDS,
    },
    LanguageSpec {
        name: "typescript",
        language: typescript_language,
        extensions: &["ts", "mts", "cts"],
        filenames: &[],
        keywords: TYPESCRIPT_KEYWORDS,
    },
    LanguageSpec {
        name: "tsx",
        language: tsx_language,
        extensions: &["tsx", "jsx"],
        filenames: &[],
        keywords: TYPESCRIPT_KEYWORDS,
    },
    LanguageSpec {
        name: "json",
        language: json_language,
        extensions: &["json"],
        filenames: &["package-lock.json", "tsconfig.json"],
        keywords: JSON_KEYWORDS,
    },
    LanguageSpec {
        name: "c",
        language: c_language,
        extensions: &["c", "h"],
        filenames: &[],
        keywords: C_KEYWORDS,
    },
    LanguageSpec {
        name: "cpp",
        language: cpp_language,
        extensions: &["cc", "cpp", "cxx", "hpp", "hh", "hxx"],
        filenames: &[],
        keywords: CPP_KEYWORDS,
    },
];

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while",
];

const PYTHON_KEYWORDS: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
    "else", "except", "False", "finally", "for", "from", "global", "if", "import", "in", "is",
    "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return", "True", "try", "while",
    "with", "yield",
];

const GO_KEYWORDS: &[&str] = &[
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "nil",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
];

const JAVASCRIPT_KEYWORDS: &[&str] = &[
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "from",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "let",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

const TYPESCRIPT_KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "asserts",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "declare",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "from",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "infer",
    "instanceof",
    "interface",
    "is",
    "keyof",
    "let",
    "module",
    "namespace",
    "new",
    "null",
    "private",
    "protected",
    "public",
    "readonly",
    "return",
    "satisfies",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "type",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

const JSON_KEYWORDS: &[&str] = &["false", "null", "true"];

const C_KEYWORDS: &[&str] = &[
    "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
    "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long", "register",
    "restrict", "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
    "union", "unsigned", "void", "volatile", "while",
];

const CPP_KEYWORDS: &[&str] = &[
    "alignas",
    "alignof",
    "and",
    "asm",
    "auto",
    "bool",
    "break",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "constexpr",
    "continue",
    "decltype",
    "default",
    "delete",
    "do",
    "double",
    "else",
    "enum",
    "explicit",
    "export",
    "extern",
    "false",
    "float",
    "for",
    "friend",
    "if",
    "inline",
    "int",
    "long",
    "namespace",
    "new",
    "noexcept",
    "nullptr",
    "operator",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "struct",
    "switch",
    "template",
    "this",
    "throw",
    "true",
    "try",
    "typedef",
    "typename",
    "union",
    "unsigned",
    "using",
    "virtual",
    "void",
    "volatile",
    "while",
];

fn language_for_path(path: &std::path::Path) -> Option<LanguageSpec> {
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    LANGUAGES
        .iter()
        .copied()
        .find(|language| language.filenames.contains(&filename))
        .or_else(|| {
            LANGUAGES
                .iter()
                .copied()
                .find(|language| language.extensions.contains(&extension))
        })
}

fn rust_language() -> Language {
    tree_sitter_rust::LANGUAGE.into()
}

fn python_language() -> Language {
    tree_sitter_python::LANGUAGE.into()
}

fn go_language() -> Language {
    tree_sitter_go::LANGUAGE.into()
}

fn javascript_language() -> Language {
    tree_sitter_javascript::LANGUAGE.into()
}

fn typescript_language() -> Language {
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
}

fn tsx_language() -> Language {
    tree_sitter_typescript::LANGUAGE_TSX.into()
}

fn json_language() -> Language {
    tree_sitter_json::LANGUAGE.into()
}

fn c_language() -> Language {
    tree_sitter_c::LANGUAGE.into()
}

fn cpp_language() -> Language {
    tree_sitter_cpp::LANGUAGE.into()
}

#[derive(Clone, Copy)]
struct HighlightRange {
    line: usize,
    start: usize,
    end: usize,
    class: HighlightClass,
}

fn highlight_ranges(root: Node<'_>) -> Vec<HighlightRange> {
    let mut ranges = Vec::new();
    collect_highlights(root, &mut ranges);
    ranges
}

fn lexical_highlight_ranges(lines: &[String], keywords: &[&str]) -> Vec<HighlightRange> {
    let mut ranges = Vec::new();
    for (line_index, line) in lines.iter().enumerate() {
        let mut start = None;
        for (index, character) in line
            .char_indices()
            .chain(std::iter::once((line.len(), ' ')))
        {
            if is_identifier_char(character) {
                start.get_or_insert(index);
            } else if let Some(word_start) = start.take() {
                let word = &line[word_start..index];
                if keywords.contains(&word) {
                    ranges.push(HighlightRange {
                        line: line_index,
                        start: word_start,
                        end: index,
                        class: HighlightClass::Keyword,
                    });
                }
            }
        }
    }
    ranges
}

fn is_identifier_char(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn collect_highlights(node: Node<'_>, ranges: &mut Vec<HighlightRange>) {
    if let Some(class) = class_for_kind(node.kind()) {
        push_range(node, class, ranges);
    }
    if is_function_like(node.kind())
        && let Some(name) = node.child_by_field_name("name")
    {
        push_range(name, HighlightClass::Function, ranges);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_highlights(child, ranges);
    }
}

fn class_for_kind(kind: &str) -> Option<HighlightClass> {
    if kind.contains("comment") {
        return Some(HighlightClass::Comment);
    }
    if kind.contains("string")
        || matches!(
            kind,
            "char_literal"
                | "character_literal"
                | "interpreted_string_literal"
                | "raw_string_literal"
        )
    {
        return Some(HighlightClass::String);
    }
    if kind.contains("number")
        || matches!(
            kind,
            "float_literal" | "integer_literal" | "int_literal" | "imaginary_literal"
        )
    {
        return Some(HighlightClass::Number);
    }
    if matches!(
        kind,
        "primitive_type"
            | "type_identifier"
            | "type_parameter"
            | "qualified_type"
            | "generic_type"
            | "preproc_arg"
    ) {
        return Some(HighlightClass::Type);
    }
    match kind {
        "as" | "async" | "await" | "break" | "const" | "continue" | "crate" | "dyn" | "else"
        | "elif" | "enum" | "export" | "extends" | "extern" | "false" | "fn" | "for" | "from"
        | "func" | "function" | "go" | "if" | "impl" | "import" | "in" | "interface" | "let"
        | "loop" | "match" | "mod" | "move" | "mut" | "namespace" | "new" | "nil" | "null"
        | "package" | "pub" | "public" | "private" | "protected" | "ref" | "return" | "self"
        | "static" | "struct" | "super" | "switch" | "this" | "trait" | "true" | "type"
        | "typeof" | "unsafe" | "use" | "var" | "void" | "where" | "while" => {
            Some(HighlightClass::Keyword)
        }
        _ => None,
    }
}

fn is_function_like(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "function_signature_item"
            | "function_definition"
            | "function_declaration"
            | "method_definition"
            | "method_declaration"
            | "method_elem"
            | "arrow_function"
    )
}

fn push_range(node: Node<'_>, class: HighlightClass, ranges: &mut Vec<HighlightRange>) {
    let start = node.start_position();
    let end = node.end_position();
    if start.row == end.row {
        ranges.push(HighlightRange {
            line: start.row,
            start: start.column,
            end: end.column,
            class,
        });
        return;
    }

    ranges.push(HighlightRange {
        line: start.row,
        start: start.column,
        end: usize::MAX,
        class,
    });
    for line in (start.row + 1)..end.row {
        ranges.push(HighlightRange {
            line,
            start: 0,
            end: usize::MAX,
            class,
        });
    }
    ranges.push(HighlightRange {
        line: end.row,
        start: 0,
        end: end.column,
        class,
    });
}

fn split_highlight_lines(lines: &[String], ranges: &[HighlightRange]) -> Vec<Vec<HighlightSpan>> {
    lines
        .iter()
        .enumerate()
        .map(|(line_index, line)| {
            let mut classes = vec![None; line.len()];
            for range in ranges.iter().filter(|range| range.line == line_index) {
                for item in classes
                    .iter_mut()
                    .take(range.end.min(line.len()))
                    .skip(range.start.min(line.len()))
                {
                    *item = Some(range.class);
                }
            }
            split_line(line, &classes)
        })
        .collect()
}

fn split_line(line: &str, classes: &[Option<HighlightClass>]) -> Vec<HighlightSpan> {
    if line.is_empty() {
        return vec![HighlightSpan {
            text: String::new(),
            class: None,
        }];
    }

    let mut spans = Vec::new();
    let mut start = 0;
    let mut class = classes[0];
    for index in line
        .char_indices()
        .map(|(index, _)| index)
        .skip(1)
        .chain(std::iter::once(line.len()))
    {
        let next = classes.get(index).copied().unwrap_or(None);
        if next != class {
            spans.push(HighlightSpan {
                text: line[start..index].to_owned(),
                class,
            });
            start = index;
            class = next;
        }
    }
    if start < line.len() {
        spans.push(HighlightSpan {
            text: line[start..].to_owned(),
            class,
        });
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_rust_function_for_line() {
        let source = "fn a() {\n  let x = 1;\n}\n\nfn b() {}\n";
        assert_eq!(
            function_for_line(std::path::Path::new("lib.rs"), source, 2),
            Some("a".into())
        );
        assert_eq!(
            function_for_line(std::path::Path::new("lib.rs"), source, 5),
            Some("b".into())
        );
    }

    #[test]
    fn detects_supported_languages_by_path() {
        assert_eq!(
            language_name_for_path(std::path::Path::new("main.rs")),
            Some("rust")
        );
        assert_eq!(
            language_name_for_path(std::path::Path::new("app.py")),
            Some("python")
        );
        assert_eq!(
            language_name_for_path(std::path::Path::new("main.go")),
            Some("go")
        );
        assert_eq!(
            language_name_for_path(std::path::Path::new("component.tsx")),
            Some("tsx")
        );
        assert_eq!(
            language_name_for_path(std::path::Path::new("package-lock.json")),
            Some("json")
        );
    }

    #[test]
    fn highlights_rust_keyword_function_string_and_comment() {
        let lines = vec![
            "fn main() {".to_owned(),
            "    let s = \"hi\"; // greet".to_owned(),
            "}".to_owned(),
        ];

        let highlighted = highlight_lines(std::path::Path::new("main.rs"), &lines).unwrap();
        let flat = highlighted.into_iter().flatten().collect::<Vec<_>>();

        assert!(flat.contains(&span("fn", Some(HighlightClass::Keyword))));
        assert!(flat.contains(&span("main", Some(HighlightClass::Function))));
        assert!(flat.contains(&span("\"hi\"", Some(HighlightClass::String))));
        assert!(flat.contains(&span("// greet", Some(HighlightClass::Comment))));
    }

    #[test]
    fn highlights_non_rust_language() {
        let lines = vec![
            "def main():".to_owned(),
            "    return \"hi\" # greet".to_owned(),
        ];

        let highlighted = highlight_lines(std::path::Path::new("main.py"), &lines).unwrap();
        let flat = highlighted.into_iter().flatten().collect::<Vec<_>>();

        assert!(flat.contains(&span("def", Some(HighlightClass::Keyword))));
        assert!(flat.contains(&span("main", Some(HighlightClass::Function))));
        assert!(flat.contains(&span("\"hi\"", Some(HighlightClass::String))));
        assert!(flat.contains(&span("# greet", Some(HighlightClass::Comment))));
    }

    #[test]
    fn highlighted_spans_preserve_all_text() {
        let lines = vec!["fn main() {".to_owned(), "    value".to_owned()];

        let highlighted = highlight_lines(std::path::Path::new("main.rs"), &lines).unwrap();
        let rebuilt = highlighted
            .into_iter()
            .map(|line| line.into_iter().map(|span| span.text).collect::<String>())
            .collect::<Vec<_>>();

        assert_eq!(rebuilt, lines);
    }

    #[test]
    fn skips_non_rust_files() {
        assert!(
            highlight_lines(
                std::path::Path::new("COMMIT_EDITMSG"),
                &["fn x() {}".into()]
            )
            .is_none()
        );
    }

    fn span(text: &str, class: Option<HighlightClass>) -> HighlightSpan {
        HighlightSpan {
            text: text.to_owned(),
            class,
        }
    }
}
