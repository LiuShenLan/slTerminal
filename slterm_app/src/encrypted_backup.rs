//! 口令保护的本地加密备份。
//!
//! 本模块刻意不遍历用户 home 目录:下方白名单即导出与导入的安全边界
//! (白名单即安全边界,枚举即授权)。AI 会话库、环境变量、凭据库、token
//! 来源文件(`~/.claude/settings.json`)永不读写进出备份;清单外文件不存在
//! 「顺手带上」的通道,导入导出同一道边界。
//!
//! 格式:`SLTMBK01 | salt(16) | nonce(12) | ciphertext`。完整头部
//! (魔数｜盐｜nonce)同时作 AES-256-GCM 的 AAD,格式身份、KDF 参数与
//! nonce 都由认证标签显式绑定——魔数不符即「非本应用备份」一票拒解,
//! 不接受旧版仅魔数 AAD 的包。密钥派生 = Argon2id v0x13,m=19456 KiB、
//! t=2、p=1(契约数值随上游基线)。派生密钥、解密明文与恢复点明文全程
//! `Zeroizing`;解密失败消息不回显口令或任何派生材料。
//!
//! 恢复序:先完整鉴权(`open`)→ `validate_archive` 全量校验 → 才允许
//! 任何落盘。校验任何一条不过整单拒写,半份不可信报文零副作用。
//!
//! 类别与条目只来自登记白名单：设置类别只投影 `SettingsRegistry` 中
//! 「可导出且非敏感」的字段，并包含合法自定义主题文档。未知命名空间、
//! 未登记字段、会话原文、学习历史与工作区物理文件都不存在入包通道；
//! 凭据值永不入包。
//!
//! 消费点:收集/密封/维护期恢复的生产入口尚未接入设置页；运行期也不提供
//! 整包或选择性恢复的直接写入口。恢复只经 [`maintenance::stage_restore`]
//! 暂存并登记维护请求，由启动维护期应用与回滚；撤回实现必须复用同一维护
//! 请求时序,不得新增运行期写盘通道。

use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use serde::{Deserialize, Serialize};
use slterm_path::AppPaths;
use zeroize::Zeroizing;

pub(crate) mod maintenance;
pub(crate) mod recovery;

/// 备份格式魔数(已裁:去品牌,8 字节,同时作 AAD)。
const MAGIC: &[u8; 8] = b"SLTMBK01";
/// 归档格式版本;不符即「版本不支持」一票拒。
const ARCHIVE_VERSION: u32 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
/// 口令最小长度(chars 计数,契约数值随上游基线);UI 前置校验同源引用。
pub(crate) const MIN_PASSPHRASE_CHARS: usize = 8;
/// 备份归档明文硬上限(契约数值):超出即拒绝密封与解封。
const MAX_ARCHIVE_PLAINTEXT_BYTES: usize = 64 * 1024 * 1024;

/// 自定义主题文件名后缀(与主题库持久化规则同源)。
const THEME_FILE_SUFFIX: &str = ".slterm-theme.json";

/// 备份类别(与设计类别集合同源;凭据域值永不入包)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackupCategory {
    /// 应用设置与自定义主题；条目只含登记导出字段与合法主题文档。
    Settings,
}

/// 类别根归属是备份收集、恢复与撤回共用的唯一寻址规则。
fn category_root(paths: &AppPaths, category: BackupCategory) -> &Path {
    match category {
        BackupCategory::Settings => paths.config_root(),
    }
}

/// 类别选择集;设置类别包含应用设置与自定义主题。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BackupSelection {
    pub settings: bool,
}

impl Default for BackupSelection {
    fn default() -> Self {
        Self { settings: true }
    }
}

impl BackupSelection {
    pub(crate) fn is_empty(self) -> bool {
        self.categories().next().is_none()
    }

    pub(crate) fn set(&mut self, category: BackupCategory, enabled: bool) {
        match category {
            BackupCategory::Settings => self.settings = enabled,
        }
    }

    /// 读取单类别选中态(UI 开关渲染与 `set` 共用同一字段映射)。
    pub(crate) fn get(self, category: BackupCategory) -> bool {
        match category {
            BackupCategory::Settings => self.settings,
        }
    }

    pub(crate) fn from_categories(categories: impl IntoIterator<Item = BackupCategory>) -> Self {
        let mut selection = Self { settings: false };
        for category in categories {
            selection.set(category, true);
        }
        selection
    }

    pub(crate) fn categories(self) -> impl Iterator<Item = BackupCategory> {
        [(self.settings, BackupCategory::Settings)]
            .into_iter()
            .filter_map(|(selected, category)| selected.then_some(category))
    }
}

/// 归档中的单个文件条目;`name` 是相对类别所属数据根的归档名(正斜杠)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackupEntry {
    pub category: BackupCategory,
    pub name: String,
    pub bytes: Vec<u8>,
}

/// 归档清单。设备名字段不迁(无本地消费方)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackupManifest {
    pub version: u32,
    pub categories: Vec<BackupCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackupArchive {
    pub manifest: BackupManifest,
    pub entries: Vec<BackupEntry>,
}

