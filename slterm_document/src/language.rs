//! 封闭语言注册表与真实 Tree-sitter 查询映射。

use std::path::Path;

use tree_sitter::Language;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LanguageId {
    Rust,
    Markdown,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
    Jsx,
    C,
    Cpp,
    CSharp,
    Go,
    Java,
    Bash,
    PowerShell,
    Toml,
    Yaml,
    Json,
    Jsonc,
    Html,
    Css,
    Scss,
    Less,
    Xml,
    Sql,
    Ini,
    Diff,
    Dockerfile,
    Make,
    PlainText,
}

impl LanguageId {
    pub const ALL: [Self; 29] = [
        Self::Rust,
        Self::Markdown,
        Self::Python,
        Self::JavaScript,
        Self::TypeScript,
        Self::Tsx,
        Self::Jsx,
        Self::C,
        Self::Cpp,
        Self::CSharp,
        Self::Go,
        Self::Java,
        Self::Bash,
        Self::PowerShell,
        Self::Toml,
        Self::Yaml,
        Self::Json,
        Self::Jsonc,
        Self::Html,
        Self::Css,
        Self::Scss,
        Self::Less,
        Self::Xml,
        Self::Sql,
        Self::Ini,
        Self::Diff,
        Self::Dockerfile,
        Self::Make,
        Self::PlainText,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Markdown => "markdown",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::Jsx => "jsx",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Go => "go",
            Self::Java => "java",
            Self::Bash => "bash",
            Self::PowerShell => "powershell",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::Json => "json",
            Self::Jsonc => "jsonc",
            Self::Html => "html",
            Self::Css => "css",
            Self::Scss => "scss",
            Self::Less => "less",
            Self::Xml => "xml",
            Self::Sql => "sql",
            Self::Ini => "ini",
            Self::Diff => "diff",
            Self::Dockerfile => "dockerfile",
            Self::Make => "make",
            Self::PlainText => "text",
        }
    }
}

pub struct LanguageDescriptor {
    id: LanguageId,
    extensions: &'static [&'static str],
    filenames: &'static [&'static str],
    markdown_aliases: &'static [&'static str],
    grammar: fn() -> Option<Language>,
    highlights: fn() -> Option<String>,
    folds: Option<&'static str>,
}

impl LanguageDescriptor {
    pub const fn id(&self) -> LanguageId {
        self.id
    }

    pub fn grammar(&self) -> Option<Language> {
        (self.grammar)()
    }

    pub fn highlight_query(&self) -> Option<String> {
        (self.highlights)()
    }

    pub const fn fold_query(&self) -> Option<&'static str> {
        self.folds
    }

    pub const fn markdown_aliases(&self) -> &'static [&'static str] {
        self.markdown_aliases
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LanguageRegistry;

impl LanguageRegistry {
    pub const fn new() -> Self {
        Self
    }

    /// 扩展名与固定文件名均按 Windows 语义忽略 ASCII 大小写；未知类型不猜测。
    pub fn for_path(&self, path: &Path) -> Option<LanguageId> {
        let file_name = path.file_name()?.to_str()?.to_ascii_lowercase();
        if let Some(descriptor) = DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.filenames.contains(&file_name.as_str()))
        {
            return Some(descriptor.id);
        }
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.extensions.contains(&extension.as_str()))
            .map(|descriptor| descriptor.id)
    }

    /// Markdown 围栏别名只归一到封闭语言，不推断未注册类型。
    pub fn for_markdown_alias(&self, alias: &str) -> Option<LanguageId> {
        let alias = alias.to_ascii_lowercase();
        DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.markdown_aliases.contains(&alias.as_str()))
            .map(|descriptor| descriptor.id)
    }

    pub fn descriptor(&self, id: LanguageId) -> &'static LanguageDescriptor {
        DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.id == id)
            .expect("LanguageId 必须登记在封闭描述符表")
    }
}

macro_rules! descriptor {
    (
        $id:ident,
        [$($extension:literal),* $(,)?],
        [$($filename:literal),* $(,)?],
        [$($alias:literal),* $(,)?],
        $grammar:expr,
        $highlights:expr,
        $folds:expr
    ) => {
        LanguageDescriptor {
            id: LanguageId::$id,
            extensions: &[$($extension),*],
            filenames: &[$($filename),*],
            markdown_aliases: &[$($alias),*],
            grammar: $grammar,
            highlights: $highlights,
            folds: Some($folds),
        }
    };
}

