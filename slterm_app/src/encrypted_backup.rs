//! 口令保护的本地加密备份。
//!
//! 本模块刻意不遍历用户 home 目录:下方白名单即导出与导入的安全边界
//! (白名单即安全边界,枚举即授权)。AI 会话库、环境变量、凭据库、token
//! 来源文件(`~/.claude/settings.json`)永不读写进出备份;清单外文件不存在
//! 「顺手带上」的通道,导入导出同一道边界。
//!
//! 格式:`TRMBK001 | salt(16) | nonce(12) | ciphertext`。魔数同时作
//! AES-256-GCM 的 AAD,格式身份与密文认证绑死——魔数不符即
//! 「非本应用备份」一票拒解。密钥派生 = Argon2id v0x13,m=19456 KiB、
//! t=2、p=1(契约数值随上游基线)。派生密钥、解密明文与恢复点明文全程
//! `Zeroizing`;解密失败消息不回显口令或任何派生材料。
//!
//! 恢复序:先完整鉴权(`open`)→ `validate_archive` 全量校验 → 才允许
//! 任何落盘。校验任何一条不过整单拒写,半份不可信报文零副作用。
//!
//! 类别清单与敏感键剥除表按键域终态重定:设置/会话/自定义
//! 主题/命令历史/目录历史五类;凭据值永不入包。
//!
//! 消费点:设置页安全分区的加密备份导出已接(类别选择 + masked 口令 +
//! 后台整单收集/密封/原子写,失败明示);恢复与撤回 UI 尚未接线,其生产接缝
//! 保持 [`restore`]/[`recovery::restore_selected`]/[`recovery::undo`]。

use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zeroize::Zeroizing;

pub(crate) mod recovery;

/// 备份格式魔数(已裁:去品牌,8 字节,同时作 AAD)。
const MAGIC: &[u8; 8] = b"TRMBK001";
/// 归档格式版本;不符即「版本不支持」一票拒。
const ARCHIVE_VERSION: u32 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
/// 口令最小长度(chars 计数,契约数值随上游基线);UI 前置校验同源引用。
pub(crate) const MIN_PASSPHRASE_CHARS: usize = 8;

/// 自定义主题文件名后缀(与主题库持久化规则同源)。
const THEME_FILE_SUFFIX: &str = ".slterm-theme.json";

/// 敏感键字面量表(按 06 键域终态重定)。
///
/// 键域终态不含任何凭据键——凭据值只允许存在于 Windows 凭据管理器。
/// 本表是导出清洗与导入复验共用的第二道防线:手改文档塞入的凭据字段一律
/// 在进出备份两面被剥除/拒绝。比较口径 = 去首尾空白后 ASCII 小写精确匹配。
const SENSITIVE_KEYS: &[&str] = &[
    "access_token",
    "accesstoken",
    "api_key",
    "api-key",
    "apikey",
    "auth_token",
    "authtoken",
    "authorization",
    "client_secret",
    "clientsecret",
    "credential",
    "credentials",
    "password",
    "passwd",
    "private_key",
    "privatekey",
    "refresh_token",
    "refreshtoken",
    "secret",
    "session_token",
    "sessiontoken",
    "token",
];

/// 备份类别(已裁:按 06 键域终态重定;凭据域值永不入包)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackupCategory {
    /// `settings.json`(导出侧剥敏感键,导入侧复验)。
    Settings,
    /// `session.json` 会话快照。
    Session,
    /// 自定义主题文档(`themes/*.slterm-theme.json`)。
    Themes,
    /// 命令历史 `history.jsonl`。
    CommandHistory,
    /// 目录 frecency `directory_history.json`。
    DirectoryHistory,
}

/// 类别选择集;默认仅设置一项(保守默认思想随上游基线)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BackupSelection {
    pub settings: bool,
    pub session: bool,
    pub themes: bool,
    pub command_history: bool,
    pub directory_history: bool,
}