/// 类别汇总(UI 预览消费)。
#[derive(Debug, Clone)]
pub(crate) struct CategorySummary {
    pub category: BackupCategory,
    pub files: usize,
    pub bytes: u64,
}

impl BackupArchive {
    pub(crate) fn summary(&self) -> Vec<CategorySummary> {
        self.manifest
            .categories
            .iter()
            .map(|category| {
                let entries = self
                    .entries
                    .iter()
                    .filter(|entry| entry.category == *category);
                let (files, bytes) = entries.fold((0, 0), |(files, bytes), entry| {
                    (files + 1, bytes + entry.bytes.len() as u64)
                });
                CategorySummary {
                    category: *category,
                    files,
                    bytes,
                }
            })
            .collect()
    }
}

fn validate_passphrase(passphrase: &str) -> Result<(), String> {
    (passphrase.chars().count() >= MIN_PASSPHRASE_CHARS)
        .then_some(())
        .ok_or_else(|| "备份口令至少需要 8 个字符".to_owned())
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], String> {
    let params = argon2::Params::new(19_456, 2, 1, Some(KEY_LEN))
        .map_err(|error| format!("Argon2id 参数无效: {error}"))?;
    let argon = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut key = [0; KEY_LEN];
    argon
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|error| format!("密钥派生失败: {error}"))?;
    Ok(key)
}

/// 序列化并密封归档 → `SLTMBK01 | salt | nonce | ciphertext`。
pub(crate) fn seal(archive: &BackupArchive, passphrase: &str) -> Result<Vec<u8>, String> {
    validate_passphrase(passphrase)?;
    if archive.manifest.version != ARCHIVE_VERSION {
        return Err("备份归档版本不受支持".to_owned());
    }
    validate_archive(archive)?;
    let plaintext = Zeroizing::new(
        serde_json::to_vec(archive).map_err(|error| format!("序列化备份归档失败: {error}"))?,
    );
    encrypt_bytes(&plaintext, passphrase)
}

fn encrypt_bytes(plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    validate_passphrase(passphrase)?;
    if plaintext.len() > MAX_ARCHIVE_PLAINTEXT_BYTES {
        return Err("备份明文超过 64 MiB 上限".to_owned());
    }
    let mut salt = [0; SALT_LEN];
    let mut nonce = [0; NONCE_LEN];
    getrandom::fill(&mut salt).map_err(|error| format!("系统随机源失败: {error}"))?;
    getrandom::fill(&mut nonce).map_err(|error| format!("系统随机源失败: {error}"))?;
    let key = Zeroizing::new(derive_key(passphrase, &salt)?);
    // 完整头部入 AAD：格式身份、盐与 nonce 都由认证标签显式绑定，
    // 不依赖 KDF／GCM 的隐式性质；未来追加头部字段时无需改认证口径。
    let mut header = Vec::with_capacity(MAGIC.len() + SALT_LEN + NONCE_LEN);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&salt);
    header.extend_from_slice(&nonce);
    let ciphertext = Aes256Gcm::new((&*key).into())
        .encrypt(
            &Nonce::try_from(&nonce[..]).map_err(|_| "备份 nonce 无效".to_owned())?,
            Payload {
                msg: plaintext,
                aad: &header,
            },
        )
        .map_err(|_| "备份加密失败".to_owned())?;
    let mut output = header;
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// 完整鉴权 → 解密 → 反序列化 → 全量校验。
pub(crate) fn open(packet: &[u8], passphrase: &str) -> Result<BackupArchive, String> {
    let plaintext = decrypt_bytes(packet, passphrase)?;
    let archive: BackupArchive =
        serde_json::from_slice(&plaintext).map_err(|error| format!("备份归档内容无效: {error}"))?;
    validate_archive(&archive)?;
    Ok(archive)
}

fn decrypt_bytes(packet: &[u8], passphrase: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    validate_passphrase(passphrase)?;
    let header_len = MAGIC.len() + SALT_LEN + NONCE_LEN;
    // 密文 = 明文 + 16 字节 GCM tag；超限包在解密前拒绝，避免无界分配。
    if packet.len().saturating_sub(header_len) > MAX_ARCHIVE_PLAINTEXT_BYTES + 16 {
        return Err("备份明文超过 64 MiB 上限".to_owned());
    }
    // 先验魔数再派生密钥:魔数不符即一票拒解,不浪费派生成本。
    if packet.len() <= header_len || packet.get(..MAGIC.len()) != Some(MAGIC) {
        return Err("不是本应用的加密备份或版本不受支持".to_owned());
    }
    let salt = &packet[MAGIC.len()..MAGIC.len() + SALT_LEN];
    let nonce = &packet[MAGIC.len() + SALT_LEN..header_len];
    let key = Zeroizing::new(derive_key(passphrase, salt)?);
    let plaintext = Aes256Gcm::new((&*key).into())
        .decrypt(
            &Nonce::try_from(nonce).map_err(|_| "备份 nonce 无效".to_owned())?,
            Payload {
                msg: &packet[header_len..],
                aad: &packet[..header_len],
            },
        )
        .map_err(|_| "备份认证失败: 口令错误或文件已被篡改".to_owned())?;
    Ok(Zeroizing::new(plaintext))
}