const FOLD_BLOCK: &str = "(block) @fold";
const FOLD_COMPOUND: &str = "(compound_statement) @fold";
const FOLD_WEB: &str = "(statement_block) @fold\n(class_body) @fold\n(object) @fold";
const FOLD_DATA: &str = "(object) @fold\n(array) @fold";

static DESCRIPTORS: &[LanguageDescriptor] = &[
    descriptor!(
        Rust,
        ["rs"],
        [],
        ["rust", "rs"],
        || Some(tree_sitter_rust::LANGUAGE.into()),
        || Some(tree_sitter_rust::HIGHLIGHTS_QUERY.to_owned()),
        "(block) @fold\n(struct_item) @fold\n(enum_item) @fold"
    ),
    descriptor!(
        Markdown,
        ["md", "markdown", "mdown", "mkd"],
        [],
        ["md", "markdown", "mdown", "mkd"],
        || Some(tree_sitter_md::LANGUAGE.into()),
        || Some(tree_sitter_md::HIGHLIGHT_QUERY_BLOCK.to_owned()),
        "(section) @fold"
    ),
    descriptor!(
        Python,
        ["py", "pyi", "pyw"],
        [],
        ["python", "py", "pyi", "pyw"],
        || Some(tree_sitter_python::LANGUAGE.into()),
        || Some(tree_sitter_python::HIGHLIGHTS_QUERY.to_owned()),
        FOLD_BLOCK
    ),
    descriptor!(
        JavaScript,
        ["js", "mjs", "cjs"],
        [],
        ["javascript", "js", "mjs", "cjs", "node"],
        || Some(tree_sitter_javascript::LANGUAGE.into()),
        || Some(tree_sitter_javascript::HIGHLIGHT_QUERY.to_owned()),
        FOLD_WEB
    ),
    descriptor!(
        TypeScript,
        ["ts", "mts", "cts"],
        [],
        ["typescript", "ts", "mts", "cts"],
        || Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        || Some(tree_sitter_typescript::HIGHLIGHTS_QUERY.to_owned()),
        FOLD_WEB
    ),
    descriptor!(
        Tsx,
        ["tsx"],
        [],
        ["tsx"],
        || Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        || Some(tree_sitter_typescript::HIGHLIGHTS_QUERY.to_owned()),
        FOLD_WEB
    ),
    descriptor!(
        Jsx,
        ["jsx"],
        [],
        ["jsx"],
        || Some(tree_sitter_javascript::LANGUAGE.into()),
        || Some(format!(
            "{}\n{}",
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::JSX_HIGHLIGHT_QUERY
        )),
        FOLD_WEB
    ),
    descriptor!(
        C,
        ["c", "h"],
        [],
        ["c", "h"],
        || Some(tree_sitter_c::LANGUAGE.into()),
        || Some(tree_sitter_c::HIGHLIGHT_QUERY.to_owned()),
        FOLD_COMPOUND
    ),
    descriptor!(
        Cpp,
        ["cpp", "cc", "cxx", "hpp", "hh"],
        [],
        ["cpp", "c++", "cc", "cxx", "hpp", "hh"],
        || Some(tree_sitter_cpp::LANGUAGE.into()),
        || Some(tree_sitter_cpp::HIGHLIGHT_QUERY.to_owned()),
        FOLD_COMPOUND
    ),
    descriptor!(
        CSharp,
        ["cs"],
        [],
        ["csharp", "cs", "c#"],
        || Some(tree_sitter_c_sharp::LANGUAGE.into()),
        || Some(tree_sitter_c_sharp::HIGHLIGHTS_QUERY.to_owned()),
        "(block) @fold\n(class_declaration) @fold"
    ),
    descriptor!(
        Go,
        ["go"],
        [],
        ["go", "golang"],
        || Some(tree_sitter_go::LANGUAGE.into()),
        || Some(tree_sitter_go::HIGHLIGHTS_QUERY.to_owned()),
        "(block) @fold\n(struct_type) @fold"
    ),
    descriptor!(
        Java,
        ["java"],
        [],
        ["java"],
        || Some(tree_sitter_java::LANGUAGE.into()),
        || Some(tree_sitter_java::HIGHLIGHTS_QUERY.to_owned()),
        "(block) @fold\n(class_body) @fold"
    ),
    descriptor!(
        Bash,
        ["sh", "bash", "zsh"],
        [],
        ["bash", "sh", "shell", "zsh"],
        || Some(tree_sitter_bash::LANGUAGE.into()),
        || Some(tree_sitter_bash::HIGHLIGHT_QUERY.to_owned()),
        "(compound_statement) @fold\n(function_definition) @fold"
    ),
    descriptor!(
        PowerShell,
        ["ps1", "psm1", "psd1"],
        [],
        ["powershell", "ps1", "psm1", "psd1", "pwsh"],
        || Some(tree_sitter_powershell::LANGUAGE.into()),
        || Some(tree_sitter_powershell::HIGHLIGHTS_QUERY.to_owned()),
        "(statement_block) @fold"
    ),
    descriptor!(
        Toml,
        ["toml"],
        [],
        ["toml"],
        || Some(tree_sitter_toml_ng::LANGUAGE.into()),
        || Some(tree_sitter_toml_ng::HIGHLIGHTS_QUERY.to_owned()),
        "(table) @fold\n(array) @fold"
    ),
    descriptor!(
        Yaml,
        ["yaml", "yml"],
        [],
        ["yaml", "yml"],
        || Some(tree_sitter_yaml::LANGUAGE.into()),
        || Some(tree_sitter_yaml::HIGHLIGHTS_QUERY.to_owned()),
        "(block_mapping) @fold\n(block_sequence) @fold"
    ),
    descriptor!(
        Json,
        ["json", "jsonl", "ndjson"],
        [],
        ["json", "jsonl", "ndjson"],
        || Some(tree_sitter_json::LANGUAGE.into()),
        || Some(tree_sitter_json::HIGHLIGHTS_QUERY.to_owned()),
        FOLD_DATA
    ),
    descriptor!(
        Jsonc,
        ["jsonc", "json5"],
        [],
        ["jsonc", "json5"],
        || Some(tree_sitter_json5::LANGUAGE.into()),
        || Some(tree_sitter_json5::HIGHLIGHTS_QUERY.to_owned()),
        FOLD_DATA
    ),
    descriptor!(
        Html,
        ["html", "htm"],
        [],
        ["html", "htm"],
        || Some(tree_sitter_html::LANGUAGE.into()),
        || Some(tree_sitter_html::HIGHLIGHTS_QUERY.to_owned()),
        "(element) @fold\n(script_element) @fold\n(style_element) @fold"
    ),
    descriptor!(
        Css,
        ["css"],
        [],
        ["css"],
        || Some(tree_sitter_css::LANGUAGE.into()),
        || Some(tree_sitter_css::HIGHLIGHTS_QUERY.to_owned()),
        FOLD_BLOCK
    ),
    descriptor!(
        Scss,
        ["scss"],
        [],
        ["scss"],
        || Some(tree_sitter_scss::language()),
        || Some(tree_sitter_scss::HIGHLIGHTS_QUERY.to_owned()),
        FOLD_BLOCK
    ),
    descriptor!(
        Less,
        ["less"],
        [],
        ["less"],
        || Some(tree_sitter_less::language()),
        || Some(tree_sitter_less::HIGHLIGHTS_QUERY.to_owned()),
        FOLD_BLOCK
    ),
    descriptor!(
        Xml,
        ["xml", "svg"],
        [],
        ["xml", "svg"],
        || Some(tree_sitter_xml::LANGUAGE_XML.into()),
        || Some(tree_sitter_xml::XML_HIGHLIGHT_QUERY.to_owned()),
        "(element) @fold"
    ),
    descriptor!(
        Sql,
        ["sql"],
        [],
        ["sql"],
        || Some(tree_sitter_sequel::LANGUAGE.into()),
        || Some(tree_sitter_sequel::HIGHLIGHTS_QUERY.to_owned()),
        "(statement) @fold"
    ),
    descriptor!(
        Ini,
        ["ini", "cfg", "conf"],
        [],
        ["ini", "cfg", "conf"],
        || Some(tree_sitter_ini::LANGUAGE.into()),
        || Some(tree_sitter_ini::HIGHLIGHTS_QUERY.to_owned()),
        "(section) @fold"
    ),
    descriptor!(
        Diff,
        ["diff", "patch"],
        [],
        ["diff", "patch"],
        || Some(tree_sitter_diff::LANGUAGE.into()),
        || Some(tree_sitter_diff::HIGHLIGHTS_QUERY.to_owned()),
        "(hunk) @fold"
    ),
    descriptor!(
        Dockerfile,
        ["dockerfile"],
        ["dockerfile"],
        ["dockerfile", "docker"],
        || Some(tree_sitter_dockerfile::LANGUAGE.into()),
        || Some(tree_sitter_dockerfile::HIGHLIGHTS_QUERY.to_owned()),
        "(heredoc_block) @fold"
    ),
    descriptor!(
        Make,
        ["mk"],
        ["makefile", "gnumakefile"],
        ["make", "mk", "makefile", "gnumakefile"],
        || Some(tree_sitter_make::LANGUAGE.into()),
        || Some(tree_sitter_make::HIGHLIGHTS_QUERY.to_owned()),
        "(rule) @fold"
    ),
    LanguageDescriptor {
        id: LanguageId::PlainText,
        extensions: &["txt", "log", "text"],
        filenames: &[],
        markdown_aliases: &["text", "txt", "log", "plain", "plaintext"],
        grammar: || None,
        highlights: || None,
        folds: None,
    },
];

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tree_sitter::Query;

    use crate::{LanguageId, LanguageRegistry};

    #[test]
    fn language_registry_resolves_paths_and_markdown_aliases() {
        let registry = LanguageRegistry::new();
        for (path, expected) in [
            ("main.rs", LanguageId::Rust),
            ("README.MD", LanguageId::Markdown),
            ("script.PS1", LanguageId::PowerShell),
            ("window.pyw", LanguageId::Python),
            ("icon.svg", LanguageId::Xml),
            ("Dockerfile", LanguageId::Dockerfile),
            ("Makefile", LanguageId::Make),
            ("config.JSONC", LanguageId::Jsonc),
            ("config.json5", LanguageId::Jsonc),
            ("notes.txt", LanguageId::PlainText),
        ] {
            assert_eq!(registry.for_path(Path::new(path)), Some(expected), "{path}");
        }
        assert_eq!(registry.for_path(Path::new("unknown.rb")), None);
        let markdown = registry.descriptor(LanguageId::Markdown);
        assert!(markdown.markdown_aliases().contains(&"md"));
        assert!(markdown.markdown_aliases().contains(&"markdown"));
    }

    #[test]
    fn language_registry_resolves_markdown_aliases_case_insensitively() {
        let registry = LanguageRegistry::new();
        for (alias, expected) in [
            ("rust", LanguageId::Rust),
            ("RS", LanguageId::Rust),
            ("py", LanguageId::Python),
            ("js", LanguageId::JavaScript),
            ("TS", LanguageId::TypeScript),
            ("c++", LanguageId::Cpp),
            ("cs", LanguageId::CSharp),
            ("shell", LanguageId::Bash),
            ("ps1", LanguageId::PowerShell),
            ("yml", LanguageId::Yaml),
            ("json5", LanguageId::Jsonc),
            ("svg", LanguageId::Xml),
            ("docker", LanguageId::Dockerfile),
            ("makefile", LanguageId::Make),
            ("plain", LanguageId::PlainText),
        ] {
            assert_eq!(
                registry.for_markdown_alias(alias),
                Some(expected),
                "{alias}"
            );
        }
        assert_eq!(registry.for_markdown_alias("not-a-language"), None);
        for id in LanguageId::ALL {
            assert!(
                registry
                    .descriptor(id)
                    .markdown_aliases()
                    .contains(&id.as_str()),
                "{id:?} 必须登记规范围栏别名"
            );
        }
    }

    #[test]
    fn every_registered_language_has_real_highlight_and_fold_queries() {
        let registry = LanguageRegistry::new();
        for id in LanguageId::ALL {
            let descriptor = registry.descriptor(id);
            if id == LanguageId::PlainText {
                assert!(descriptor.grammar().is_none());
                continue;
            }
            let language = descriptor.grammar().unwrap();
            let highlights = Query::new(&language, &descriptor.highlight_query().unwrap())
                .unwrap_or_else(|error| panic!("{id:?} highlights 无效：{error}"));
            assert!(!highlights.capture_names().is_empty(), "{id:?}");
            let folds = Query::new(&language, descriptor.fold_query().unwrap())
                .unwrap_or_else(|error| panic!("{id:?} folds 无效：{error}"));
            assert!(folds.capture_names().contains(&"fold"), "{id:?}");
        }
    }
}
