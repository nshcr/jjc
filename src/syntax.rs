use tree_sitter::Node;
use tree_sitter::Parser;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionRange {
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
}

pub fn function_for_line(path: &std::path::Path, source: &str, line: usize) -> Option<String> {
    if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
        return None;
    }
    rust_functions(source)
        .into_iter()
        .find(|range| range.start_line <= line && line <= range.end_line)
        .map(|range| range.name)
}

fn rust_functions(source: &str) -> Vec<FunctionRange> {
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .is_err()
    {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };
    let mut ranges = Vec::new();
    collect_functions(tree.root_node(), source.as_bytes(), &mut ranges);
    ranges
}

fn collect_functions(node: Node<'_>, source: &[u8], ranges: &mut Vec<FunctionRange>) {
    if matches!(node.kind(), "function_item" | "function_signature_item")
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
}