/// 按选择集收集双数据根的白名单文件。
pub(crate) fn collect(selection: BackupSelection) -> Result<BackupArchive, String> {
    collect_from(crate::platform::dirs::app_paths(), selection)
}

fn collect_from(paths: &AppPaths, selection: BackupSelection) -> Result<BackupArchive, String> {
    if selection.is_empty() {
        return Err("至少选择一个备份类别".to_owned());
    }
    let mut entries = Vec::new();
    for category in selection.categories() {
        let root = category_root(paths, category);
        match category {
            BackupCategory::Settings => {
                add_file(
                    root,
                    category,
                    "settings.json",
                    &mut entries,
                    filter_settings,
                )?;
                add_themes(root, category, &mut entries)?;
            }
        }
    }
    Ok(BackupArchive {
        manifest: BackupManifest {
            version: ARCHIVE_VERSION,
            categories: selection.categories().collect(),
        },
        entries,
    })
}

/// 导出侧设置投影：只保留注册表登记为可导出且非敏感的字段，
/// 未知命名空间、未登记字段与敏感键一律不入包；结构按段重建。
fn filter_settings(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    use slterm_settings::json::Value as SettingsValue;
    let tree = slterm_settings::json::parse(&bytes)
        .map_err(|error| format!("备份设置文档不是合法 JSON: {error}"))?;
    if !matches!(tree, SettingsValue::Object(_)) {
        return Err("备份设置文档根必须是 JSON 对象".to_owned());
    }
    let raw = slterm_settings::RawSettings::from_json_bytes(&bytes);
    // 段 → 登记导出字段；未知段与未登记键没有入口。
    let mut segments: std::collections::BTreeMap<&'static str, Vec<(String, SettingsValue)>> =
        std::collections::BTreeMap::new();
    for def in slterm_settings::keys::KEYS
        .iter()
        .filter(|def| def.exportable && !def.sensitive)
    {
        let Some(value) = raw.persisted_value(def.key) else {
            continue;
        };
        if matches!(value, SettingsValue::Null) || !(def.validator)(value) {
            return Err(format!("备份设置字段未通过校验: {}", def.key));
        }
        segments
            .entry(def.domain.segment())
            .or_default()
            .push((def.key.to_owned(), value.clone()));
    }
    let mut root = SettingsValue::empty_object();
    for (segment, members) in segments {
        root.set(segment, SettingsValue::Object(members));
    }
    Ok(slterm_settings::json::to_string_pretty(&root).into_bytes())
}

/// 导入侧复验：设置文档只能包含登记导出字段，未知段/键与敏感字段整单拒绝。
fn settings_document_is_exportable(bytes: &[u8]) -> bool {
    use slterm_settings::json::Value as SettingsValue;
    let Ok(SettingsValue::Object(root)) = slterm_settings::json::parse(bytes) else {
        return false;
    };
    root.into_iter().all(|(segment, values)| {
        let Some(domain) = slterm_settings::keys::SettingsDomain::ALL
            .into_iter()
            .find(|domain| domain.segment() == segment)
        else {
            return false;
        };
        let SettingsValue::Object(values) = values else {
            return false;
        };
        values.into_iter().all(|(key, value)| {
            let Some(def) = slterm_settings::keys::lookup(&key) else {
                return false;
            };
            def.domain == domain
                && def.exportable
                && !def.sensitive
                && !matches!(value, SettingsValue::Null)
                && (def.validator)(&value)
        })
    })
}

/// 读取白名单文件并施加变换;缺失静默跳过(该类别本机尚无文件)。
fn add_file(
    root: &Path,
    category: BackupCategory,
    name: &str,
    entries: &mut Vec<BackupEntry>,
    transform: fn(Vec<u8>) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    let path = root.join(name);
    // 白名单目标必须是普通文件；符号链接/junction 不得把根外内容带入包内。
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(format!("备份文件不是普通文件: {name}"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("读取备份文件元数据 {name} 失败: {error}")),
    }
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("读取备份文件 {name} 失败: {error}")),
    };
    entries.push(BackupEntry {
        category,
        name: name.to_owned(),
        bytes: transform(bytes)?,
    });
    Ok(())
}