impl Default for BackupSelection {
    fn default() -> Self {
        Self {
            settings: true,
            session: false,
            themes: false,
            command_history: false,
            directory_history: false,
        }
    }
}

impl BackupSelection {
    pub(crate) fn is_empty(self) -> bool {
        self.categories().next().is_none()
    }

    pub(crate) fn set(&mut self, category: BackupCategory, enabled: bool) {
        *match category {
            BackupCategory::Settings => &mut self.settings,
            BackupCategory::Session => &mut self.session,
            BackupCategory::Themes => &mut self.themes,
            BackupCategory::CommandHistory => &mut self.command_history,
            BackupCategory::DirectoryHistory => &mut self.directory_history,
        } = enabled;
    }

    /// 读取单类别选中态(UI 开关渲染与 `set` 共用同一字段映射)。
    pub(crate) fn get(self, category: BackupCategory) -> bool {
        match category {
            BackupCategory::Settings => self.settings,
            BackupCategory::Session => self.session,
            BackupCategory::Themes => self.themes,
            BackupCategory::CommandHistory => self.command_history,
            BackupCategory::DirectoryHistory => self.directory_history,
        }
    }

    pub(crate) fn from_categories(categories: impl IntoIterator<Item = BackupCategory>) -> Self {
        let mut selection = Self {
            settings: false,
            session: false,
            themes: false,
            command_history: false,
            directory_history: false,
        };
        for category in categories {
            selection.set(category, true);
        }
        selection
    }

    pub(crate) fn categories(self) -> impl Iterator<Item = BackupCategory> {
        [
            (self.settings, BackupCategory::Settings),
            (self.session, BackupCategory::Session),
            (self.themes, BackupCategory::Themes),
            (self.command_history, BackupCategory::CommandHistory),
            (self.directory_history, BackupCategory::DirectoryHistory),
        ]
        .into_iter()
        .filter_map(|(selected, category)| selected.then_some(category))
    }
}

/// 归档中的单个文件条目;`name` 是相对数据目录的归档名(正斜杠)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BackupEntry {
    pub category: BackupCategory,
    pub name: String,
    pub bytes: Vec<u8>,
}

/// 归档清单。设备名字段不迁(无本地消费方)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BackupManifest {
    pub version: u32,
    pub categories: Vec<BackupCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// 序列化并密封归档 → `TRMBK001 | salt | nonce | ciphertext`。
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
    let mut salt = [0; SALT_LEN];
    let mut nonce = [0; NONCE_LEN];
    getrandom::fill(&mut salt).map_err(|error| format!("系统随机源失败: {error}"))?;
    getrandom::fill(&mut nonce).map_err(|error| format!("系统随机源失败: {error}"))?;
    let key = Zeroizing::new(derive_key(passphrase, &salt)?);
    let ciphertext = Aes256Gcm::new((&*key).into())
        .encrypt(
            &Nonce::try_from(&nonce[..]).map_err(|_| "备份 nonce 无效".to_owned())?,
            Payload {
                msg: plaintext,
                aad: MAGIC,
            },
        )
        .map_err(|_| "备份加密失败".to_owned())?;
    let mut output = Vec::with_capacity(MAGIC.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce);
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
                aad: MAGIC,
            },
        )
        .map_err(|_| "备份认证失败: 口令错误或文件已被篡改".to_owned())?;
    Ok(Zeroizing::new(plaintext))
}

/// 按选择集收集数据目录白名单文件。
pub(crate) fn collect(selection: BackupSelection) -> Result<BackupArchive, String> {
    collect_from(crate::platform::dirs::data_dir(), selection)
}

