use crate::format::{new_ulid, Block, BlockSet, Fileset, Index, Symbols, Tail};
use std::collections::BTreeSet;
use std::path::Path;
use tree_sitter::{Language, Node, Parser};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceLanguage {
    Python,
    Rust,
    Cpp,
}

impl SourceLanguage {
    pub fn detect(path: &Path, explicit: Option<&str>) -> Result<Self, String> {
        if let Some(language) = explicit.filter(|value| !value.is_empty()) {
            return match language.to_ascii_lowercase().as_str() {
                "python" | "py" => Ok(Self::Python),
                "rust" | "rs" => Ok(Self::Rust),
                "cpp" | "c++" | "cc" | "cxx" => Ok(Self::Cpp),
                other => Err(format!("unsupported language: {other}")),
            };
        }
        match path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("py") | Some("pyw") => Ok(Self::Python),
            Some("rs") => Ok(Self::Rust),
            Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") | Some("hh")
            | Some("hxx") | Some("h") => Ok(Self::Cpp),
            _ => Err(format!(
                "cannot detect language from {}; pass --lang python|rust|cpp",
                path.display()
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Rust => "rust",
            Self::Cpp => "cpp",
        }
    }

    fn grammar(self) -> Language {
        match self {
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
        }
    }
}

pub fn extract_symbols(src: &[u8], language: SourceLanguage) -> Symbols {
    let mut defines = BTreeSet::new();
    let mut uses = BTreeSet::new();
    let mut parser = Parser::new();
    if parser.set_language(&language.grammar()).is_ok() {
        if let Some(tree) = parser.parse(src, None) {
            collect_symbols(
                tree.root_node(),
                src,
                language,
                &mut defines,
                &mut uses,
            );
        }
    }
    Symbols {
        defines: defines.into_iter().collect(),
        uses: uses.into_iter().collect(),
    }
}

fn collect_symbols(
    node: Node<'_>,
    src: &[u8],
    language: SourceLanguage,
    defines: &mut BTreeSet<String>,
    uses: &mut BTreeSet<String>,
) {
    if is_identifier_kind(node.kind()) {
        if let Ok(identifier) = node.utf8_text(src) {
            if !identifier.is_empty() {
                uses.insert(identifier.to_string());
            }
        }
    }

    if is_definition_kind(language, node.kind()) {
        let name_node = node
            .child_by_field_name("name")
            .or_else(|| node.child_by_field_name("declarator").and_then(declarator_name));
        if let Some(name_node) = name_node {
            if let Ok(name) = name_node.utf8_text(src) {
                if !name.is_empty() {
                    defines.insert(name.to_string());
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_symbols(child, src, language, defines, uses);
    }
}

fn last_identifier<'tree>(node: Node<'tree>) -> Option<Node<'tree>> {
    let mut result = is_identifier_kind(node.kind()).then_some(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(identifier) = last_identifier(child) {
            result = Some(identifier);
        }
    }
    result
}

fn declarator_name<'tree>(node: Node<'tree>) -> Option<Node<'tree>> {
    if is_identifier_kind(node.kind()) && node.kind() != "scoped_identifier" {
        return Some(node);
    }
    if let Some(name) = node.child_by_field_name("name") {
        return declarator_name(name).or_else(|| last_identifier(name));
    }
    if let Some(declarator) = node.child_by_field_name("declarator") {
        return declarator_name(declarator).or_else(|| last_identifier(declarator));
    }
    last_identifier(node)
}

fn is_identifier_kind(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "type_identifier"
            | "field_identifier"
            | "namespace_identifier"
            | "scoped_identifier"
    )
}

fn is_definition_kind(language: SourceLanguage, kind: &str) -> bool {
    match language {
        SourceLanguage::Python => matches!(kind, "function_definition" | "class_definition"),
        SourceLanguage::Rust => matches!(
            kind,
            "function_item"
                | "struct_item"
                | "enum_item"
                | "trait_item"
                | "type_item"
                | "const_item"
                | "static_item"
                | "mod_item"
                | "macro_definition"
        ),
        SourceLanguage::Cpp => matches!(
            kind,
            "function_definition"
                | "class_specifier"
                | "struct_specifier"
                | "union_specifier"
                | "enum_specifier"
                | "namespace_definition"
        ),
    }
}

