//! AI provider 元数据与凭据引用(凭据纪律归 secrets 域)。
//!
//! 文件边界即凭据边界:provider 元数据独立于设置主文件——后者是用户可编辑
//! 的运行态文件,而凭据需要稳定身份,绝不能与普通设置同文件序列化。
//! `providers.json` 里只有引用字段(`api_key_set`/`api_key_hint`),明文
//! 永不入 store,文件可整份展示/备份/diff 而无泄露。
//!
//! 凭据读写经 [`CredentialStore`] 注入消费(凭据管理器三原语的应用层投影);
//! 真实 Windows 凭据管理器实现归 M10.9,本步用 mock 断言调用契约。

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::secrets::{CredentialStore, SecretBytes};

/// provider 元数据文件名(数据目录内)。
const STORE_FILE: &str = "providers.json";

/// 十三家 provider 种类;不裁剪。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderKind {
    OpenAi,
    Anthropic,
    Google,
    Ollama,
    OpenRouter,
    Qwen,
    DeepSeek,
    Kimi,
    Zhipu,
    Doubao,
    Mimo,
    AzureOpenAi,
    Custom,
}

impl ProviderKind {
    pub(crate) const PRESETS: [Self; 13] = [
        Self::OpenAi,
        Self::Anthropic,
        Self::Google,
        Self::Ollama,
        Self::OpenRouter,
        Self::Qwen,
        Self::DeepSeek,
        Self::Kimi,
        Self::Zhipu,
        Self::Doubao,
        Self::Mimo,
        Self::AzureOpenAi,
        Self::Custom,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Google => "Google",
            Self::Ollama => "Ollama",
            Self::OpenRouter => "OpenRouter",
            Self::Qwen => "Qwen",
            Self::DeepSeek => "DeepSeek",
            Self::Kimi => "Kimi",
            Self::Zhipu => "Zhipu",
            Self::Doubao => "Doubao",
            Self::Mimo => "Xiaomi MiMo",
            Self::AzureOpenAi => "Azure OpenAI",
            Self::Custom => "OpenAI Compatible",
        }
    }

    pub(crate) fn default_base_url(self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Google => "https://generativelanguage.googleapis.com/v1beta",
            Self::Ollama => "http://localhost:11434/v1",
            Self::OpenRouter => "https://openrouter.ai/api/v1",
            Self::Qwen => "https://dashscope.aliyuncs.com/compatible-mode/v1",
            Self::DeepSeek => "https://api.deepseek.com/v1",
            Self::Kimi => "https://api.moonshot.ai/v1",
            Self::Zhipu => "https://open.bigmodel.cn/api/paas/v4",
            Self::Doubao => "https://ark.cn-beijing.volces.com/api/v3",
            Self::Mimo => "https://api.xiaomimimo.com/v1",
            // Azure 走部署路径形态,资源名由用户填写。
            Self::AzureOpenAi => "https://{resource}.openai.azure.com/openai/deployments",
            Self::Custom => "",
        }
    }

    pub(crate) fn default_model(self) -> &'static str {
        match self {
            Self::OpenAi => "gpt-5.4-mini",
            Self::Anthropic => "claude-sonnet-4-5",
            Self::Google => "gemini-2.5-flash",
            Self::Ollama => "qwen3",
            Self::OpenRouter => "openai/gpt-5.4-mini",
            Self::Qwen => "qwen3.7-plus",
            Self::DeepSeek => "deepseek-chat",
            Self::Kimi => "kimi-k2.6",
            Self::Zhipu => "glm-5.1",
            Self::Doubao => "doubao-seed-2-0-pro-260215",
            Self::Mimo => "mimo-v2.5-pro",
            Self::AzureOpenAi => "gpt-4o",
            Self::Custom => "",
        }
    }

    /// Ollama 是本地服务,免 key;其余 provider 都需要凭据。
    pub(crate) fn requires_api_key(self) -> bool {
        !matches!(self, Self::Ollama)
    }

    /// Anthropic/Google 两族协议例外,其余走 OpenAI 兼容形态。
    pub(crate) fn uses_openai_protocol(self) -> bool {
        !matches!(self, Self::Anthropic | Self::Google)
    }
}