fn collect_from(root: &Path, selection: BackupSelection) -> Result<BackupArchive, String> {
    if selection.is_empty() {
        return Err("至少选择一个备份类别".to_owned());
    }
    let mut entries = Vec::new();
    for category in selection.categories() {
        match category {
            BackupCategory::Settings => add_file(
                root,
                category,
                "settings.json",
                &mut entries,
                filter_settings,
            )?,
            BackupCategory::Session => {
                add_file(root, category, "session.json", &mut entries, identity)?
            }
            BackupCategory::Themes => add_themes(root, &mut entries)?,
            BackupCategory::CommandHistory => {
                add_file(root, category, "history.jsonl", &mut entries, identity)?
            }
            BackupCategory::DirectoryHistory => add_file(
                root,
                category,
                "directory_history.json",
                &mut entries,
                identity,
            )?,
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

fn identity(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    Ok(bytes)
}

/// 导出侧敏感键剥除(递归,任意深度);文档必须是 JSON 对象。
///
/// 键域终态没有凭据键,本函数只对手改/未知字段生效;解析失败视为不可信
/// 内容,拒绝进入备份而不是原样携带。
fn filter_sensitive_document(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("备份文档不是合法 JSON: {error}"))?;
    if !value.is_object() {
        return Err("备份文档根必须是 JSON 对象".to_owned());
    }
    let filtered = strip_sensitive_keys(value);
    serde_json::to_vec(&filtered).map_err(|error| format!("序列化备份文档失败: {error}"))
}

/// `settings.json` 专用导出清洗:语义同 [`filter_sensitive_document`]。
fn filter_settings(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    filter_sensitive_document(bytes)
}

/// 导入侧复验:文档可解析且不含任何敏感键(与导出清洗同一张表)。
fn document_is_sanitized(bytes: &[u8]) -> bool {
    match serde_json::from_slice::<Value>(bytes) {
        Ok(value) if value.is_object() => strip_sensitive_keys(value.clone()) == value,
        Ok(_) | Err(_) => false,
    }
}

fn strip_sensitive_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(key, _)| !is_sensitive_key(key))
                .map(|(key, value)| (key, strip_sensitive_keys(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(strip_sensitive_keys).collect()),
        other => other,
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.trim().to_ascii_lowercase();
    SENSITIVE_KEYS.contains(&normalized.as_str())
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
fn add_themes(root: &Path, entries: &mut Vec<BackupEntry>) -> Result<(), String> {
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
            category: BackupCategory::Themes,
            name: format!("themes/{file_name}"),
            bytes,
        });
    }
    Ok(())
}

fn is_valid_theme_id(id: &str) -> bool {
    // 主题库 id 校验单点复用(store::validate_id),不复制第二份规则。
    crate::theme_library::ThemeLibraryStore::new(Path::new("."))
        .path_for(id)
        .is_ok()
}

/// 先鉴权后落盘:完整解密校验通过才允许写任何文件。
pub(crate) fn restore(packet: &[u8], passphrase: &str) -> Result<(), String> {
    restore_at(crate::platform::dirs::data_dir(), packet, passphrase)
}

/// 显式根入口(测试注入与工作区导入共用);鉴权失败时零 IO。
fn restore_at(root: &Path, packet: &[u8], passphrase: &str) -> Result<(), String> {
    let archive = open(packet, passphrase)?;
    restore_to(root, &archive)
}