pub fn split_source(
    src: &[u8],
    root: &Path,
    posix_path: &str,
    name: &str,
    purpose: &str,
    language: SourceLanguage,
) -> Result<BlockSet, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar())
        .map_err(|error| format!("tree-sitter {} language load failed: {error}", language.name()))?;
    let tree = parser
        .parse(src, None)
        .ok_or_else(|| "tree-sitter parse failed".to_string())?;
    let root_node = tree.root_node();
    if root_node.has_error() {
        return Err(format!(
            "tree-sitter {} parse contains syntax errors; fix the source or language selection before splitting",
            language.name()
        ));
    }
    let mut boundaries = vec![0usize];
    let mut cursor = root_node.walk();
    let children: Vec<Node<'_>> = root_node.named_children(&mut cursor).collect();
    for (index, child) in children.iter().copied().enumerate() {
        if is_top_level_boundary(language, child) {
            boundaries.push(boundary_start(src, &children, index, language));
        }
    }
    boundaries.push(src.len());
    boundaries.sort_unstable();
    boundaries.dedup();

    let fileset_ulid = new_ulid();
    let mut blocks = Vec::new();
    let mut code = Vec::new();
    let mut byte_offset = 0u64;

    for window in boundaries.windows(2) {
        let start = window[0];
        let end = window[1];
        if end <= start {
            continue;
        }
        let data = &src[start..end];
        let seq = blocks.len() + 1;
        let signature = first_nonblank_line_bytes(data);
        let summary = if signature.is_empty() {
            format!("(seq {seq})")
        } else {
            String::from_utf8_lossy(signature).into_owned()
        };
        let byte_length = data.len() as u64;
        blocks.push(Block {
            ulid: new_ulid(),
            seq,
            byte_offset,
            byte_length,
            line_start: 0,
            line_end: 0,
            tail: Tail {
                purpose: String::new(),
                summary,
            },
            symbols: extract_symbols(data, language),
        });
        code.extend_from_slice(data);
        byte_offset += byte_length;
    }

    let fileset = Fileset {
        ulid: fileset_ulid,
        name: name.to_string(),
        path: posix_path.to_string(),
        lang: language.name().to_string(),
        purpose: purpose.to_string(),
        breakdown: blocks
            .iter()
            .map(|block| block.tail.summary.clone())
            .collect(),
        source_sha256: String::new(),
    };
    let mut blockset = BlockSet {
        root: root.to_path_buf(),
        index: Index {
            rev: 1,
            fileset,
            blocks,
            deleted: Vec::new(),
        },
        code,
    };
    blockset.recount_lines();
    Ok(blockset)
}

