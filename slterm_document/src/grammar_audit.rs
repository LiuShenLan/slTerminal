//! 锁定 grammar 的 Windows 构建、ABI、真实高亮查询与最小样例审计。

use tree_sitter::{Language, Parser, Query};

// 两个旧 binding 未导出 LanguageFn；只在测试中把同一 C 入口适配到当前运行时。
unsafe extern "C" {
    fn tree_sitter_json5() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_dockerfile() -> *const tree_sitter::ffi::TSLanguage;
}

fn json5_language() -> Language {
    // 引用绑定 crate，确保其原生静态库参与测试链接。
    let _ = tree_sitter_json5::NODE_TYPES;
    // 安全性：符号由锁定的 tree-sitter-json5 grammar 提供，返回非空静态 TSLanguage。
    unsafe { Language::from_raw(tree_sitter_json5()) }
}

fn dockerfile_language() -> Language {
    // 引用绑定 crate，确保其原生静态库参与测试链接。
    let _ = tree_sitter_dockerfile::NODE_TYPES;
    // 安全性：符号由锁定的 tree-sitter-dockerfile grammar 提供，返回非空静态 TSLanguage。
    unsafe { Language::from_raw(tree_sitter_dockerfile()) }
}

fn audit_grammar(name: &str, language: Language, highlights: &str, sample: &str) {
    assert!(
        (13..=15).contains(&language.abi_version()),
        "{name} ABI {} 不在 13..=15",
        language.abi_version()
    );

    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .unwrap_or_else(|error| panic!("{name} grammar 加载失败：{error}"));

    let query = Query::new(&language, highlights)
        .unwrap_or_else(|error| panic!("{name} highlights query 编译失败：{error}"));
    assert!(
        !query.capture_names().is_empty(),
        "{name} highlights query 不含捕获"
    );

    let tree = parser
        .parse(sample, None)
        .unwrap_or_else(|| panic!("{name} 最小样例解析未返回语法树"));
    assert!(
        !tree.root_node().has_error(),
        "{name} 最小样例存在语法错误：{}",
        tree.root_node().to_sexp()
    );
}

macro_rules! audit_grammar {
    ($test_name:ident, $name:literal, $language:expr, $highlights:expr, $sample:expr) => {
        #[test]
        fn $test_name() {
            audit_grammar($name, $language, $highlights, $sample);
        }
    };
}

#[test]
fn javascript_grammar_loads_with_real_highlights() {
    let highlights = [
        tree_sitter_javascript::HIGHLIGHT_QUERY,
        tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
    ]
    .join("\n");
    audit_grammar(
        "JavaScript",
        tree_sitter_javascript::LANGUAGE.into(),
        &highlights,
        "const element = <span>ok</span>;\n",
    );
}