/// 一条 provider 元数据。凭据只存引用,明文永不入 store。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AiProvider {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub website_url: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub enabled: bool,
    /// A credential reference only. The actual key is held by the OS store.
    #[serde(default)]
    pub api_key_set: bool,
    /// 尾四掩码(`api_key_hint()` 产出);user-facing fingerprint。
    #[serde(default)]
    pub api_key_hint: String,
    /// `full_url` 直通形态:不再拼接协议路径。
    #[serde(default)]
    pub full_url: bool,
}

impl AiProvider {
    pub(crate) fn preset(kind: ProviderKind, id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: kind.label().to_owned(),
            note: String::new(),
            website_url: String::new(),
            kind,
            base_url: kind.default_base_url().to_owned(),
            model: kind.default_model().to_owned(),
            enabled: true,
            api_key_set: false,
            api_key_hint: String::new(),
            full_url: false,
        }
    }
}

/// provider 集合 + 当前活动项。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProviderStore {
    #[serde(default)]
    pub active_id: String,
    #[serde(default)]
    pub providers: Vec<AiProvider>,
}

/// 设置页可编辑的非敏感字段(两种呈现共用)。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProviderMetadataDraft {
    pub name: String,
    pub note: String,
    pub website_url: String,
    pub base_url: String,
    pub model: String,
}

impl From<&AiProvider> for ProviderMetadataDraft {
    fn from(provider: &AiProvider) -> Self {
        Self {
            name: provider.name.clone(),
            note: provider.note.clone(),
            website_url: provider.website_url.clone(),
            base_url: provider.base_url.clone(),
            model: provider.model.clone(),
        }
    }
}

fn clean_provider_field(value: &str, allow_whitespace: bool) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_control() && (allow_whitespace || !ch.is_whitespace()))
        .take(512)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// 共享清洗:两个呈现入口只经这一份输入策略。
pub(crate) fn apply_metadata_draft(provider: &mut AiProvider, draft: ProviderMetadataDraft) {
    provider.name = clean_provider_field(&draft.name, false);
    provider.note = clean_provider_field(&draft.note, true);
    provider.website_url = clean_provider_field(&draft.website_url, false);
    provider.base_url = clean_provider_field(&draft.base_url, false);
    provider.model = clean_provider_field(&draft.model, false);
}

pub(crate) fn store_path() -> PathBuf {
    crate::platform::dirs::data_dir().join(STORE_FILE)
}

pub(crate) fn load() -> ProviderStore {
    load_at(&store_path())
}

/// 显式路径读入口(测试注入)。缺失/损坏都回默认后再补全预设:
/// 元数据文件不含凭据,损坏没有隔离价值。
pub(crate) fn load_at(path: &Path) -> ProviderStore {
    let mut store = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    normalize(&mut store);
    store
}

/// 补全缺失预设并修复 active_id。
///
/// 旧构建只在点击预设后才落记录;保留既有记录(含多个自定义项)原样可用,
/// 只追加缺失预设,让可见列表是可按索引寻址的完整集合。
fn normalize(store: &mut ProviderStore) {
    let mut existing = std::mem::take(&mut store.providers);
    let mut ordered = Vec::with_capacity(ProviderKind::PRESETS.len().max(existing.len()));
    for kind in ProviderKind::PRESETS {
        if kind == ProviderKind::Custom {
            continue;
        }
        if let Some(index) = existing.iter().position(|provider| provider.kind == kind) {
            ordered.push(existing.remove(index));
        } else {
            ordered.push(AiProvider::preset(kind, preset_id(kind)));
        }
    }
    let had_custom = existing
        .iter()
        .any(|provider| provider.kind == ProviderKind::Custom);
    ordered.extend(
        existing
            .into_iter()
            .filter(|provider| provider.kind == ProviderKind::Custom),
    );
    if !had_custom {
        ordered.push(AiProvider::preset(
            ProviderKind::Custom,
            preset_id(ProviderKind::Custom),
        ));
    }
    store.providers = ordered;
    if !store
        .providers
        .iter()
        .any(|provider| provider.id == store.active_id)
    {
        store.active_id = store
            .providers
            .first()
            .map(|provider| provider.id.clone())
            .unwrap_or_default();
    }
}

