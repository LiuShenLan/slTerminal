//! i18n 静态生成器:构建期把 `i18n/*.json` 编译成 typed `Message` 枚举与
//! 进程级 `MESSAGES` 表,运行时零解析、零文件读、零锁。
//!
//! 目录遍历由 `LanguagePref` 注册表驱动;合同全族在构建期强制:
//! - en-US 与 zh-CN 键集完全相同且非空(不等即构建失败);
//! - 每语言键集不得超出英文;命名占位符集合跨语言一致;
//! - message id 合法 + 变体名不碰撞;重复 id/空串/非字符串/嵌套与点号
//!   冲突全部拒绝;
//! - source_id 反查只在所有语言同义时生成(歧义短语不借翻译)。
//!
//! 部分目录构建期回退英文(缺失键直接填英文值),运行时不做回退逻辑;
//! 当前两语均走完整键集硬合同,回退分支是机制保留。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::Path;

use slterm_settings::LanguagePref;

#[path = "i18n/catalog.rs"]
mod catalog;

/// 构建期生成 `translations.rs`。
pub fn generate(directory: &Path, output: &Path) -> Result<(), String> {
    let mut catalogs = BTreeMap::new();
    for info in LanguagePref::LANGUAGES {
        let path = directory.join(format!("{}.json", info.code));
        println!("cargo:rerun-if-changed={}", path.display());
        let source = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let messages =
            catalog::parse(&source).map_err(|error| format!("{}: {error}", info.code))?;
        catalogs.insert(info.code, messages);
    }
    let english = &catalogs["en-US"];
    let chinese = &catalogs["zh-CN"];
    if english.is_empty() || !english.keys().eq(chinese.keys()) {
        return Err(
            "English and Simplified Chinese must have identical nonempty message ids".into(),
        );
    }
    for (locale, messages) in &catalogs {
        for (key, message) in messages {
            let base = english
                .get(key)
                .ok_or_else(|| format!("{locale}: unknown message id {key}"))?;
            if catalog::placeholders(base)? != catalog::placeholders(message)? {
                return Err(format!("{locale}: placeholder mismatch for {key}"));
            }
        }
    }
    let mut variants = BTreeSet::new();
    let keys = english.keys().collect::<Vec<_>>();
    let mut code = String::new();
    // 生成物是目录的机械投影:未被消费点引用的 ID/常量由 allow(dead_code)
    // 放行(目录可先于消费点亮;消费点随设置页/壳层迁移逐个接线)。
    code.push_str(
        "#[allow(dead_code)]\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n#[repr(usize)]\npub enum Message {\n",
    );
    for key in &keys {
        let variant = message_variant(key)?;
        if !variants.insert(variant.clone()) {
            return Err(format!("message variant collision: {key}"));
        }
        writeln!(code, "{variant},").unwrap();
    }
    code.push_str(
        "}\n#[allow(dead_code)]\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n#[repr(usize)]\npub enum UiLanguage {\n",
    );
    for info in LanguagePref::LANGUAGES {
        writeln!(code, "{},", info.rust_variant).unwrap();
    }
    code.push_str("}\n#[allow(dead_code)]\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]\npub enum LanguagePreference {\n#[default]\nSystem,\n");
    for info in LanguagePref::LANGUAGES {
        writeln!(code, "{},", info.rust_variant).unwrap();
    }
    code.push_str("}\n#[allow(dead_code)]\nimpl LanguagePreference {\npub const ALL: &'static [Self] = &[Self::System,");
    for info in LanguagePref::LANGUAGES {
        write!(code, "Self::{},", info.rust_variant).unwrap();
    }
    code.push_str("];\npub const fn shared(self) -> slterm_settings::LanguagePref { match self {\nSelf::System => slterm_settings::LanguagePref::System,\n");
    for info in LanguagePref::LANGUAGES {
        writeln!(
            code,
            "Self::{0} => slterm_settings::LanguagePref::{0},",
            info.rust_variant
        )
        .unwrap();
    }
    code.push_str("}}\npub const fn explicit(self) -> Option<UiLanguage> { match self {\nSelf::System => None,\n");
    for info in LanguagePref::LANGUAGES {
        writeln!(
            code,
            "Self::{0} => Some(UiLanguage::{0}),",
            info.rust_variant
        )
        .unwrap();
    }
    code.push_str("}}}\n#[allow(dead_code)]\nimpl From<slterm_settings::LanguagePref> for LanguagePreference {\nfn from(value: slterm_settings::LanguagePref) -> Self {match value {\nslterm_settings::LanguagePref::System => Self::System,\n");
    for info in LanguagePref::LANGUAGES {
        writeln!(
            code,
            "slterm_settings::LanguagePref::{0} => Self::{0},",
            info.rust_variant
        )
        .unwrap();
    }
    code.push_str(
        "}}}\n#[allow(dead_code)]\nimpl UiLanguage {\npub const ALL: &'static [Self] = &[",
    );
    for info in LanguagePref::LANGUAGES {
        write!(code, "Self::{},", info.rust_variant).unwrap();
    }
    code.push_str("];\npub const fn code(self) -> &'static str { match self {\n");
    for info in LanguagePref::LANGUAGES {
        writeln!(code, "Self::{} => {:?},", info.rust_variant, info.code).unwrap();
    }
    code.push_str("}}\npub const fn gpui_component_locale(self) -> &'static str { match self {\n");
    for info in LanguagePref::LANGUAGES {
        writeln!(
            code,
            "Self::{} => {:?},",
            info.rust_variant, info.component_locale
        )
        .unwrap();
    }
    code.push_str(
        "}}}\n#[allow(dead_code)]\nfn message_id(key: &str) -> Option<Message> { match key {\n",
    );
    for key in &keys {
        writeln!(code, "{key:?} => Some(Message::{}),", message_variant(key)?).unwrap();
    }
    code.push_str("_ => None,\n}}\n");
    let mut aliases = BTreeMap::new();
    let mut ambiguous = BTreeSet::new();
    for (index, key) in keys.iter().enumerate() {
        let source = &english[*key];
        if let Some(previous) = aliases.insert(source, index)
            && catalogs
                .values()
                .any(|messages| messages.get(*key) != messages.get(keys[previous]))
        {
            ambiguous.insert(source);
        }
    }
    code.push_str(
        "#[allow(dead_code)]\nfn source_id(english: &str) -> Option<Message> { match english {\n",
    );
    for (source, index) in aliases {
        if !ambiguous.contains(source) {
            writeln!(
                code,
                "{source:?} => Some(Message::{}),",
                message_variant(keys[index])?
            )
            .unwrap();
        }
    }
    code.push_str("_ => None,\n}}\n");
    // 常量数组在 debug profile 可能落到调用方栈上;共享查表固定一份静态分配。
    writeln!(
        code,
        "#[allow(dead_code)]\nstatic MESSAGES: [[&str; {}]; {}] = [",
        keys.len(),
        catalogs.len()
    )
    .unwrap();
    for info in LanguagePref::LANGUAGES {
        code.push_str("[\n");
        for key in &keys {
            let message = translated(&catalogs, english, info.code, key);
            writeln!(code, "{message:?},").unwrap();
        }
        code.push_str("],\n");
    }
    code.push_str("];\n");
    writeln!(
        code,
        "#[allow(dead_code)]\npub const MESSAGE_COUNT: usize = {};",
        keys.len()
    )
    .unwrap();
    let bytes = catalogs
        .values()
        .flat_map(|messages| messages.values())
        .map(String::len)
        .sum::<usize>();
    writeln!(
        code,
        "#[allow(dead_code)]\npub const TRANSLATED_BYTES: usize = {bytes};"
    )
    .unwrap();
    std::fs::write(output, code).map_err(|error| error.to_string())
}

