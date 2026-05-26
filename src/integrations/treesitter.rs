use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor, Tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightKind {
    Keyword,
    Type,
    Function,
    String,
    Number,
    Comment,
    Operator,
    Variable,
    Constant,
    Attribute,
    Punctuation,
}

#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub kind: HighlightKind,
}

struct LangConfig {
    language: Language,
    query: Query,
    capture_map: Vec<HighlightKind>,
}

pub struct Highlighter {
    parser: Parser,
    config: Option<LangConfig>,
    tree: Option<Tree>,
    pub spans: Vec<HighlightSpan>,
}

const HIGHLIGHT_QUERY: &str = r#"
(line_comment) @comment
(block_comment) @comment
(string_literal) @string
(raw_string_literal) @string
(interpreted_string_literal) @string
(rune_literal) @string
(char_literal) @string
(integer_literal) @number
(float_literal) @number
(boolean_literal) @constant
(true) @constant
(false) @constant
(nil) @constant
(type_identifier) @type
(primitive_type) @type
(field_identifier) @variable
(identifier) @variable
(function_item name: (identifier) @function)
(call_expression function: (identifier) @function)
(function_declaration name: (identifier) @function)
(call_expression function: (identifier) @function)
(method_declaration name: (field_identifier) @function)
"#;

const KEYWORD_NAMES: &[&str] = &[
    "fn", "let", "mut", "pub", "use", "mod", "struct", "enum", "impl", "trait",
    "for", "while", "loop", "if", "else", "match", "return", "break", "continue",
    "const", "static", "type", "where", "async", "await", "move", "ref", "self",
    "super", "crate", "as", "in", "true", "false",
    "func", "var", "package", "import", "defer", "go", "chan", "select",
    "switch", "case", "default", "range", "map", "interface", "nil",
    "def", "class", "from", "import", "pass", "yield", "lambda", "with",
    "try", "except", "finally", "raise", "global", "nonlocal",
    "function", "var", "let", "const", "new", "delete", "typeof", "instanceof",
    "throw", "catch", "finally", "yield", "of",
];

impl Highlighter {
    pub fn new() -> Self {
        Self {
            parser: Parser::new(),
            config: None,
            tree: None,
            spans: Vec::new(),
        }
    }

    pub fn set_language_for_file(&mut self, path: &Path) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let language = match ext {
            "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
            "go" => Some(tree_sitter_go::LANGUAGE.into()),
            "js" | "jsx" | "ts" | "tsx" | "mjs" => {
                Some(tree_sitter_javascript::LANGUAGE.into())
            }
            "py" => Some(tree_sitter_python::LANGUAGE.into()),
            "json" => Some(tree_sitter_json::LANGUAGE.into()),
            "toml" => Some(tree_sitter_toml_ng::LANGUAGE.into()),
            _ => None,
        };

        if let Some(lang) = language {
            self.parser.set_language(&lang).ok();

            if let Ok(query) = Query::new(&lang, HIGHLIGHT_QUERY) {
                let capture_map: Vec<HighlightKind> = query
                    .capture_names()
                    .iter()
                    .map(|name| match &**name {
                        "comment" => HighlightKind::Comment,
                        "string" => HighlightKind::String,
                        "number" => HighlightKind::Number,
                        "constant" => HighlightKind::Constant,
                        "type" => HighlightKind::Type,
                        "function" => HighlightKind::Function,
                        "variable" => HighlightKind::Variable,
                        "keyword" => HighlightKind::Keyword,
                        "operator" => HighlightKind::Operator,
                        "attribute" => HighlightKind::Attribute,
                        _ => HighlightKind::Variable,
                    })
                    .collect();

                self.config = Some(LangConfig {
                    language: lang,
                    query,
                    capture_map,
                });
            } else {
                self.config = None;
            }
        } else {
            self.config = None;
        }
        self.tree = None;
        self.spans.clear();
    }

    pub fn parse(&mut self, source: &str) {
        let Some(config) = &self.config else {
            self.spans.clear();
            return;
        };

        let tree = self.parser.parse(source, self.tree.as_ref());
        let Some(tree) = tree else {
            return;
        };

        let mut spans = Vec::new();
        let mut cursor = QueryCursor::new();

        let mut matches = cursor.matches(&config.query, tree.root_node(), source.as_bytes());
        loop {
            matches.advance();
            let Some(m) = matches.get() else { break };
            for cap in m.captures {
                let node = cap.node;
                let kind = config.capture_map.get(cap.index as usize).copied()
                    .unwrap_or(HighlightKind::Variable);

                spans.push(HighlightSpan {
                    start_byte: node.start_byte(),
                    end_byte: node.end_byte(),
                    start_line: node.start_position().row,
                    start_col: node.start_position().column,
                    end_line: node.end_position().row,
                    end_col: node.end_position().column,
                    kind,
                });
            }
        }

        self.apply_keyword_highlights(source, &mut spans);

        spans.sort_by_key(|s| (s.start_byte, std::cmp::Reverse(s.end_byte)));
        self.spans = spans;
        self.tree = Some(tree);
    }

    fn apply_keyword_highlights(&self, source: &str, spans: &mut Vec<HighlightSpan>) {
        for (line_idx, line) in source.lines().enumerate() {
            let line_start_byte = source[..source.lines().take(line_idx).map(|l| l.len() + 1).sum::<usize>()].len();
            let _ = line_start_byte; // We compute col-based positions

            let mut col = 0;
            for word in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if !word.is_empty() && KEYWORD_NAMES.contains(&word) {
                    let word_start_col = line[col..].find(word).map(|i| col + i).unwrap_or(col);
                    let already_highlighted = spans.iter().any(|s| {
                        s.start_line == line_idx
                            && s.start_col <= word_start_col
                            && s.end_col >= word_start_col + word.len()
                            && matches!(s.kind, HighlightKind::Function | HighlightKind::Type)
                    });

                    if !already_highlighted {
                        spans.push(HighlightSpan {
                            start_byte: 0,
                            end_byte: 0,
                            start_line: line_idx,
                            start_col: word_start_col,
                            end_line: line_idx,
                            end_col: word_start_col + word.len(),
                            kind: HighlightKind::Keyword,
                        });
                    }
                }
                col += word.len() + 1;
            }
        }
    }

    pub fn get_highlight_at(&self, line: usize, col: usize) -> Option<HighlightKind> {
        for span in &self.spans {
            if line < span.start_line || line > span.end_line {
                continue;
            }
            if line == span.start_line && col < span.start_col {
                continue;
            }
            if line == span.end_line && col >= span.end_col {
                continue;
            }
            return Some(span.kind);
        }
        None
    }
}

impl HighlightKind {
    pub fn to_color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            HighlightKind::Keyword => Color::Rgb(198, 120, 221),   // purple
            HighlightKind::Type => Color::Rgb(229, 192, 123),      // yellow
            HighlightKind::Function => Color::Rgb(97, 175, 239),   // blue
            HighlightKind::String => Color::Rgb(152, 195, 121),    // green
            HighlightKind::Number => Color::Rgb(209, 154, 102),    // orange
            HighlightKind::Comment => Color::Rgb(92, 99, 112),     // gray
            HighlightKind::Operator => Color::Rgb(86, 182, 194),   // cyan
            HighlightKind::Variable => Color::Rgb(171, 178, 191),  // light gray
            HighlightKind::Constant => Color::Rgb(209, 154, 102),  // orange
            HighlightKind::Attribute => Color::Rgb(229, 192, 123), // yellow
            HighlightKind::Punctuation => Color::Rgb(171, 178, 191),
        }
    }
}