/// 测试注入的恢复故障点(仅在测试构建存在);`None` = 关闭,
/// `Some(name)` = 写该条目前注入失败。
#[cfg(test)]
static RESTORE_FAILPOINT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn restore_to(root: &Path, archive: &BackupArchive) -> Result<(), String> {
    validate_archive(archive)?;
    let paths = archive
        .entries
        .iter()
        .map(|entry| restore_path(root, &entry.name))
        .collect::<Result<Vec<_>, _>>()?;
    for (entry, path) in archive.entries.iter().zip(paths) {
        #[cfg(test)]
        if RESTORE_FAILPOINT.lock().expect("恢复故障点锁").as_deref() == Some(entry.name.as_str())
        {
            return Err("恢复故障注入".to_owned());
        }
        crate::atomic_file::write(&path, &entry.bytes)
            .map_err(|error| format!("恢复 {} 失败: {error}", entry.name))?;
    }
    Ok(())
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
    let mut names = HashSet::new();
    for entry in &archive.entries {
        if !categories.contains(&entry.category) || !names.insert(entry.name.clone()) {
            return Err("备份条目类别未授权或文件名重复".to_owned());
        }
        safe_path(Path::new("."), &entry.name)?;
        let allowed = match entry.category {
            BackupCategory::Settings => {
                entry.name == "settings.json" && document_is_sanitized(&entry.bytes)
            }
            BackupCategory::Session => entry.name == "session.json" && is_json_object(&entry.bytes),
            BackupCategory::Themes => theme_entry_allowed(&entry.name, &entry.bytes),
            BackupCategory::CommandHistory => entry.name == "history.jsonl",
            BackupCategory::DirectoryHistory => {
                entry.name == "directory_history.json" && is_json_object(&entry.bytes)
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

fn is_json_object(bytes: &[u8]) -> bool {
    matches!(serde_json::from_slice::<Value>(bytes), Ok(Value::Object(_)))
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
                categories: vec![
                    BackupCategory::Settings,
                    BackupCategory::Session,
                    BackupCategory::Themes,
                    BackupCategory::CommandHistory,
                    BackupCategory::DirectoryHistory,
                ],
            },
            entries: vec![
                BackupEntry {
                    category: BackupCategory::Settings,
                    name: "settings.json".into(),
                    bytes: br#"{"appearance":{"theme":"Nord"}}"#.to_vec(),
                },
                BackupEntry {
                    category: BackupCategory::Session,
                    name: "session.json".into(),
                    bytes: br#"{"version":1}"#.to_vec(),
                },
                BackupEntry {
                    category: BackupCategory::Themes,
                    name: "themes/nord-copy.slterm-theme.json".into(),
                    bytes: theme_bytes(),
                },
                BackupEntry {
                    category: BackupCategory::CommandHistory,
                    name: "history.jsonl".into(),
                    bytes: b"{\"cmd\":\"cargo test\"}\n".to_vec(),
                },
                BackupEntry {
                    category: BackupCategory::DirectoryHistory,
                    name: "directory_history.json".into(),
                    bytes: br#"{"version":1,"entries":[]}"#.to_vec(),
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

        let empty = BackupSelection {
            settings: false,
            ..selection
        };
        assert!(empty.is_empty());
    }

    #[test]
    fn selection_set_and_from_categories_agree() {
        let mut selection = BackupSelection {
            settings: false,
            ..BackupSelection::default()
        };
        selection.set(BackupCategory::Session, true);
        selection.set(BackupCategory::Themes, true);
        assert!(selection.get(BackupCategory::Session));
        assert_eq!(
            selection.categories().collect::<Vec<_>>(),
            vec![BackupCategory::Session, BackupCategory::Themes]
        );
        assert_eq!(
            BackupSelection::from_categories([BackupCategory::Session, BackupCategory::Themes]),
            selection
        );
        selection.set(BackupCategory::Session, false);
        assert!(!selection.session);
        assert!(!selection.get(BackupCategory::Session));
    }

    #[test]
    fn seal_open_round_trips_every_category() {
        let original = full_archive();
        let plaintext = serde_json::to_vec(&original).expect("序列化");
        let packet = seal(&original, PASSPHRASE).expect("密封");
        assert_eq!(&packet[..8], MAGIC);
        assert_eq!(
            packet.len(),
            MAGIC.len() + SALT_LEN + NONCE_LEN + plaintext.len() + 16,
            "布局 = 魔数|盐|nonce|密文(含 16 字节 GCM tag)"
        );
        assert_eq!(open(&packet, PASSPHRASE).expect("解封"), original);
        // 每个类别的明文标记都只能以密文形态存在。
        for marker in [
            b"settings.json".as_slice(),
            b"session.json".as_slice(),
            b"themes/nord-copy.slterm-theme.json".as_slice(),
            b"history.jsonl".as_slice(),
            b"directory_history.json".as_slice(),
        ] {
            assert!(
                plaintext
                    .windows(marker.len())
                    .any(|window| window == marker),
                "测试标记不在明文归档中: {marker:?}"
            );
            assert!(
                !packet.windows(marker.len()).any(|window| window == marker),
                "密文泄露明文标记: {marker:?}"
            );
        }
    }

    #[test]
    fn wrong_passphrase_short_passphrase_and_tampering_are_rejected() {
        let packet = seal(&archive(), PASSPHRASE).expect("密封");
        assert!(open(&packet, "wrong horse").is_err());
        assert!(open(&packet, SHORT_PASSPHRASE).is_err());
        assert!(seal(&archive(), SHORT_PASSPHRASE).is_err());

        // 魔数错位(改一个字节)。
        let mut bad_magic = packet.clone();
        bad_magic[0] = b'X';
        assert!(open(&bad_magic, PASSPHRASE).is_err());

        // 密文改一字节。
        let mut bad_ciphertext = packet.clone();
        let last = bad_ciphertext.len() - 1;
        bad_ciphertext[last] ^= 0x01;
        assert!(open(&bad_ciphertext, PASSPHRASE).is_err());

        // 盐改一字节 → 派生密钥不同。
        let mut bad_salt = packet.clone();
        bad_salt[MAGIC.len()] ^= 0x01;
        assert!(open(&bad_salt, PASSPHRASE).is_err());

        // 截断:去尾一字节 / 只剩头部。
        assert!(open(&packet[..packet.len() - 1], PASSPHRASE).is_err());
        assert!(open(&packet[..MAGIC.len() + SALT_LEN + NONCE_LEN], PASSPHRASE).is_err());
        assert!(open(&[], PASSPHRASE).is_err());
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
                    aad: b"TRMBK000",
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
        let mut future = archive();
        future.manifest.version = ARCHIVE_VERSION + 1;
        assert!(seal(&future, PASSPHRASE).is_err());

        // 手工加密一个未来版本归档:open 必须在反序列化后拒绝。
        let plaintext = serde_json::to_vec(&future).unwrap();
        let packet = encrypt_bytes(&plaintext, PASSPHRASE).unwrap();
        assert!(open(&packet, PASSPHRASE).is_err());
    }

    #[test]
    fn collection_uses_allowlist_and_strips_sensitive_keys() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(
            root.join("settings.json"),
            r#"{
              "appearance": {
                "theme": "Nord",
                "api_key": "sk-test-settings-secret",
                "nested": {"Access_Token": "sk-test-nested-secret"}
              },
              "system": {"terminal_proxy": true},
              "handmade": {"note": "keep"}
            }"#,
        )
        .unwrap();
        fs::write(root.join("session.json"), r#"{"version":1}"#).unwrap();
        fs::write(root.join("history.jsonl"), "{\"cmd\":\"ls\"}\n").unwrap();
        fs::write(
            root.join("directory_history.json"),
            r#"{"version":1,"entries":[]}"#,
        )
        .unwrap();
        fs::create_dir(root.join("themes")).unwrap();
        fs::write(
            root.join("themes")
                .join(format!("nord-copy{THEME_FILE_SUFFIX}")),
            theme_bytes(),
        )
        .unwrap();
        // 坏主题的硬失败路径由 `invalid_theme_documents_abort_the_backup`
        // 单独覆盖;本用例只验证合法白名单与敏感键剥除。
        // 清单外敏感面:AI 会话库/未知文件/日志/恢复点目录,一律不得入包。
        fs::write(root.join("ai_sessions.json"), "{\"sessions\":[]}").unwrap();
        fs::write(root.join("obsolete.txt"), "enabled=true").unwrap();
        fs::write(root.join("slterm_debug.log"), "log").unwrap();
        fs::create_dir(root.join("backup-recovery")).unwrap();

        let selection = BackupSelection::from_categories([
            BackupCategory::Settings,
            BackupCategory::Session,
            BackupCategory::Themes,
            BackupCategory::CommandHistory,
            BackupCategory::DirectoryHistory,
        ]);
        let archive = collect_from(root, selection).expect("收集");
        let names: HashSet<_> = archive
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(
            names,
            HashSet::from([
                "settings.json",
                "session.json",
                "themes/nord-copy.slterm-theme.json",
                "history.jsonl",
                "directory_history.json"
            ])
        );

        let settings = archive
            .entries
            .iter()
            .find(|entry| entry.name == "settings.json")
            .unwrap();
        let settings_text = String::from_utf8(settings.bytes.clone()).unwrap();
        assert!(!settings_text.contains("sk-test-settings-secret"));
        assert!(!settings_text.contains("sk-test-nested-secret"));
        assert!(settings_text.contains("Nord"));
        assert!(settings_text.contains("terminal_proxy"));
        assert!(settings_text.contains("keep"), "未知段原样保留");
        assert!(
            !settings_text.contains("Access_Token"),
            "大小写变体同样剥除"
        );

        // 整个归档密封后仍不含任何假凭据。
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

    /// 防复发:非法主题文档必须整单失败,不得静默缺件。
    #[test]
    fn invalid_theme_documents_abort_the_backup() {
        let directory = tempfile::tempdir().unwrap();
        let themes = directory.path().join("themes");
        fs::create_dir_all(&themes).unwrap();
        fs::write(themes.join("bad.slterm-theme.json"), "{ not a theme }").unwrap();
        let selection = BackupSelection::from_categories([BackupCategory::Themes]);
        let error = collect_from(directory.path(), selection).expect_err("非法主题必须中止备份");
        assert!(error.contains("主题"), "{error}");
    }

    #[test]
    fn collection_rejects_empty_selection_and_unparseable_settings() {
        let directory = tempfile::tempdir().unwrap();
        assert!(collect_from(directory.path(), BackupSelection::from_categories([])).is_err());
        fs::write(directory.path().join("settings.json"), "{ not json").unwrap();
        assert!(collect_from(directory.path(), BackupSelection::default()).is_err());
        // 可解析但根非对象:设置文档合同是对象,四态读取会判损坏,备份拒收。
        fs::write(directory.path().join("settings.json"), "[1, 2, 3]").unwrap();
        assert!(collect_from(directory.path(), BackupSelection::default()).is_err());
    }

    #[test]
    fn registered_setting_keys_never_collide_with_the_sensitive_table() {
        for def in slterm_settings::keys::KEYS {
            assert!(
                !is_sensitive_key(def.key),
                "注册键 {} 与敏感键剥除表冲突",
                def.key
            );
        }
        assert!(is_sensitive_key("api_key"));
    }

    #[test]
    fn sensitive_key_matching_normalizes_case_hyphens_and_padding() {
        for key in ["API_KEY", "ApiKey", "api-key", "  token  ", "Authorization"] {
            assert!(is_sensitive_key(key), "未命中敏感键: {key}");
        }
        for key in ["theme", "terminal_proxy"] {
            assert!(!is_sensitive_key(key), "误伤普通键: {key}");
        }
    }

    #[test]
    fn restore_rejects_unsanitized_settings_before_writing() {
        let archive = BackupArchive {
            manifest: BackupManifest {
                version: ARCHIVE_VERSION,
                categories: vec![BackupCategory::Settings],
            },
            entries: vec![BackupEntry {
                category: BackupCategory::Settings,
                name: "settings.json".into(),
                bytes: br#"{"appearance":{"api_key":"sk-test-import"}}"#.to_vec(),
            }],
        };
        let directory = tempfile::tempdir().unwrap();
        assert!(restore_to(directory.path(), &archive).is_err());
        assert!(
            !directory.path().join("settings.json").exists(),
            "导入复验失败必须在落盘前拒绝"
        );
        assert!(seal(&archive, PASSPHRASE).is_err(), "导出侧同样拒收");
    }

    #[test]
    fn restore_rejects_path_traversal_duplicates_and_unknown_names() {
        let directory = tempfile::tempdir().unwrap();
        let traversal = BackupArchive {
            manifest: BackupManifest {
                version: ARCHIVE_VERSION,
                categories: vec![BackupCategory::Settings],
            },
            entries: vec![BackupEntry {
                category: BackupCategory::Settings,
                name: "../outside.json".into(),
                bytes: b"{}".to_vec(),
            }],
        };
        assert!(restore_to(directory.path(), &traversal).is_err());
        assert!(
            !directory
                .path()
                .parent()
                .unwrap()
                .join("outside.json")
                .exists()
        );

        let unsigned_name = BackupArchive {
            manifest: BackupManifest {
                version: ARCHIVE_VERSION,
                categories: vec![BackupCategory::Session],
            },
            entries: vec![BackupEntry {
                category: BackupCategory::Session,
                name: "settings.json".into(),
                bytes: b"{}".to_vec(),
            }],
        };
        assert!(restore_to(directory.path(), &unsigned_name).is_err());

        let mut duplicate = archive();
        duplicate.entries.push(duplicate.entries[0].clone());
        assert!(restore_to(directory.path(), &duplicate).is_err());
        assert!(!directory.path().join("settings.json").exists());

        let unlisted_category = BackupArchive {
            manifest: BackupManifest {
                version: ARCHIVE_VERSION,
                categories: vec![BackupCategory::Session],
            },
            entries: vec![BackupEntry {
                category: BackupCategory::Settings,
                name: "settings.json".into(),
                bytes: b"{}".to_vec(),
            }],
        };
        assert!(restore_to(directory.path(), &unlisted_category).is_err());
    }

    #[test]
    fn restore_to_writes_every_category_and_packet_restore_round_trips() {
        let directory = tempfile::tempdir().unwrap();
        let original = full_archive();
        restore_to(directory.path(), &original).expect("恢复");
        assert!(directory.path().join("settings.json").is_file());
        assert!(directory.path().join("session.json").is_file());
        assert!(
            directory
                .path()
                .join("themes/nord-copy.slterm-theme.json")
                .is_file()
        );
        assert!(directory.path().join("history.jsonl").is_file());
        assert!(directory.path().join("directory_history.json").is_file());

        // 整包往返:open 后的归档与 restore_to 的落盘字节一致。
        let packet = seal(&original, PASSPHRASE).unwrap();
        let opened = open(&packet, PASSPHRASE).unwrap();
        let second = tempfile::tempdir().unwrap();
        restore_to(second.path(), &opened).unwrap();
        for entry in &opened.entries {
            assert_eq!(
                fs::read(second.path().join(&entry.name)).unwrap(),
                entry.bytes
            );
        }
    }

    #[test]
    fn restore_rejects_authentication_failure_without_side_effects() {
        let packet = seal(&full_archive(), PASSPHRASE).unwrap();
        let directory = tempfile::tempdir().unwrap();
        // 显式根入口证明鉴权先于任何落盘:错口令/篡改都在零 IO 下拒绝。
        assert!(restore_at(directory.path(), &packet, "wrong horse").is_err());
        let mut tampered = packet;
        let last = tampered.len() - 1;
        tampered[last] ^= 0xFF;
        assert!(restore_at(directory.path(), &tampered, PASSPHRASE).is_err());
        let names: Vec<_> = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert!(names.is_empty(), "鉴权失败不得产生任何文件: {names:?}");
    }

    #[test]
    fn summary_counts_files_and_bytes_per_category() {
        let summary = full_archive().summary();
        assert_eq!(summary.len(), 5);
        let settings = summary
            .iter()
            .find(|item| item.category == BackupCategory::Settings)
            .unwrap();
        assert_eq!(settings.files, 1);
        assert_eq!(
            settings.bytes,
            br#"{"appearance":{"theme":"Nord"}}"#.len() as u64
        );
    }
}