/// 收集自定义主题文档:只认 `themes/` 下合法 id + 固定后缀的普通文件;
/// 符号链接跳过(安全边界)。坏文档整单失败:静默跳过会让备份缺件
/// 而用户误以为完整。
fn add_themes(
    root: &Path,
    category: BackupCategory,
    entries: &mut Vec<BackupEntry>,
) -> Result<(), String> {
    let directory = root.join("themes");
    let read_dir = match fs::read_dir(&directory) {
        Ok(read_dir) => read_dir,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("读取主题目录失败: {error}")),
    };
    let mut names = Vec::new();
    for item in read_dir {
        let path = item
            .map_err(|error| format!("读取主题目录项失败: {error}"))?
            .path();
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(id) = file_name.strip_suffix(THEME_FILE_SUFFIX) else {
            continue;
        };
        if !is_valid_theme_id(id) {
            continue;
        }
        names.push(file_name.to_owned());
    }
    // 归档条目顺序稳定:目录枚举序不可依赖。
    names.sort();
    for file_name in names {
        let path = directory.join(&file_name);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("读取主题文件元数据失败: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| format!("读取主题文件失败: {error}"))?;
        // 导入侧复验同一判据。坏文档整单失败:备份必须要么完整、
        // 要么明确报错,不能静默缺件。
        if crate::theme_library::ThemeDocument::from_json_bytes(&bytes).is_err() {
            crate::audit::record(
                crate::audit::AuditEvent::new(
                    "backup_invalid_theme",
                    "encrypted_backup::add_themes",
                    crate::audit::AuditResult::Error,
                )
                .with_path(&path),
            );
            return Err(format!("主题文件不是有效文档,备份中止: {}", path.display()));
        }
        entries.push(BackupEntry {
            category,
            name: format!("themes/{file_name}"),
            bytes,
        });
    }
    Ok(())
}

fn is_valid_theme_id(id: &str) -> bool {
    // 身份规则单点 = ThemeId 解析（UUIDv7），不复制第二份规则。
    id.parse::<slterm_settings::ThemeId>().is_ok()
}

/// 归档名 → 目标路径:空名/绝对路径/非 Normal 分量(含 `..`)一律拒。
fn safe_path(root: &Path, name: &str) -> Result<PathBuf, String> {
    let path = Path::new(name);
    if name.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("不安全的备份路径: {name:?}"));
    }
    Ok(root.join(path))
}

/// 目标路径 + 父链核查:父链命中符号链接或非目录即拒(防越界写)。
fn restore_path(root: &Path, name: &str) -> Result<PathBuf, String> {
    let path = safe_path(root, name)?;
    let mut current = root.to_owned();
    for component in Path::new(name).components() {
        let Component::Normal(component) = component else {
            unreachable!("safe_path 已拒绝非 Normal 分量");
        };
        current.push(component);
        if current == path {
            break;
        }
        if let Ok(metadata) = fs::symlink_metadata(&current)
            && (metadata.file_type().is_symlink() || !metadata.is_dir())
        {
            return Err(format!("备份目标父链不安全: {}", current.display()));
        }
    }
    Ok(path)
}

/// 恢复前全量校验:版本/类别无重复/条目类别受清单授权/重名/路径/按类别
/// 文件名白名单/JSON 可解析项。任何一条不过整单拒写。
fn validate_archive(archive: &BackupArchive) -> Result<(), String> {
    if archive.manifest.version != ARCHIVE_VERSION {
        return Err("备份归档版本不受支持".to_owned());
    }
    let categories: HashSet<_> = archive.manifest.categories.iter().copied().collect();
    if categories.len() != archive.manifest.categories.len() {
        return Err("备份类别清单存在重复项".to_owned());
    }
    if categories.is_empty() || archive.entries.is_empty() {
        return Err("备份归档不含任何条目".to_owned());
    }
    let mut names = HashSet::new();
    for entry in &archive.entries {
        if !categories.contains(&entry.category) || !names.insert(entry.name.clone()) {
            return Err("备份条目类别未授权或文件名重复".to_owned());
        }
        safe_path(Path::new("."), &entry.name)?;
        let allowed = match entry.category {
            BackupCategory::Settings => {
                (entry.name == "settings.json"
                    && entry.bytes.len() <= slterm_settings::MAX_PERSIST_BYTES
                    && settings_document_is_exportable(&entry.bytes))
                    || theme_entry_allowed(&entry.name, &entry.bytes)
            }
        };
        if !allowed {
            return Err(format!(
                "类别 {:?} 不允许条目 {:?}",
                entry.category, entry.name
            ));
        }
    }
    Ok(())
}

fn theme_entry_allowed(name: &str, bytes: &[u8]) -> bool {
    let Some(file_name) = name.strip_prefix("themes/") else {
        return false;
    };
    if file_name.contains('/') {
        return false;
    }
    let Some(id) = file_name.strip_suffix(THEME_FILE_SUFFIX) else {
        return false;
    };
    is_valid_theme_id(id) && crate::theme_library::ThemeDocument::from_json_bytes(bytes).is_ok()
}

#[cfg(test)]
mod encrypted_backup_tests {
    use super::*;
    use std::collections::HashSet;

    const PASSPHRASE: &str = "correct horse";
    const SHORT_PASSPHRASE: &str = "short";
    const THEME_NAME: &str = "themes/018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b.slterm-theme.json";

    fn single_root_paths(root: &Path) -> AppPaths {
        AppPaths::new(root, root).unwrap()
    }

    fn split_paths(root: &Path) -> AppPaths {
        AppPaths::new(root.join("config"), root.join("local")).unwrap()
    }

    fn theme_bytes() -> Vec<u8> {
        crate::theme_library::builtin_documents()
            .first()
            .expect("内置主题文档非空")
            .to_json_bytes()
            .expect("内置主题文档可序列化")
    }