/// 单条译文:该语言缺键时构建期填英文值(运行时无回退分支)。
fn translated<'a>(
    catalogs: &'a BTreeMap<&'static str, BTreeMap<String, String>>,
    english: &'a BTreeMap<String, String>,
    locale: &str,
    key: &str,
) -> &'a str {
    catalogs
        .get(locale)
        .and_then(|messages| messages.get(key))
        .or_else(|| english.get(key))
        .map(String::as_str)
        .unwrap_or_default()
}

/// 消息 id → Rust 枚举变体名(按 `.`/`_`/`-` 分段首字母大写)。
fn message_variant(key: &str) -> Result<String, String> {
    let mut variant = String::new();
    for part in key.split(['.', '_', '-']) {
        let mut letters = part.chars();
        let first = letters
            .next()
            .ok_or_else(|| format!("empty id segment: {key}"))?;
        if !first.is_ascii_alphabetic()
            || !letters.clone().all(|letter| letter.is_ascii_alphanumeric())
        {
            return Err(format!("invalid message id: {key}"));
        }
        variant.push(first.to_ascii_uppercase());
        variant.extend(letters);
    }
    Ok(variant)
}

#[cfg(test)]
mod generator_tests {
    use super::catalog;

    /// 临时目录内写两份目录并跑生成器,返回生成源码。
    fn fixture(english: &str, chinese: &str) -> Result<String, String> {
        static SEQUENCE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "slterm-catalog-test-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        for info in slterm_settings::LanguagePref::LANGUAGES {
            let source = match info.code {
                "en-US" => english,
                "zh-CN" => chinese,
                _ => "{}",
            };
            std::fs::write(root.join(format!("{}.json", info.code)), source).unwrap();
        }
        let output = root.join("generated.rs");
        let result = super::generate(&root, &output)
            .and_then(|()| std::fs::read_to_string(output).map_err(|error| error.to_string()));
        std::fs::remove_dir_all(&root).unwrap();
        result
    }

    /// 两语完整时生成 typed 枚举;每语各一行共享同一 id。
    #[test]
    fn complete_catalogs_generate_typed_messages() {
        let source = fixture(
            r#"{"greeting":"Hello {name}"}"#,
            r#"{"greeting":"你好 {name}"}"#,
        )
        .unwrap();
        assert!(source.contains("pub enum Message"));
        assert!(source.contains("Greeting,"));
        assert!(source.contains("\"Hello {name}\","));
        assert!(source.contains("\"你好 {name}\","));
    }

    /// 部分目录的回退机制在构建期取英文值(运行时不做回退)。
    #[test]
    fn partial_catalogs_fall_back_to_english_at_build_time() {
        let english =
            std::collections::BTreeMap::from([("greeting".to_owned(), "Hello".to_owned())]);
        let mut catalogs = std::collections::BTreeMap::new();
        catalogs.insert("en-US", english.clone());
        catalogs.insert("zh-CN", std::collections::BTreeMap::new());
        assert_eq!(
            super::translated(&catalogs, &english, "zh-CN", "greeting"),
            "Hello"
        );
        assert_eq!(
            super::translated(&catalogs, &english, "xx-XX", "greeting"),
            "Hello"
        );
    }

    /// 键集不等、空目录、未知键、占位符不一致、变体碰撞全部拒绝。
    #[test]
    fn rejects_unknown_keys_placeholder_mismatches_and_id_collisions() {
        let english = r#"{"greeting":"Hello {name}"}"#;
        assert!(
            fixture(english, r#"{"greeting":"你好 {name}","extra":"多余"}"#).is_err(),
            "每语言键集不得超出英文"
        );
        assert!(
            fixture(english, r#"{"other":"差异"}"#).is_err(),
            "两语键集必须全等"
        );
        assert!(fixture("{}", "{}").is_err(), "目录不得为空");
        assert!(
            fixture(english, r#"{"greeting":"你好 {user}"}"#).is_err(),
            "占位符集合跨语言一致"
        );
        let colliding = r#"{"a_b":"First","a-b":"Second"}"#;
        assert!(fixture(colliding, colliding).is_err(), "变体名碰撞必须拒绝");
        assert!(
            fixture(r#"{"bad..id":"A"}"#, r#"{"bad..id":"B"}"#).is_err(),
            "非法 id 必须拒绝"
        );
    }

    /// 同一英文短语在不同语言里译义不同 → 不生成 source_id 反查。
    #[test]
    fn ambiguous_source_phrases_require_contextual_ids() {
        let source = fixture(
            r#"{"verb":"Open","state":"Open"}"#,
            r#"{"verb":"打开","state":"打开状态"}"#,
        )
        .unwrap();
        assert!(!source.contains("\"Open\" => Some("));
        assert!(source.contains("\"verb\" => Some(Message::Verb)"));
    }

    /// 重复键/嵌套点号冲突/非字符串/空串全拒。
    #[test]
    fn rejects_duplicate_empty_and_nonstring_messages() {
        for source in [
            r#"{"a":"A","a":"B"}"#,
            r#"{"a.b":"A","a":{"b":"B"}}"#,
            r#"{"a":null}"#,
            r#"{"a":""}"#,
        ] {
            assert!(catalog::parse(source).is_err(), "{source}");
        }
    }

    /// 占位符支持 Unicode 文案、重复引用与 `{{`/`}}` 转义。
    #[test]
    fn placeholders_support_unicode_text_repetitions_and_escaped_braces() {
        let names = catalog::placeholders("你好 {name}: {{code}} {name} {count}").unwrap();
        assert_eq!(names.into_iter().collect::<Vec<_>>(), ["count", "name"]);
        assert!(catalog::placeholders("broken {name").is_err());
        assert!(catalog::placeholders("broken }").is_err());
    }
}