audit_grammar!(
    typescript_grammar_loads_with_real_highlights,
    "TypeScript",
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    tree_sitter_typescript::HIGHLIGHTS_QUERY,
    "const value: number = 1;\n"
);
audit_grammar!(
    tsx_grammar_loads_with_real_highlights,
    "TSX",
    tree_sitter_typescript::LANGUAGE_TSX.into(),
    tree_sitter_typescript::HIGHLIGHTS_QUERY,
    "const element: JSX.Element = <span>ok</span>;\n"
);
audit_grammar!(
    python_grammar_loads_with_real_highlights,
    "Python",
    tree_sitter_python::LANGUAGE.into(),
    tree_sitter_python::HIGHLIGHTS_QUERY,
    "def answer():\n    return 42\n"
);
audit_grammar!(
    rust_grammar_loads_with_real_highlights,
    "Rust",
    tree_sitter_rust::LANGUAGE.into(),
    tree_sitter_rust::HIGHLIGHTS_QUERY,
    "fn main() { println!(\"ok\"); }\n"
);
audit_grammar!(
    json_grammar_loads_with_real_highlights,
    "JSON",
    tree_sitter_json::LANGUAGE.into(),
    tree_sitter_json::HIGHLIGHTS_QUERY,
    "{\"ok\":true}\n"
);
audit_grammar!(
    jsonc_grammar_loads_with_real_highlights,
    "JSONC",
    json5_language(),
    include_str!("grammar_audit_json5.scm"),
    "{unquoted: 1, /* comment */ trailing: true,}\n"
);
audit_grammar!(
    html_grammar_loads_with_real_highlights,
    "HTML",
    tree_sitter_html::LANGUAGE.into(),
    tree_sitter_html::HIGHLIGHTS_QUERY,
    "<!DOCTYPE html><html><body>ok</body></html>\n"
);
audit_grammar!(
    css_grammar_loads_with_real_highlights,
    "CSS",
    tree_sitter_css::LANGUAGE.into(),
    tree_sitter_css::HIGHLIGHTS_QUERY,
    "a { color: red; }\n"
);
audit_grammar!(
    scss_grammar_loads_with_real_highlights,
    "SCSS",
    tree_sitter_scss::language(),
    tree_sitter_scss::HIGHLIGHTS_QUERY,
    "$color: red;\na { color: $color; }\n"
);
audit_grammar!(
    less_grammar_loads_with_real_highlights,
    "Less",
    tree_sitter_less::language(),
    tree_sitter_less::HIGHLIGHTS_QUERY,
    "@color: red;\na { color: @color; }\n"
);
audit_grammar!(
    markdown_block_grammar_loads_with_real_highlights,
    "Markdown block",
    tree_sitter_md::LANGUAGE.into(),
    tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
    "# heading\n"
);
audit_grammar!(
    markdown_inline_grammar_loads_with_real_highlights,
    "Markdown inline",
    tree_sitter_md::INLINE_LANGUAGE.into(),
    tree_sitter_md::HIGHLIGHT_QUERY_INLINE,
    "**bold**\n"
);
audit_grammar!(
    xml_grammar_loads_with_real_highlights,
    "XML",
    tree_sitter_xml::LANGUAGE_XML.into(),
    tree_sitter_xml::XML_HIGHLIGHT_QUERY,
    "<root>ok</root>\n"
);
audit_grammar!(
    shell_grammar_loads_with_real_highlights,
    "Shell",
    tree_sitter_bash::LANGUAGE.into(),
    tree_sitter_bash::HIGHLIGHT_QUERY,
    "echo \"ok\"\n"
);
audit_grammar!(
    powershell_grammar_loads_with_real_highlights,
    "PowerShell",
    tree_sitter_powershell::LANGUAGE.into(),
    tree_sitter_powershell::HIGHLIGHTS_QUERY,
    "Write-Output \"ok\"\n"
);
audit_grammar!(
    go_grammar_loads_with_real_highlights,
    "Go",
    tree_sitter_go::LANGUAGE.into(),
    tree_sitter_go::HIGHLIGHTS_QUERY,
    "package main\nfunc main() {}\n"
);
audit_grammar!(
    java_grammar_loads_with_real_highlights,
    "Java",
    tree_sitter_java::LANGUAGE.into(),
    tree_sitter_java::HIGHLIGHTS_QUERY,
    "class Main {}\n"
);
audit_grammar!(
    c_grammar_loads_with_real_highlights,
    "C",
    tree_sitter_c::LANGUAGE.into(),
    tree_sitter_c::HIGHLIGHT_QUERY,
    "int main(void) { return 0; }\n"
);
audit_grammar!(
    cpp_grammar_loads_with_real_highlights,
    "C++",
    tree_sitter_cpp::LANGUAGE.into(),
    tree_sitter_cpp::HIGHLIGHT_QUERY,
    "int main() { return 0; }\n"
);
audit_grammar!(
    c_sharp_grammar_loads_with_real_highlights,
    "C#",
    tree_sitter_c_sharp::LANGUAGE.into(),
    tree_sitter_c_sharp::HIGHLIGHTS_QUERY,
    "class Program {}\n"
);
audit_grammar!(
    sql_grammar_loads_with_real_highlights,
    "SQL",
    tree_sitter_sequel::LANGUAGE.into(),
    tree_sitter_sequel::HIGHLIGHTS_QUERY,
    "SELECT 1;\n"
);
audit_grammar!(
    yaml_grammar_loads_with_real_highlights,
    "YAML",
    tree_sitter_yaml::LANGUAGE.into(),
    tree_sitter_yaml::HIGHLIGHTS_QUERY,
    "key: value\n"
);
audit_grammar!(
    toml_grammar_loads_with_real_highlights,
    "TOML",
    tree_sitter_toml_ng::LANGUAGE.into(),
    tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
    "key = \"value\"\n"
);
audit_grammar!(
    ini_grammar_loads_with_real_highlights,
    "INI",
    tree_sitter_ini::LANGUAGE.into(),
    tree_sitter_ini::HIGHLIGHTS_QUERY,
    "[section]\nkey = value\n"
);
audit_grammar!(
    diff_grammar_loads_with_real_highlights,
    "Diff",
    tree_sitter_diff::LANGUAGE.into(),
    tree_sitter_diff::HIGHLIGHTS_QUERY,
    "--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+new\n"
);
audit_grammar!(
    dockerfile_grammar_loads_with_real_highlights,
    "Dockerfile",
    dockerfile_language(),
    include_str!("grammar_audit_dockerfile.scm"),
    "FROM alpine:3.22\n"
);
audit_grammar!(
    make_grammar_loads_with_real_highlights,
    "Make",
    tree_sitter_make::LANGUAGE.into(),
    tree_sitter_make::HIGHLIGHTS_QUERY,
    "all:\n\t@echo ok\n"
);