    fn archive() -> BackupArchive {
        BackupArchive {
            manifest: BackupManifest {
                version: ARCHIVE_VERSION,
                categories: vec![BackupCategory::Settings],
            },
            entries: vec![BackupEntry {
                category: BackupCategory::Settings,
                name: "settings.json".into(),
                bytes: br#"{"appearance":{"theme":"Nord"}}"#.to_vec(),
            }],
        }
    }

    fn full_archive() -> BackupArchive {
        BackupArchive {
            manifest: BackupManifest {
                version: ARCHIVE_VERSION,
                categories: vec![BackupCategory::Settings],
            },
            entries: vec![
                BackupEntry {
                    category: BackupCategory::Settings,
                    name: "settings.json".into(),
                    bytes: br#"{"appearance":{"theme":"Nord"}}"#.to_vec(),
                },
                BackupEntry {
                    category: BackupCategory::Settings,
                    name: THEME_NAME.into(),
                    bytes: theme_bytes(),
                },
            ],
        }
    }

    #[test]
    fn defaults_only_select_settings() {
        let selection = BackupSelection::default();
        assert_eq!(
            selection.categories().collect::<Vec<_>>(),
            vec![BackupCategory::Settings]
        );
        assert!(!selection.is_empty());

        let empty = BackupSelection { settings: false };
        assert!(empty.is_empty());
    }

    #[test]
    fn selection_set_and_from_categories_agree() {
        let mut selection = BackupSelection { settings: false };
        assert!(!selection.get(BackupCategory::Settings));
        selection.set(BackupCategory::Settings, true);
        assert!(selection.get(BackupCategory::Settings));
        assert_eq!(
            selection.categories().collect::<Vec<_>>(),
            vec![BackupCategory::Settings]
        );
        assert_eq!(
            BackupSelection::from_categories([BackupCategory::Settings]),
            selection
        );
    }

    /// 篡改包在鉴权阶段失败，目标目录零写入。
    #[test]
    fn backup_tamper_rejected_zero_write() {
        let directory = tempfile::tempdir().unwrap();
        let paths = split_paths(directory.path());
        fs::create_dir_all(paths.config_root()).unwrap();
        fs::create_dir_all(paths.local_root()).unwrap();
        let settings = br#"{"appearance":{"theme":"Nord"}}"#.to_vec();
        fs::write(paths.config_root().join("settings.json"), &settings).unwrap();
        fs::create_dir_all(paths.config_root().join("themes")).unwrap();
        let theme_path = paths.config_root().join(THEME_NAME);
        fs::write(&theme_path, theme_bytes()).unwrap();

        let mut packet = seal(&full_archive(), PASSPHRASE).unwrap();
        let last = packet.len() - 1;
        packet[last] ^= 0xff;
        assert!(
            open(&packet, PASSPHRASE).is_err(),
            "篡改包必须在鉴权阶段拒绝"
        );
        assert_eq!(
            fs::read(paths.config_root().join("settings.json")).unwrap(),
            settings,
            "鉴权失败必须零写入"
        );
        assert_eq!(fs::read(&theme_path).unwrap(), theme_bytes());
    }

    /// 凭据值永不入包：未登记段与敏感字段在导出投影中直接不存在。
    #[test]
    fn backup_export_never_carries_credentials() {
        let directory = tempfile::tempdir().unwrap();
        let paths = split_paths(directory.path());
        fs::create_dir_all(paths.config_root()).unwrap();
        fs::create_dir_all(paths.local_root()).unwrap();
        fs::write(
            paths.config_root().join("settings.json"),
            br#"{"appearance":{"theme":"Nord"},"token":"sk-test-secret","handmade":{"password":"sk-test-secret"}}"#,
        )
        .unwrap();

        let archive = collect_from(&paths, BackupSelection::default()).unwrap();
        let settings = archive
            .entries
            .iter()
            .find(|entry| entry.name == "settings.json")
            .expect("设置类别必须入包");
        let text = String::from_utf8_lossy(&settings.bytes);
        assert!(!text.contains("sk-test-secret"), "敏感值必须从导出中剥除");
        assert!(!text.contains("handmade"), "未知命名空间不得导出");
        assert!(!text.contains("\"token\""));

        let packet = seal(&archive, PASSPHRASE).unwrap();
        let opened = open(&packet, PASSPHRASE).unwrap();
        assert_eq!(opened, archive, "打开只还原已投影的归档内容");
        let opened_text = String::from_utf8_lossy(&opened.entries[0].bytes);
        assert!(!opened_text.contains("sk-test-secret"));
    }