pub(crate) fn preset_id(kind: ProviderKind) -> String {
    format!(
        "preset-{}",
        kind.label().to_ascii_lowercase().replace(' ', "-")
    )
}

pub(crate) fn next_custom_id(store: &ProviderStore) -> String {
    let mut sequence = 1usize;
    loop {
        let id = format!("custom-{sequence}");
        if !store.providers.iter().any(|provider| provider.id == id) {
            return id;
        }
        sequence += 1;
    }
}

pub(crate) fn active_enabled(store: &ProviderStore) -> Option<&AiProvider> {
    store
        .providers
        .iter()
        .find(|provider| provider.id == store.active_id && provider.enabled)
}

pub(crate) fn save(store: &ProviderStore) -> io::Result<()> {
    save_at(&store_path(), store)
}

/// 显式路径写入口:临时文件 + 原子替换,读者永远看不到撕裂 JSON。
pub(crate) fn save_at(path: &Path, store: &ProviderStore) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(store)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    crate::atomic_file::write(path, text.as_bytes())
}

/// 凭据 target 命名空间归调用方:`Slterm/AI/<id>`。
pub(crate) fn credential_target(id: &str) -> String {
    format!("Slterm/AI/{id}")
}

/// 尾四掩码:trim 后为空 → 空串;否则 `••••` + 末四位(chars 级)。
pub(crate) fn api_key_hint(key: &str) -> String {
    let key = key.trim();
    if key.is_empty() {
        return String::new();
    }
    let tail: String = key
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("••••{tail}")
}

/// 写入凭据并返回尾四掩码。入 OS 库即返回,不落中间态。
pub(crate) fn save_api_key(
    credentials: &dyn CredentialStore,
    id: &str,
    key: &str,
) -> io::Result<String> {
    let key = key.trim();
    if key.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "API key is empty",
        ));
    }
    credentials.store(&credential_target(id), key.as_bytes())?;
    Ok(api_key_hint(key))
}

/// Store a replacement key and update only its non-secret metadata.
///
/// The plaintext never enters `ProviderStore` and callers must clear their
/// write-only draft immediately after this returns.
pub(crate) fn store_provider_api_key(
    credentials: &dyn CredentialStore,
    provider: &mut AiProvider,
    key: &str,
) -> io::Result<()> {
    let hint = save_api_key(credentials, &provider.id, key)?;
    provider.api_key_set = true;
    provider.api_key_hint = hint;
    Ok(())
}

pub(crate) fn delete_api_key(credentials: &dyn CredentialStore, id: &str) -> io::Result<()> {
    credentials.delete(&credential_target(id))
}

/// Remove a provider and its credential through one presentation-neutral
/// transition. Metadata is mutated only after credential deletion succeeds,
/// avoiding an unreachable orphaned secret.
pub(crate) fn remove_provider(
    credentials: &dyn CredentialStore,
    store: &mut ProviderStore,
    id: &str,
) -> io::Result<()> {
    remove_provider_at(credentials, store, id, &store_path())
}

/// 显式路径版本(测试注入):删除顺序固定为「先定位、再凭据、后元数据、
/// 再落盘」;不存在的 id 零凭据调用。
pub(crate) fn remove_provider_at(
    credentials: &dyn CredentialStore,
    store: &mut ProviderStore,
    id: &str,
    path: &Path,
) -> io::Result<()> {
    let Some(index) = store
        .providers
        .iter()
        .position(|provider| provider.id == id)
    else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "provider not found",
        ));
    };
    delete_api_key(credentials, id)?;
    store.providers.remove(index);
    if store.active_id == id {
        store.active_id = store
            .providers
            .first()
            .map(|provider| provider.id.clone())
            .unwrap_or_default();
    }
    save_at(path, store)
}

