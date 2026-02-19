use regex::Regex;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub line: usize,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Function,
    Struct,
    Trait,
    Enum,
    Impl,
    Const,
    Type,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolKind::Function => write!(f, "fn"),
            SymbolKind::Struct => write!(f, "struct"),
            SymbolKind::Trait => write!(f, "trait"),
            SymbolKind::Enum => write!(f, "enum"),
            SymbolKind::Impl => write!(f, "impl"),
            SymbolKind::Const => write!(f, "const"),
            SymbolKind::Type => write!(f, "type"),
        }
    }
}

pub fn extract_symbols(file: &Path, content: &str) -> Vec<Symbol> {
    let file_str = file.to_string_lossy().to_string();
    let mut symbols = Vec::new();

    let patterns: &[(&str, SymbolKind)] = &[
        (
            r"(?:pub(?:\([^)]*\))?\s+)?fn\s+(\w+)\s*[<(]",
            SymbolKind::Function,
        ),
        (
            r"(?:pub(?:\([^)]*\))?\s+)?struct\s+(\w+)",
            SymbolKind::Struct,
        ),
        (r"(?:pub(?:\([^)]*\))?\s+)?trait\s+(\w+)", SymbolKind::Trait),
        (r"(?:pub(?:\([^)]*\))?\s+)?enum\s+(\w+)", SymbolKind::Enum),
        (r"impl(?:<[^>]*>)?\s+(?:\w+::)*(\w+)", SymbolKind::Impl),
        (r"(?:pub(?:\([^)]*\))?\s+)?const\s+(\w+)", SymbolKind::Const),
        (r"(?:pub(?:\([^)]*\))?\s+)?type\s+(\w+)", SymbolKind::Type),
    ];

    let compiled: Vec<(Regex, &SymbolKind)> = patterns
        .iter()
        .filter_map(|(p, k)| Regex::new(p).ok().map(|r| (r, k)))
        .collect();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("*") {
            continue;
        }
        for (re, kind) in &compiled {
            if let Some(cap) = re.captures(trimmed) {
                let name = cap[1].to_string();
                symbols.push(Symbol {
                    name,
                    kind: (*kind).clone(),
                    file: file_str.clone(),
                    line: line_num + 1,
                    signature: trimmed.chars().take(120).collect(),
                });
                break;
            }
        }
    }

    symbols
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_function() {
        let content = "pub fn chat(messages: Vec<Message>) -> AiResponse {\n    todo!()\n}";
        let symbols = extract_symbols(Path::new("test.rs"), content);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "chat");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
    }

    #[test]
    fn test_extract_struct() {
        let content = "pub struct AiClient {\n    config: Config,\n}";
        let symbols = extract_symbols(Path::new("test.rs"), content);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "AiClient");
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn test_skip_comments() {
        let content = "// fn should_skip() {}\nfn real() {}";
        let symbols = extract_symbols(Path::new("test.rs"), content);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "real");
    }
}