    /// 工作区物理文件与学习历史永不进入设置类别。
    #[test]
    fn backup_excludes_local_workspace_and_credentials() {
        let directory = tempfile::tempdir().unwrap();
        let paths = split_paths(directory.path());
        fs::create_dir_all(paths.config_root()).unwrap();
        fs::create_dir_all(paths.local_root()).unwrap();
        fs::write(
            paths.config_root().join("settings.json"),
            br#"{"appearance":{"theme":"Nord"}}"#,
        )
        .unwrap();
        fs::write(paths.local_root().join("session.json"), br#"{"version":1}"#).unwrap();
        fs::write(paths.local_root().join("workspace.db"), b"sqlite").unwrap();
        fs::write(paths.local_root().join("history.jsonl"), b"cmd").unwrap();
        fs::write(paths.local_root().join("learning.db"), b"learning").unwrap();

        let archive = collect_from(&paths, BackupSelection::default()).unwrap();
        let names: HashSet<_> = archive
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(names, HashSet::from(["settings.json"]));
    }

    #[test]
    fn archive_rejects_unknown_fields_and_empty_selection() {
        let archive = archive();
        let mut value = serde_json::to_value(&archive).unwrap();
        value["extra"] = serde_json::json!(1);
        assert!(
            serde_json::from_value::<BackupArchive>(value).is_err(),
            "归档 DTO 必须拒绝未知字段"
        );
        let mut entry_extra = serde_json::to_value(&archive).unwrap();
        entry_extra["entries"][0]["extra"] = serde_json::json!(1);
        assert!(
            serde_json::from_value::<BackupArchive>(entry_extra).is_err(),
            "条目 DTO 必须拒绝未知字段"
        );
        let empty = BackupArchive {
            manifest: BackupManifest {
                version: ARCHIVE_VERSION,
                categories: Vec::new(),
            },
            entries: Vec::new(),
        };
        assert!(validate_archive(&empty).is_err());
        assert!(
            collect_from(
                &single_root_paths(tempfile::tempdir().unwrap().path()),
                BackupSelection { settings: false },
            )
            .is_err()
        );
    }

    #[test]
    fn archive_plaintext_is_bounded_to_64_mib() {
        let oversized = vec![b'a'; MAX_ARCHIVE_PLAINTEXT_BYTES + 1];
        assert!(encrypt_bytes(&oversized, PASSPHRASE).is_err());

        let mut packet = Vec::new();
        packet.extend_from_slice(MAGIC);
        packet.extend_from_slice(&[0u8; SALT_LEN + NONCE_LEN]);
        packet.extend(std::iter::repeat_n(
            0u8,
            MAX_ARCHIVE_PLAINTEXT_BYTES + 16 + 1,
        ));
        assert!(
            decrypt_bytes(&packet, PASSPHRASE)
                .unwrap_err()
                .contains("64 MiB")
        );
    }

    #[test]
    fn seal_open_round_trips_every_category() {
        let archive = full_archive();
        assert!(validate_archive(&archive).is_ok());
        let packet = seal(&archive, PASSPHRASE).unwrap();
        assert_eq!(open(&packet, PASSPHRASE).unwrap(), archive);
    }

    #[test]
    fn wrong_passphrase_short_passphrase_and_tampering_are_rejected() {
        let packet = seal(&archive(), PASSPHRASE).expect("密封");
        assert!(open(&packet, "wrong horse").is_err());
        assert!(open(&packet, SHORT_PASSPHRASE).is_err());
        assert!(seal(&archive(), SHORT_PASSPHRASE).is_err());

        let mut bad_magic = packet.clone();
        bad_magic[0] = b'X';
        assert!(open(&bad_magic, PASSPHRASE).is_err());

        let mut bad_ciphertext = packet.clone();
        let last = bad_ciphertext.len() - 1;
        bad_ciphertext[last] ^= 0x01;
        assert!(open(&bad_ciphertext, PASSPHRASE).is_err());

        let mut bad_salt = packet.clone();
        bad_salt[MAGIC.len()] ^= 0x01;
        assert!(open(&bad_salt, PASSPHRASE).is_err());

        assert!(open(&packet[..packet.len() - 1], PASSPHRASE).is_err());
        assert!(open(&packet[..MAGIC.len() + SALT_LEN + NONCE_LEN], PASSPHRASE).is_err());
        assert!(open(&[], PASSPHRASE).is_err());
    }

    #[test]
    fn legacy_magic_only_aad_packet_is_rejected() {
        // 旧口径只把魔数作 AAD；当前合同要求完整头部（魔数｜盐｜nonce）入 AAD，
        // 旧包必须作为格式不符被拒绝，避免两套认证范围并存。
        let mut salt = [0; SALT_LEN];
        let mut nonce = [0; NONCE_LEN];
        getrandom::fill(&mut salt).unwrap();
        getrandom::fill(&mut nonce).unwrap();
        let key = Zeroizing::new(derive_key(PASSPHRASE, &salt).unwrap());
        let plaintext = serde_json::to_vec(&archive()).unwrap();
        let ciphertext = Aes256Gcm::new((&*key).into())
            .encrypt(
                &Nonce::try_from(&nonce[..]).unwrap(),
                Payload {
                    msg: &plaintext,
                    aad: MAGIC,
                },
            )
            .unwrap();
        let mut packet = Vec::new();
        packet.extend_from_slice(MAGIC);
        packet.extend_from_slice(&salt);
        packet.extend_from_slice(&nonce);
        packet.extend_from_slice(&ciphertext);
        let error = open(&packet, PASSPHRASE).unwrap_err();
        assert!(
            error.contains("认证失败"),
            "旧的魔数-only AAD 包必须被拒绝: {error}"
        );
    }

    #[test]
    fn aad_mismatch_is_rejected_even_with_valid_magic_prefix() {
        // 手工构造:魔数前缀正确,但密文用另一串 AAD 密封 —— 认证必须失败。
        let mut salt = [0; SALT_LEN];
        let mut nonce = [0; NONCE_LEN];
        getrandom::fill(&mut salt).unwrap();
        getrandom::fill(&mut nonce).unwrap();
        let key = Zeroizing::new(derive_key(PASSPHRASE, &salt).unwrap());
        let plaintext = serde_json::to_vec(&archive()).unwrap();
        let ciphertext = Aes256Gcm::new((&*key).into())
            .encrypt(
                &Nonce::try_from(&nonce[..]).unwrap(),
                Payload {
                    msg: &plaintext,
                    aad: b"SLTMBK00",
                },
            )
            .unwrap();
        let mut packet = Vec::new();
        packet.extend_from_slice(MAGIC);
        packet.extend_from_slice(&salt);
        packet.extend_from_slice(&nonce);
        packet.extend_from_slice(&ciphertext);
        let error = open(&packet, PASSPHRASE).unwrap_err();
        assert!(
            error.contains("认证失败"),
            "错误消息应指向认证失败: {error}"
        );
    }

    #[test]
    fn unsupported_archive_version_is_rejected_on_both_sides() {
        let mut archive = archive();
        archive.manifest.version = ARCHIVE_VERSION + 1;
        assert!(seal(&archive, PASSPHRASE).is_err());
        assert!(validate_archive(&archive).is_err());
    }

    /// 导出只收登记且非敏感的字段；未知段、未登记键与敏感键都不存在。
    #[test]
    fn collection_uses_registry_export_allowlist() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(
            root.join("settings.json"),
            r#"{
              "appearance": {
                "theme": "Nord",
                "api_key": "sk-test-settings-secret",
                "custom_theme": "acme-dark",
                "nested": {"Access_Token": "sk-test-nested-secret"}
              },
              "system": {"terminal_proxy": true},
              "handmade": {"note": "keep"}
            }"#,
        )
        .unwrap();
        fs::write(root.join("session.json"), r#"{"version":1}"#).unwrap();
        fs::write(root.join("history.jsonl"), "{\"cmd\":\"ls\"}\n").unwrap();
        fs::create_dir(root.join("themes")).unwrap();
        fs::write(root.join(THEME_NAME), theme_bytes()).unwrap();
        fs::write(root.join("learning.db"), b"private-learning-data").unwrap();

        let archive = collect_from(
            &single_root_paths(root),
            BackupSelection::from_categories([BackupCategory::Settings]),
        )
        .expect("收集");
        let names: HashSet<_> = archive
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(names, HashSet::from(["settings.json", THEME_NAME]));

        let settings = archive
            .entries
            .iter()
            .find(|entry| entry.name == "settings.json")
            .unwrap();
        let settings_text = String::from_utf8(settings.bytes.clone()).unwrap();
        assert!(!settings_text.contains("sk-test-settings-secret"));
        assert!(!settings_text.contains("sk-test-nested-secret"));
        assert!(!settings_text.contains("Access_Token"));
        assert!(!settings_text.contains("handmade"), "未知命名空间不得入包");
        assert!(
            !settings_text.contains("custom_theme"),
            "未登记导出的主题引用不得入包"
        );
        assert!(settings_text.contains("Nord"));
        assert!(settings_text.contains("terminal_proxy"));
        assert!(validate_archive(&archive).is_ok());

        let packet = seal(&archive, PASSPHRASE).expect("密封");
        for secret in ["sk-test-settings-secret", "sk-test-nested-secret"] {
            assert!(
                !packet
                    .windows(secret.len())
                    .any(|window| window == secret.as_bytes()),
                "凭据值进入备份: {secret}"
            );
        }
    }