/// 读出即得 [`SecretBytes`] 句柄;明文不出凭据域。
pub(crate) fn load_api_key(
    credentials: &dyn CredentialStore,
    id: &str,
) -> io::Result<Option<SecretBytes>> {
    credentials.load(&credential_target(id))
}

#[cfg(test)]
mod ai_providers_tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::{HashMap, HashSet};

    // ---- 十三家 PRESETS 完整性 ----

    #[test]
    fn presets_cover_every_provider_kind_exactly_once() {
        assert_eq!(ProviderKind::PRESETS.len(), 13);
        let unique: HashSet<ProviderKind> = ProviderKind::PRESETS.into_iter().collect();
        assert_eq!(
            unique.len(),
            ProviderKind::PRESETS.len(),
            "PRESETS 不得重复"
        );
    }

    #[test]
    fn every_preset_has_complete_metadata() {
        for kind in ProviderKind::PRESETS {
            let provider = AiProvider::preset(kind, preset_id(kind));
            assert!(!provider.id.is_empty(), "{kind:?}");
            assert!(!provider.name.is_empty(), "{kind:?}");
            assert_eq!(provider.name, kind.label());
            assert_eq!(provider.base_url, kind.default_base_url());
            assert_eq!(provider.model, kind.default_model());
            assert!(!provider.api_key_set, "{kind:?}");
            assert!(provider.api_key_hint.is_empty(), "{kind:?}");
            assert!(!provider.full_url, "{kind:?}");
            assert!(provider.enabled, "{kind:?}");
            if kind == ProviderKind::Custom {
                // 自定义项由用户填写;预置留空而不是塞一个假端点。
                assert!(provider.base_url.is_empty());
                assert!(provider.model.is_empty());
            } else {
                assert!(!provider.base_url.is_empty(), "{kind:?}");
                assert!(
                    provider.base_url.starts_with("https://")
                        || provider.base_url.starts_with("http://"),
                    "{kind:?}: {}",
                    provider.base_url
                );
                assert!(!provider.model.is_empty(), "{kind:?}");
            }
        }
    }

    #[test]
    fn api_key_requirements_follow_provider_kind() {
        for kind in ProviderKind::PRESETS {
            assert_eq!(
                kind.requires_api_key(),
                kind != ProviderKind::Ollama,
                "{kind:?}"
            );
        }
    }

    #[test]
    fn protocol_families_are_anthropic_google_and_openai_compatible() {
        for kind in ProviderKind::PRESETS {
            let expected = !matches!(kind, ProviderKind::Anthropic | ProviderKind::Google);
            assert_eq!(kind.uses_openai_protocol(), expected, "{kind:?}");
        }
    }

    #[test]
    fn preset_ids_are_stable_and_lowercase() {
        assert_eq!(preset_id(ProviderKind::OpenAi), "preset-openai");
        assert_eq!(preset_id(ProviderKind::AzureOpenAi), "preset-azure-openai");
        assert_eq!(preset_id(ProviderKind::Mimo), "preset-xiaomi-mimo");
        let ids: HashSet<String> = ProviderKind::PRESETS.into_iter().map(preset_id).collect();
        assert_eq!(ids.len(), ProviderKind::PRESETS.len());
    }

    // ---- 尾四掩码 ----

    #[test]
    fn key_hint_never_contains_the_full_secret() {
        let hint = api_key_hint("sk-this-is-a-long-secret");
        assert_eq!(hint, "••••cret");
        assert!(!hint.contains("long-secret"));
    }

    #[test]
    fn key_hint_trims_and_keeps_only_four_chars() {
        assert_eq!(api_key_hint("  sk-test  "), "••••test");
        assert_eq!(api_key_hint("abc"), "••••abc");
        assert_eq!(api_key_hint("abcd"), "••••abcd");
        assert_eq!(api_key_hint(""), "");
        assert_eq!(api_key_hint("   "), "");
    }

    #[test]
    fn credential_target_is_namespaced_per_provider() {
        assert_eq!(
            credential_target("preset-openai"),
            "Slterm/AI/preset-openai"
        );
        assert_eq!(credential_target("custom-2"), "Slterm/AI/custom-2");
    }

    /// 类型层凭据边界:provider 元数据序列化后只有引用字段,绝无明文 token。
    #[test]
    fn provider_store_serializes_only_credential_references() {
        let fake = "sk-test-never-serialized-plaintext";
        let mut provider =
            AiProvider::preset(ProviderKind::OpenAi, preset_id(ProviderKind::OpenAi));
        provider.api_key_set = true;
        provider.api_key_hint = api_key_hint(fake);
        let store = ProviderStore {
            active_id: provider.id.clone(),
            providers: vec![provider],
        };
        let json = serde_json::to_string(&store).expect("provider store 可序列化");
        assert!(!json.contains(fake), "明文 token 不得进入序列化结果");
        let value: serde_json::Value = serde_json::from_str(&json).expect("JSON 可解析");
        let keys: HashSet<&str> = value["providers"][0]
            .as_object()
            .expect("provider 是 JSON 对象")
            .keys()
            .map(String::as_str)
            .collect();
        for forbidden in ["token", "secret", "api_key", "plaintext"] {
            assert!(!keys.contains(forbidden), "凭据明文键 {forbidden} 不得出域");
        }
        assert!(keys.contains("api_key_set"));
        assert!(keys.contains("api_key_hint"));
    }

    // ---- 元数据清洗 ----

    #[test]
    fn metadata_draft_uses_one_shared_sanitizer() {
        let mut provider = AiProvider::preset(ProviderKind::Custom, "custom-test");
        apply_metadata_draft(
            &mut provider,
            ProviderMetadataDraft {
                name: " My Provider \n".into(),
                note: " keep spaces \n but no controls ".into(),
                website_url: " https://example.com/a b ".into(),
                base_url: " https://api.example.com /v1 ".into(),
                model: " model name ".into(),
            },
        );
        assert_eq!(provider.name, "MyProvider");
        assert_eq!(provider.note, "keep spaces  but no controls");
        assert_eq!(provider.website_url, "https://example.com/ab");
        assert_eq!(provider.base_url, "https://api.example.com/v1");
        assert_eq!(provider.model, "modelname");
    }

    // ---- store 持久化与补全 ----

    #[test]
    fn store_round_trips_atomically_with_all_presets() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        let store = load_at(&path);
        assert_eq!(store.providers.len(), ProviderKind::PRESETS.len());
        assert_eq!(store.active_id, preset_id(ProviderKind::OpenAi));

        save_at(&path, &store).unwrap();
        assert_eq!(load_at(&path), store);
        let names: Vec<String> = std::fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, [STORE_FILE.to_owned()], "原子替换不得残留临时文件");
    }

    #[test]
    fn corrupt_store_falls_back_to_full_presets() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        std::fs::write(&path, "{ not json").unwrap();
        let store = load_at(&path);
        assert_eq!(store.providers.len(), ProviderKind::PRESETS.len());
    }

    #[test]
    fn normalization_preserves_multiple_custom_entries() {
        let mut store = ProviderStore {
            active_id: "custom-2".to_owned(),
            providers: vec![
                AiProvider::preset(ProviderKind::Custom, "custom-1"),
                AiProvider::preset(ProviderKind::Custom, "custom-2"),
            ],
        };
        normalize(&mut store);
        assert_eq!(
            store
                .providers
                .iter()
                .filter(|provider| provider.kind == ProviderKind::Custom)
                .count(),
            2
        );
        assert_eq!(store.providers.len(), ProviderKind::PRESETS.len() + 1);
        assert_eq!(store.active_id, "custom-2");
        assert_eq!(next_custom_id(&store), "custom-3");
    }

    #[test]
    fn normalization_repairs_missing_active_id() {
        let mut store = ProviderStore::default();
        normalize(&mut store);
        assert_eq!(store.active_id, preset_id(ProviderKind::OpenAi));
        assert_eq!(
            active_enabled(&store).map(|provider| provider.kind),
            Some(ProviderKind::OpenAi)
        );
    }

    // ---- 凭据读写:注入 mock 断言调用契约 ----

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum CredentialCall {
        Store(String, Vec<u8>),
        Load(String),
        Delete(String),
        Exists(String),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum CredentialOp {
        Store,
        Load,
        Delete,
    }

    #[derive(Default)]
    struct MockCredentials {
        calls: RefCell<Vec<CredentialCall>>,
        secrets: RefCell<HashMap<String, Vec<u8>>>,
        fail_on: RefCell<Option<CredentialOp>>,
    }

    impl MockCredentials {
        fn failing(op: CredentialOp) -> Self {
            Self {
                fail_on: RefCell::new(Some(op)),
                ..Self::default()
            }
        }

        fn calls(&self) -> Vec<CredentialCall> {
            self.calls.borrow().clone()
        }

        fn should_fail(&self, op: CredentialOp) -> bool {
            *self.fail_on.borrow() == Some(op)
        }
    }

    impl CredentialStore for MockCredentials {
        fn load(&self, target: &str) -> io::Result<Option<SecretBytes>> {
            self.calls
                .borrow_mut()
                .push(CredentialCall::Load(target.to_owned()));
            if self.should_fail(CredentialOp::Load) {
                return Err(io::Error::other("mock load failure"));
            }
            Ok(self
                .secrets
                .borrow()
                .get(target)
                .cloned()
                .map(SecretBytes::new))
        }

        fn store(&self, target: &str, secret: &[u8]) -> io::Result<()> {
            self.calls
                .borrow_mut()
                .push(CredentialCall::Store(target.to_owned(), secret.to_vec()));
            if self.should_fail(CredentialOp::Store) {
                return Err(io::Error::other("mock store failure"));
            }
            self.secrets
                .borrow_mut()
                .insert(target.to_owned(), secret.to_vec());
            Ok(())
        }

        fn delete(&self, target: &str) -> io::Result<()> {
            self.calls
                .borrow_mut()
                .push(CredentialCall::Delete(target.to_owned()));
            if self.should_fail(CredentialOp::Delete) {
                return Err(io::Error::other("mock delete failure"));
            }
            self.secrets.borrow_mut().remove(target);
            Ok(())
        }

        fn exists(&self, target: &str) -> io::Result<bool> {
            self.calls
                .borrow_mut()
                .push(CredentialCall::Exists(target.to_owned()));
            Ok(self.secrets.borrow().contains_key(target))
        }
    }

    #[test]
    fn save_api_key_writes_target_then_returns_tail_hint() {
        let credentials = MockCredentials::default();
        let hint = save_api_key(&credentials, "preset-openai", " sk-test-secret ").unwrap();
        assert_eq!(hint, "••••cret");
        assert_eq!(
            credentials.calls(),
            [CredentialCall::Store(
                "Slterm/AI/preset-openai".to_owned(),
                b"sk-test-secret".to_vec()
            )]
        );
        // 明文只存在于凭据后端,未进入任何元数据字段。
        assert!(!hint.contains("sk-test-secret"));
    }

    #[test]
    fn save_api_key_rejects_empty_without_touching_the_backend() {
        let credentials = MockCredentials::default();
        let error = save_api_key(&credentials, "preset-openai", "  ").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(credentials.calls().is_empty());
    }

    #[test]
    fn store_provider_api_key_updates_reference_after_backend_succeeds() {
        let credentials = MockCredentials::default();
        let mut provider = AiProvider::preset(ProviderKind::OpenAi, "preset-openai");
        store_provider_api_key(&credentials, &mut provider, "sk-test-value").unwrap();
        assert!(provider.api_key_set);
        assert_eq!(provider.api_key_hint, "••••alue");
        assert!(!provider.api_key_hint.contains("sk-test-value"));
    }

    #[test]
    fn backend_failure_leaves_provider_metadata_untouched() {
        let credentials = MockCredentials::failing(CredentialOp::Store);
        let mut provider = AiProvider::preset(ProviderKind::OpenAi, "preset-openai");
        let before = provider.clone();
        assert!(store_provider_api_key(&credentials, &mut provider, "sk-test-value").is_err());
        assert_eq!(provider, before, "写凭据失败不得留下中间态引用");
    }

    #[test]
    fn load_api_key_returns_a_zeroizing_handle() {
        let credentials = MockCredentials::default();
        save_api_key(&credentials, "preset-openai", "sk-test-value").unwrap();
        let secret = load_api_key(&credentials, "preset-openai")
            .unwrap()
            .unwrap();
        assert_eq!(secret.as_bytes(), b"sk-test-value");
        assert_eq!(secret.into_string().unwrap().as_str(), "sk-test-value");
        assert!(load_api_key(&credentials, "missing").unwrap().is_none());
        assert_eq!(
            credentials.calls().last(),
            Some(&CredentialCall::Load("Slterm/AI/missing".to_owned()))
        );
    }

    #[test]
    fn load_failure_is_surfaced_instead_of_swallowed() {
        let credentials = MockCredentials::failing(CredentialOp::Load);
        assert!(load_api_key(&credentials, "preset-openai").is_err());
    }

    #[test]
    fn delete_api_key_targets_the_namespaced_credential() {
        let credentials = MockCredentials::default();
        save_api_key(&credentials, "preset-openai", "sk-test-value").unwrap();
        delete_api_key(&credentials, "preset-openai").unwrap();
        assert_eq!(
            credentials.calls().last(),
            Some(&CredentialCall::Delete(
                "Slterm/AI/preset-openai".to_owned()
            ))
        );
        assert!(
            load_api_key(&credentials, "preset-openai")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn remove_provider_deletes_credential_before_metadata_and_disk() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        let credentials = MockCredentials::default();
        let mut store = ProviderStore {
            active_id: "custom-1".to_owned(),
            providers: vec![
                AiProvider::preset(ProviderKind::Custom, "custom-1"),
                AiProvider::preset(ProviderKind::Custom, "custom-2"),
            ],
        };
        normalize(&mut store);
        save_api_key(&credentials, "custom-1", "sk-test-value").unwrap();
        remove_provider_at(&credentials, &mut store, "custom-1", &path).unwrap();

        assert_eq!(
            credentials.calls().last(),
            Some(&CredentialCall::Delete("Slterm/AI/custom-1".to_owned()))
        );
        assert!(!store.providers.iter().any(|p| p.id == "custom-1"));
        assert_eq!(store.active_id, "preset-openai");
        assert_eq!(load_at(&path), store, "删除后元数据应已原子落盘");
        assert!(load_api_key(&credentials, "custom-1").unwrap().is_none());
    }

    #[test]
    fn remove_provider_missing_id_never_calls_credentials_or_touches_disk() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        let credentials = MockCredentials::default();
        let mut store = load_at(&path);
        let before = store.clone();
        let error = remove_provider_at(&credentials, &mut store, "missing", &path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(
            credentials.calls().is_empty(),
            "不存在的 id 不得触发凭据删除"
        );
        assert_eq!(store, before, "不存在的 id 不得改内存");
        assert!(!path.exists(), "不存在的 id 不得落盘");
    }

    #[test]
    fn remove_provider_stops_before_metadata_when_credential_delete_fails() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        let credentials = MockCredentials::failing(CredentialOp::Delete);
        let mut store = load_at(&path);
        let before = store.clone();
        assert!(remove_provider_at(&credentials, &mut store, "preset-openai", &path).is_err());
        assert_eq!(store, before, "删凭据失败不得动元数据");
        assert!(!path.exists(), "失败路径不得落盘");
        assert_eq!(
            credentials.calls(),
            [CredentialCall::Delete("Slterm/AI/preset-openai".to_owned())]
        );
    }

    #[test]
    fn exists_uses_the_backend_probe() {
        let credentials = MockCredentials::default();
        assert!(!credentials.exists("Slterm/AI/preset-openai").unwrap());
        save_api_key(&credentials, "preset-openai", "sk-test-value").unwrap();
        assert!(credentials.exists("Slterm/AI/preset-openai").unwrap());
        assert!(matches!(
            credentials.calls().last(),
            Some(CredentialCall::Exists(target)) if target == "Slterm/AI/preset-openai"
        ));
    }
}