fn boundary_start(
    src: &[u8],
    children: &[Node<'_>],
    item_index: usize,
    language: SourceLanguage,
) -> usize {
    let mut start = children[item_index].start_byte();
    let mut index = item_index;
    while index > 0 {
        let previous = children[index - 1];
        let kind = previous.kind();
        let is_attribute = language == SourceLanguage::Rust && kind == "attribute_item";
        let is_comment = kind.contains("comment");
        if !is_attribute && !is_comment {
            break;
        }
        let gap = &src[previous.end_byte()..start];
        if is_comment && gap.iter().filter(|byte| **byte == b'\n').count() > 1 {
            break;
        }
        start = previous.start_byte();
        index -= 1;
    }
    start
}

fn is_top_level_boundary(language: SourceLanguage, node: Node<'_>) -> bool {
    match language {
        SourceLanguage::Python => matches!(
            node.kind(),
            "decorated_definition" | "function_definition" | "class_definition"
        ),
        SourceLanguage::Rust => matches!(
            node.kind(),
            "function_item"
                | "struct_item"
                | "enum_item"
                | "impl_item"
                | "trait_item"
                | "type_item"
                | "mod_item"
                | "macro_definition"
        ),
        SourceLanguage::Cpp => match node.kind() {
            "function_definition"
            | "template_declaration"
            | "namespace_definition"
            | "linkage_specification"
            | "class_specifier"
            | "struct_specifier"
            | "union_specifier"
            | "enum_specifier" => true,
            "declaration" => contains_type_definition(node),
            _ => false,
        },
    }
}

fn contains_type_definition(node: Node<'_>) -> bool {
    if matches!(
        node.kind(),
        "class_specifier" | "struct_specifier" | "union_specifier" | "enum_specifier"
    ) {
        return true;
    }
    let mut cursor = node.walk();
    let contains = node
        .named_children(&mut cursor)
        .any(contains_type_definition);
    contains
}

fn first_nonblank_line_bytes(data: &[u8]) -> &[u8] {
    let mut start = 0usize;
    let mut index = 0usize;
    while index < data.len() {
        if data[index] == b'\n' {
            let mut end = index;
            while end > start && matches!(data[end - 1], b'\n' | b'\r') {
                end -= 1;
            }
            if data[start..end].iter().any(|byte| !byte.is_ascii_whitespace()) {
                return &data[start..end];
            }
            start = index + 1;
        }
        index += 1;
    }
    let mut end = data.len();
    while end > start && matches!(data[end - 1], b'\n' | b'\r') {
        end -= 1;
    }
    &data[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_round_trip(
        source: &[u8],
        language: SourceLanguage,
        minimum_blocks: usize,
    ) -> Result<(), String> {
        let blockset = split_source(
            source,
            Path::new("."),
            "src/example",
            "example",
            "test",
            language,
        )?;
        assert_eq!(blockset.code, source);
        assert!(blockset.index.blocks.len() >= minimum_blocks);
        assert_eq!(blockset.index.fileset.lang, language.name());
        Ok(())
    }

    #[test]
    fn splits_python_functions() -> Result<(), String> {
        assert_round_trip(
            b"import os\n\ndef first():\n    pass\n\ndef second():\n    pass\n",
            SourceLanguage::Python,
            3,
        )
    }

    #[test]
    fn splits_rust_items_and_extracts_symbols() -> Result<(), String> {
        let source = b"use std::path::Path;\n\npub struct App;\n\nimpl App {\n    pub fn run(&self) {}\n}\n\npub fn start() { App.run(); }\n";
        assert_round_trip(source, SourceLanguage::Rust, 4)?;
        let symbols = extract_symbols(source, SourceLanguage::Rust);
        assert!(symbols.defines.contains(&"App".to_string()));
        assert!(symbols.defines.contains(&"start".to_string()));
        Ok(())
    }

    #[test]
    fn splits_cpp_types_and_functions() -> Result<(), String> {
        let source = b"#include <string>\n\nclass App {\npublic:\n    void run();\n};\n\nvoid App::run() {}\n";
        assert_round_trip(source, SourceLanguage::Cpp, 3)?;
        let symbols = extract_symbols(source, SourceLanguage::Cpp);
        assert!(symbols.defines.contains(&"App".to_string()));
        assert!(symbols.defines.contains(&"run".to_string()));
        Ok(())
    }

    #[test]
    fn keeps_rust_documentation_and_attributes_with_item() -> Result<(), String> {
        let source = b"use std::fmt;\n\n/// Public application.\n#[derive(Debug)]\npub struct App;\n\npub fn run() {}\n";
        let blockset = split_source(
            source,
            Path::new("."),
            "src/example.rs",
            "example.rs",
            "test",
            SourceLanguage::Rust,
        )?;
        let app_block = blockset
            .index
            .blocks
            .iter()
            .find(|block| block.symbols.defines.iter().any(|symbol| symbol == "App"))
            .ok_or_else(|| "App block not found".to_string())?;
        let app_source = blockset.block_bytes(app_block);
        assert!(app_source.starts_with(b"/// Public application.\n#[derive(Debug)]"));
        assert_eq!(blockset.code, source);
        Ok(())
    }

    #[test]
    fn detects_supported_extensions() {
        assert_eq!(
            SourceLanguage::detect(Path::new("src/lib.rs"), None),
            Ok(SourceLanguage::Rust)
        );
        assert_eq!(
            SourceLanguage::detect(Path::new("src/mainwindow.cpp"), None),
            Ok(SourceLanguage::Cpp)
        );
        assert!(SourceLanguage::detect(Path::new("README.md"), None).is_err());
    }

    #[test]
    fn rejects_syntax_errors() {
        let result = split_source(
            b"fn broken( {\n",
            Path::new("."),
            "src/broken.rs",
            "broken.rs",
            "test",
            SourceLanguage::Rust,
        );
        assert!(result.is_err());
    }
}