    /// 主题只从配置根收集；本地根同名文件是诱饵。
    #[test]
    fn collection_reads_settings_and_themes_only_from_config_root() {
        let directory = tempfile::tempdir().unwrap();
        let paths = split_paths(directory.path());
        fs::create_dir_all(paths.config_root().join("themes")).unwrap();
        fs::create_dir_all(paths.local_root().join("themes")).unwrap();
        fs::write(paths.settings_path(), br#"{"appearance":{"theme":"Nord"}}"#).unwrap();
        fs::write(paths.config_root().join(THEME_NAME), theme_bytes()).unwrap();
        fs::write(
            paths
                .local_root()
                .join("themes/018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c.slterm-theme.json"),
            theme_bytes(),
        )
        .unwrap();
        fs::write(
            paths.local_root().join("settings.json"),
            br#"{"appearance":{"theme":"Linear"}}"#,
        )
        .unwrap();
        fs::write(paths.local_root().join("session.json"), br#"{"version":1}"#).unwrap();

        let archive = collect_from(
            &paths,
            BackupSelection::from_categories([BackupCategory::Settings]),
        )
        .unwrap();
        let names: HashSet<_> = archive
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(names, HashSet::from(["settings.json", THEME_NAME]));
        let settings = archive
            .entries
            .iter()
            .find(|entry| entry.name == "settings.json")
            .unwrap();
        assert!(
            String::from_utf8(settings.bytes.clone())
                .unwrap()
                .contains("Nord")
        );
    }

    /// 防复发:非法主题文档必须整单失败,不得静默缺件。
    #[test]
    fn invalid_theme_documents_abort_the_backup() {
        let directory = tempfile::tempdir().unwrap();
        let themes = directory.path().join("themes");
        fs::create_dir_all(&themes).unwrap();
        fs::write(
            themes.join("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b.slterm-theme.json"),
            "{ not a theme }",
        )
        .unwrap();
        let error = collect_from(
            &single_root_paths(directory.path()),
            BackupSelection::from_categories([BackupCategory::Settings]),
        )
        .expect_err("非法主题必须中止备份");
        assert!(error.contains("主题"), "{error}");
    }

    #[test]
    fn collection_rejects_empty_selection_unparseable_or_non_file_settings() {
        let directory = tempfile::tempdir().unwrap();
        assert!(
            collect_from(
                &single_root_paths(directory.path()),
                BackupSelection { settings: false },
            )
            .is_err()
        );
        fs::write(directory.path().join("settings.json"), "{ not json").unwrap();
        assert!(
            collect_from(
                &single_root_paths(directory.path()),
                BackupSelection::from_categories([BackupCategory::Settings]),
            )
            .is_err(),
            "损坏设置不得静默跳过或原样入包"
        );
        fs::remove_file(directory.path().join("settings.json")).unwrap();
        fs::create_dir(directory.path().join("settings.json")).unwrap();
        assert!(
            collect_from(
                &single_root_paths(directory.path()),
                BackupSelection::from_categories([BackupCategory::Settings]),
            )
            .is_err(),
            "非普通文件不得作为备份输入"
        );
    }

    #[test]
    fn import_validation_rejects_unknown_or_sensitive_settings() {
        let unknown = BackupArchive {
            manifest: BackupManifest {
                version: ARCHIVE_VERSION,
                categories: vec![BackupCategory::Settings],
            },
            entries: vec![BackupEntry {
                category: BackupCategory::Settings,
                name: "settings.json".into(),
                bytes: br#"{"appearance":{"theme":"Nord"},"handmade":{"token":"sk-test-import"}}"#
                    .to_vec(),
            }],
        };
        assert!(validate_archive(&unknown).is_err(), "未知段必须拒绝");

        let sensitive = BackupArchive {
            entries: vec![BackupEntry {
                category: BackupCategory::Settings,
                name: "settings.json".into(),
                bytes: br#"{"appearance":{"api_key":"sk-test-import"}}"#.to_vec(),
            }],
            ..unknown
        };
        assert!(validate_archive(&sensitive).is_err(), "未登记键必须拒绝");
        assert!(seal(&sensitive, PASSPHRASE).is_err(), "导出侧同样拒收");

        let hidden = BackupArchive {
            entries: vec![BackupEntry {
                category: BackupCategory::Settings,
                name: "settings.json".into(),
                bytes: br#"{"appearance":{"custom_theme":"acme"}}"#.to_vec(),
            }],
            ..archive()
        };
        assert!(validate_archive(&hidden).is_err(), "非导出字段必须拒绝");

        assert!(validate_archive(&archive()).is_ok());
    }

    #[test]
    fn archive_validation_rejects_path_traversal_duplicates_and_unknown_names() {
        let traversal = BackupArchive {
            entries: vec![BackupEntry {
                category: BackupCategory::Settings,
                name: "../outside.json".into(),
                bytes: b"{}".to_vec(),
            }],
            ..archive()
        };
        assert!(validate_archive(&traversal).is_err());

        let wrong_name = BackupArchive {
            entries: vec![BackupEntry {
                category: BackupCategory::Settings,
                name: "session.json".into(),
                bytes: b"{}".to_vec(),
            }],
            ..archive()
        };
        assert!(validate_archive(&wrong_name).is_err());

        let mut duplicate = archive();
        duplicate.entries.push(duplicate.entries[0].clone());
        assert!(validate_archive(&duplicate).is_err());
    }

    #[test]
    fn open_rejects_authentication_failure_without_side_effects() {
        let packet = seal(&full_archive(), PASSPHRASE).unwrap();
        let directory = tempfile::tempdir().unwrap();
        // 鉴权失败在打开阶段即拒绝，不进入任何落盘路径。
        assert!(open(&packet, "wrong horse").is_err());
        let mut tampered = packet;
        let last = tampered.len() - 1;
        tampered[last] ^= 0xFF;
        assert!(open(&tampered, PASSPHRASE).is_err());
        let names: Vec<_> = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert!(names.is_empty(), "鉴权失败不得产生任何文件: {names:?}");
    }

    #[test]
    fn summary_counts_files_and_bytes_per_category() {
        let archive = full_archive();
        let summary = archive.summary();
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].category, BackupCategory::Settings);
        assert_eq!(summary[0].files, 2);
        let expected = archive
            .entries
            .iter()
            .map(|entry| entry.bytes.len())
            .sum::<usize>();
        assert_eq!(summary[0].bytes, expected as u64);
    }
}
