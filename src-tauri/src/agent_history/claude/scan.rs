//! claude 历史会话扫描 —— 扫描根单点 + 会话收集（BE-02/BE-05/BE-06/BE-19，MC-301/305 下沉）
//!
//! 职责：
//! - `resolve_projects_root()`：扫描根单点（SEC-02 约束面 / BE-06 实现面，MC-305）
//! - `scan_sessions()`：遍历扫描根一级子目录收集会话元数据（provider impl 调用，
//!   命令 `agent_history_scan` 在聚合层 mod.rs 按 cliId 分发）
//! - BE-19 缓存：扫描结果按目录内容指纹进程内缓存——键不变命中则复用，不重复读盘；
//!   会话文件增删改（文件 mtime/len 或文件集合变化）→ 指纹变化 → 自动失效
//!   （CP-007 指纹口径，失效单位 = 会话文件）。`scan_sessions_with_force(force)`
//!   供命令层 force 通道强制直扫（不触缓存）
//!
//! 排除规则（规格 3.1）：`agent-*.jsonl` 平铺形态、文件名主干非 UUID 者；
//! 不递归子目录（`<id>/subagents/` 天然不命中）。
//! 容错：单文件解析失败 → 降级条目；扫描根不存在 → 空 Vec（新机无 claude 数据属正常）。
//! env 覆盖 `SLTERM_CLAUDE_PROJECTS_DIR` 留 provider 内部（MC-305：聚合层不假设
//! env 命名——未来 `SLTERM_<CLI>_PROJECTS_DIR` 同款模式自管）。

use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::agent_history::claude::jsonl;
use crate::agent_history::{is_uuid_filename, AgentHistorySession};

/// 扫描根解析单点（SEC-02/BE-06，MC-305：env 覆盖留 provider 内部）
///
/// 解析顺序：`SLTERM_CLAUDE_PROJECTS_DIR` env 非空 → 用之；
/// 否则 `crate::home::home_dir()/.claude/projects`（顶层共享 home 解析——
/// E2E 假 home 隔离下 fallback 落假屋空目录，不会静默扫真实用户历史）。
/// 每次调用时读取 env（不缓存）——E2E 进程继承 env 即可生效。
/// **生产不设置此 env，仅测试用途**（E2E fixture 隔离，防止测试触碰真实用户数据）。
pub fn resolve_projects_root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("SLTERM_CLAUDE_PROJECTS_DIR").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    crate::home::home_dir().map(|home| home.join(".claude").join("projects"))
}

/// 遍历扫描根一级子目录，收集其中 UUID 形态的顶层 *.jsonl 会话（trait 路径入口）
///
/// BE-19 缓存：扫描结果按目录内容指纹进程内缓存——键不变命中则复用，不重复读盘；
/// 会话文件增删改（文件 mtime/len 或文件集合变化）→ 指纹变化 → 自动失效重扫
/// （CP-007 指纹口径：失效单位 = 会话文件）。
pub(crate) fn scan_sessions() -> Vec<AgentHistorySession> {
    cached_scan()
}

/// 扫描入口（BE-19 契约 force 通道）：`force=true` 显式全量直扫
///
/// 命令层 `agent_history_scan(cliId, force)` 的 force 经 mod.rs `run_scan` 分发至此；
/// trait `scan()` 无 force 参数（注册表路径恒走 `scan_sessions()`）。
/// force 路径不读键、不回填缓存：键收集 = 两级 read_dir + 全量文件 stat（与重扫
/// 同量级成本），前端每 tick 恒 force（sessionRefreshTask），承担不起——CP-007
/// 实测该键成本后 force 与缓存解耦；不回填无碍正确性：内容变则键必变（文件级
/// mtime/len），后续非 force 调用自愈重扫。
pub(crate) fn scan_sessions_with_force(force: bool) -> Vec<AgentHistorySession> {
    if force {
        let Some(root) = resolve_projects_root() else {
            return Vec::new(); // 无法解析扫描根（无 home 目录）
        };
        if !root.is_dir() {
            return Vec::new(); // 扫描根不存在（新机无 claude 数据）→ 空
        }
        return scan_sessions_uncached(&root); // 显式直扫，不触缓存
    }
    cached_scan()
}

// ── BE-19 进程内扫描缓存 ──

/// 缓存键 = root 目录内容指纹（CP-007 指纹口径，失效单位 = 会话文件）
///
/// 收集 root 一级目录名清单 + 各一级目录内会话文件条目的 `(file_name, mtime_ms,
/// len)` → 两级排序 → FNV-1a 64 位哈希 + 两级条目数。失效精度 = 目录内容级：
/// 一级目录增删/改名、目录内会话文件增删改均改变指纹 → 缓存自动失效。旧键
/// (目录 mtime, 文件数) 对目录内变更不敏感（曾由前端恒 force=true 兜底）——
/// CP-007 实测 1000 会话全扫不达标后改指纹口径，锁死失效精度。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScanCacheKey {
    /// root 目录内容指纹（FNV-1a 64 位，含两级条目数）
    pub(crate) fingerprint: u64,
}

/// 缓存条目（单槽：键含 root，不同扫描根互不冲突，换根即重扫回填）
struct ScanCacheEntry {
    /// 扫描根（条目归属，防不同根串扰）
    root: PathBuf,
    /// 契约键（目录内容指纹）
    key: ScanCacheKey,
    /// 缓存扫描结果
    sessions: Vec<AgentHistorySession>,
}

/// 进程内扫描缓存（BE-19；单槽，键含 root——跨根命中恒为 miss，仅成本为重扫）
static SCAN_CACHE: OnceLock<Mutex<Option<ScanCacheEntry>>> = OnceLock::new();

/// 带缓存的扫描核心（BE-19）
///
/// 键不变命中则复用缓存结果（不重复读盘）；键变化 → 全量重扫并回填。
/// `force=true` 走显式直扫路径（见 `scan_sessions_with_force`），不触缓存。
/// 扫描根缺失/不可读不写缓存，保持既有降级语义（空 Vec）。
fn cached_scan() -> Vec<AgentHistorySession> {
    let Some(root) = resolve_projects_root() else {
        return Vec::new(); // 无法解析扫描根（无 home 目录）
    };
    if !root.is_dir() {
        return Vec::new(); // 扫描根不存在（新机无 claude 数据）→ 空
    }
    let Some(key) = cache_key_of(&root) else {
        return Vec::new(); // 根目录不可读 → 空（降级语义与既有 scan 一致）
    };
    let cache = SCAN_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache.lock();
    if let Some(entry) = guard.as_ref() {
        if entry.root == root && entry.key == key {
            return entry.sessions.clone(); // 缓存命中：不重复读盘
        }
    }
    let sessions = scan_sessions_uncached(&root);
    *guard = Some(ScanCacheEntry {
        root,
        key,
        sessions: sessions.clone(), // 回填缓存后返回（命中路径复用同一 Vec）
    });
    sessions
}

/// 计算契约缓存键 = root 目录内容指纹（CP-007 指纹口径，失效单位 = 会话文件）
///
/// 收集 root 一级目录名清单 + 各一级目录内会话文件条目的 `(file_name, mtime_ms,
/// len)`，两级均排序后经 FNV-1a 64 位逐项混合 + 两级条目数。失效精度 = 目录内容级：
/// 一级目录增删/改名、目录内会话文件增删改均改变指纹（文件删除/新增即时反映于
/// read_dir 列表；内容修改即时反映于文件自身 mtime/len——parse 侧既有用例依赖）。
/// 文件级而非目录级属性：Windows 下 Rust 读目录自身 mtime 对子文件增删的更新
/// 不可靠（9-8 实测 ≥800ms 不刷新、目录 len 恒 0），目录级信号会漏检失效。
/// 键计算成本 = read_dir(root + 各一级目录) + 文件 metadata（无 jsonl 打开/解析）。
/// 单条目读取失败（并发删除/mtime 竞态）跳过——指纹变化方向偏失效重扫，绝不让
/// 单条目异常把整条扫描降级为空；根目录不可读 → None（调用方降级空 Vec）。
fn cache_key_of(root: &Path) -> Option<ScanCacheKey> {
    let mut dirs: Vec<Vec<u8>> = Vec::new();
    let mut files: Vec<(Vec<u8>, u64, u64)> = Vec::new();
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let dir_path = entry.path();
        if !dir_path.is_dir() {
            continue; // 扫描口径：仅一级目录承载会话
        }
        let dir_name = entry
            .file_name()
            .to_string_lossy()
            .into_owned()
            .into_bytes();
        dirs.push(dir_name);
        let Ok(sub_entries) = std::fs::read_dir(&dir_path) else {
            continue; // 目录不可读视为空（指纹变 → 失效重扫，不降级整条为空）
        };
        for file in sub_entries.flatten() {
            let file_path = file.path();
            if !is_session_jsonl(&file_path) {
                continue; // 指纹口径 = 会话内容（非会话文件变化不失效，与扫描一致）
            }
            let Ok(meta) = file.metadata() else {
                continue; // 读取失败视为该文件缺席（指纹变 → 失效重扫）
            };
            let mtime_ms = match meta.modified() {
                Ok(t) => t
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0), // epoch 前时钟边界 → 0（照 file_mtime_ms 先例）
                Err(_) => continue, // mtime 不可读同视为文件缺席
            };
            files.push((
                file.file_name().to_string_lossy().into_owned().into_bytes(),
                mtime_ms,
                meta.len(),
            ));
        }
    }
    dirs.sort();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a 64 offset basis
    for name in &dirs {
        hash = fnv1a64(name, hash);
    }
    for (name, mtime_ms, len) in &files {
        hash = fnv1a64(name, hash);
        hash = fnv1a64(&mtime_ms.to_le_bytes(), hash);
        hash = fnv1a64(&len.to_le_bytes(), hash);
    }
    hash = fnv1a64(&(dirs.len() as u64).to_le_bytes(), hash); // 一级目录数入指纹
    hash = fnv1a64(&(files.len() as u64).to_le_bytes(), hash); // 会话文件数入指纹
    Some(ScanCacheKey { fingerprint: hash })
}

/// FNV-1a 64 位哈希单步混合（缓存指纹混合原语，CP-007）
fn fnv1a64(bytes: &[u8], hash: u64) -> u64 {
    bytes.iter().fold(hash, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// 全量扫描（无缓存）：遍历扫描根一级子目录，收集其中 UUID 形态的顶层 *.jsonl 会话
///
/// 无 Err 通道：任何失败（扫描根缺失/目录不可读/单文件解析失败）均降级为空或
/// 降级条目——聚合层「单 provider 失败不阻塞其他」由此保证（MC-303）。
fn scan_sessions_uncached(root: &Path) -> Vec<AgentHistorySession> {
    let mut sessions = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        let dir_path = entry.path();
        if !dir_path.is_dir() {
            continue;
        }
        // 只扫一级子目录的直属文件，不递归（subagents/ 子目录天然不命中）
        let Ok(files) = std::fs::read_dir(&dir_path) else {
            continue;
        };
        for file in files.flatten() {
            let file_path = file.path();
            if !is_session_jsonl(&file_path) {
                continue;
            }
            sessions.push(parse_session_file(&file_path));
        }
    }
    sessions
}

/// 判定是否为会话 jsonl：扩展名 jsonl + 文件名主干 UUID 形态 + 非 agent- 平铺
fn is_session_jsonl(path: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
        return false;
    }
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    !stem.starts_with("agent-") && is_uuid_filename(stem)
}

/// 解析单个会话文件 → AgentHistorySession（cli_id 打标 "claude"，MC-302）
///
/// 任何解析失败不返回 Err——降级为仅 sessionId + mtime_ms 的条目，
/// 其余字段 None / titleSource=none / cwdExists=false（BE-02 降级契约）。
fn parse_session_file(path: &Path) -> AgentHistorySession {
    let session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();
    let mtime_ms = file_mtime_ms(path);
    // 头部解析 + 尾部标题（均容错：任何失败返回空候选，不 panic）
    let head = jsonl::parse_head(path);
    let tail = jsonl::parse_tail_title(path);
    let (title, title_source) = jsonl::resolve_title(&head, tail);
    // cwd 一律从 JSONL 内容解析（目录名只是 cwd 的有损编码，禁止反解码）
    let cwd = head.cwd;
    let cwd_exists = cwd.as_ref().map(|c| Path::new(c).is_dir()).unwrap_or(false);
    AgentHistorySession {
        session_id,
        cwd,
        title,
        // 内部枚举 → DTO 开放字符串（claude 值集；UI 不消费具体值，MC-302）
        title_source: title_source.as_str().to_string(),
        first_prompt: head.first_prompt,
        mtime_ms,
        cwd_exists,
        // provider 打标（provider 内部写字面量合法，MC-302）
        cli_id: "claude".to_string(),
    }
}

/// 文件修改时间转毫秒时间戳（决策 26）；metadata 失败 → 0（BE-05）
fn file_mtime_ms(path: &Path) -> u64 {
    let modified = match std::fs::metadata(path).and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return 0,
    };
    modified
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod scan_tests {
    use super::*;
    use crate::agent_history::claude::{ScanRootGuard, TitleSource};
    use std::io::Write;

    // ── 测试辅助 ──

    /// 创建扫描根 + 一个编码目录（cwd 编码形态，如 C--Users-test-app）
    /// 返回 (TempDir 守卫, 规范化扫描根路径, 编码目录路径)
    ///
    /// 路径经 dunce::canonicalize 统一长名（8.3 短名坑，照 git/CLAUDE.md 先例）。
    fn make_scan_root() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(dir.path()).unwrap();
        let proj = root.join("C--Users-test-app");
        std::fs::create_dir_all(&proj).unwrap();
        (dir, root, proj)
    }

    /// 在编码目录下写一个有效会话文件（UUID 文件名 + summary 首行 + user prompt 行）
    /// 内容经 serde_json 序列化保证 JSON 转义正确（Windows 路径含反斜杠）
    fn write_valid_session(proj: &Path, uuid: &str) {
        let content = serde_json::json!({
            "type": "summary",
            "summary": "修复登录 bug",
            "leafUuid": "x",
        })
        .to_string()
            + "\n"
            + &serde_json::json!({
                "type": "user",
                "cwd": "C:\\test\\app",
                "message": { "content": "帮我修 bug" },
            })
            .to_string();
        std::fs::write(proj.join(format!("{uuid}.jsonl")), content).unwrap();
    }

    // ── resolve_projects_root（SEC-02/BE-06，MC-305：env 覆盖留 provider 内部） ──

    #[test]
    fn resolve_root_env_override() {
        // env 指向 tempdir → 用之（依赖 --test-threads=1 门禁；guard 测毕自动恢复）
        let dir = tempfile::tempdir().unwrap();
        let canon = dunce::canonicalize(dir.path()).unwrap();
        let _guard = ScanRootGuard::set(&canon);
        let root = resolve_projects_root().unwrap();
        assert_eq!(root, canon);
    }

    #[test]
    fn resolve_root_empty_env_falls_back_to_home() {
        // env 为空串 → 回退 crate::home/.claude/projects（双守卫：ScanRootGuard("")
        // 清 env + HomeDirGuard 注入假 home——E2E 假 home 隔离下 fallback 语义）
        let _guard = ScanRootGuard::set("");
        let home = tempfile::tempdir().unwrap();
        let _home_guard = crate::home::HomeDirGuard::set(home.path());
        let root = resolve_projects_root().unwrap();
        assert_eq!(root, home.path().join(".claude").join("projects"));
    }

    #[test]
    fn resolve_root_default_without_env() {
        // 未设 env → crate::home/.claude/projects（双守卫同上一例，unset 变体）
        let _guard = ScanRootGuard::unset();
        let home = tempfile::tempdir().unwrap();
        let _home_guard = crate::home::HomeDirGuard::set(home.path());
        let root = resolve_projects_root().unwrap();
        assert_eq!(root, home.path().join(".claude").join("projects"));
    }

    #[test]
    fn resolve_root_env_beats_userprofile() {
        // 优先级契约（防回归）：SLTERM_CLAUDE_PROJECTS_DIR env 恒胜于
        // USERPROFILE 假 home——E2E fixture 副本重定向不被假屋吞掉
        let dir = tempfile::tempdir().unwrap();
        let canon = dunce::canonicalize(dir.path()).unwrap();
        let _guard = ScanRootGuard::set(&canon);
        let home = tempfile::tempdir().unwrap();
        let _home_guard = crate::home::HomeDirGuard::set(home.path());
        let root = resolve_projects_root().unwrap();
        assert_eq!(root, canon, "env 重定向应优先于共享 home 解析");
    }

    // ── scan_sessions：排除规则 ──

    #[test]
    fn scan_excludes_agent_non_uuid_subagents() {
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        write_valid_session(&proj, uuid);
        // 排除 1：agent-*.jsonl 平铺形态
        std::fs::write(proj.join("agent-abc123.jsonl"), "{}").unwrap();
        // 排除 2：非 UUID 文件名主干
        std::fs::write(proj.join("not-a-uuid.jsonl"), "{}").unwrap();
        // 排除 3：subagents 子目录（不递归，天然不命中）
        let sub = proj.join(format!("{uuid}")).join("subagents");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("agent-def456.jsonl"), "{}").unwrap();
        // 非 jsonl 扩展名
        std::fs::write(proj.join(format!("{uuid}.txt")), "{}").unwrap();

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        assert_eq!(
            sessions.len(),
            1,
            "应仅命中 1 条 UUID 会话，实际: {:?}",
            sessions.len()
        );
        assert_eq!(sessions[0].session_id, uuid);
    }

    #[test]
    fn scan_collects_all_sessions_across_dirs() {
        // 多个编码目录 + 多个会话 → 全部收集（HFN-09②：原名暗示测顺序——扫描顺序无契约，
        // 排序是前端职责，仅断言集合）
        let (_dir, root, proj) = make_scan_root();
        let uuid1 = "123e4567-e89b-12d3-a456-426614174001";
        let uuid2 = "123e4567-e89b-12d3-a456-426614174002";
        write_valid_session(&proj, uuid1);
        write_valid_session(&proj, uuid2);
        // 第二个编码目录
        let proj2 = root.join("D--other-app");
        std::fs::create_dir_all(&proj2).unwrap();
        let uuid3 = "123e4567-e89b-12d3-a456-426614174003";
        write_valid_session(&proj2, uuid3);

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        let mut ids: Vec<&str> = sessions.iter().map(|s| s.session_id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, [uuid1, uuid2, uuid3]);
    }

    #[test]
    fn scan_root_missing_returns_empty() {
        // 扫描根不存在 → 空 Vec（非 Err）
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("不存在");
        let _guard = ScanRootGuard::set(&missing);
        let sessions = scan_sessions();
        assert!(sessions.is_empty());
    }

    // ── parse_session_file：降级条目 ──

    #[test]
    fn scan_corrupt_jsonl_produces_degraded_entry() {
        // 损坏 jsonl → 降级条目：仅 sessionId + mtime_ms，其余 None/none/false
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        std::fs::write(proj.join(format!("{uuid}.jsonl")), "{broken json 没有换行").unwrap();

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.session_id, uuid);
        assert!(s.cwd.is_none());
        assert!(s.title.is_none());
        assert_eq!(s.title_source, TitleSource::None.as_str());
        assert!(s.first_prompt.is_none());
        assert!(s.mtime_ms > 0, "降级条目应保留文件 mtime");
        assert!(!s.cwd_exists);
    }

    #[test]
    fn scan_empty_jsonl_produces_degraded_entry() {
        // 空文件 → 同样降级（不 panic）
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        std::fs::write(proj.join(format!("{uuid}.jsonl")), "").unwrap();

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, uuid);
        assert!(sessions[0].title.is_none());
    }

    #[test]
    fn scan_unreadable_session_file_produces_degraded_entry() {
        // IO 降级（HFN-05）：<uuid>.jsonl 路径是目录——条目存在但无法按文件读取
        // （File::open 失败）→ 降级条目，不 panic、不跳过
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        std::fs::create_dir_all(proj.join(format!("{uuid}.jsonl"))).unwrap();

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        assert_eq!(sessions.len(), 1, "不可读条目应降级而非崩溃/被跳过");
        let s = &sessions[0];
        assert_eq!(s.session_id, uuid);
        assert!(s.title.is_none());
        assert!(s.cwd.is_none());
        assert_eq!(s.title_source, TitleSource::None.as_str());
        assert!(!s.cwd_exists);
    }

    #[test]
    fn parse_session_file_missing_file_degraded_zero_mtime() {
        // IO 降级（HFN-05）：metadata 失败（文件不存在）→ mtime_ms=0 + 全字段降级
        let dir = tempfile::tempdir().unwrap();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        let missing = dir.path().join(format!("{uuid}.jsonl"));
        let s = parse_session_file(&missing);
        assert_eq!(s.session_id, uuid);
        assert_eq!(s.mtime_ms, 0, "metadata 失败 → mtime_ms=0");
        assert!(s.title.is_none());
        assert!(s.cwd.is_none());
        assert!(!s.cwd_exists);
    }

    // ── parse_session_file：完整字段 ──

    #[test]
    fn scan_full_session_fields() {
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        // cwd 指向存在的临时目录（验证 cwd_exists=true）
        let existing = dunce::canonicalize(proj.clone()).unwrap();
        let content = serde_json::json!({
            "type": "summary",
            "summary": "修复登录 bug",
            "leafUuid": "x",
        })
        .to_string()
            + "\n"
            + &serde_json::json!({
                "type": "user",
                "cwd": existing.to_string_lossy(),
                "message": { "content": "帮我修 bug" },
            })
            .to_string();
        let path = proj.join(format!("{uuid}.jsonl"));
        std::fs::write(&path, content).unwrap();

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        let s = &sessions[0];
        assert_eq!(s.session_id, uuid);
        // 标题回退链：custom-title/ai-title 均无 → summary 赢
        assert_eq!(s.title.as_deref(), Some("修复登录 bug"));
        assert_eq!(s.title_source, TitleSource::Summary.as_str());
        assert_eq!(s.first_prompt.as_deref(), Some("帮我修 bug"));
        assert!(s.mtime_ms > 0);
        assert_eq!(s.cwd.as_deref(), Some(existing.to_str().unwrap()));
        assert!(s.cwd_exists, "cwd 指向存在的目录 → cwd_exists=true");
        assert_eq!(s.cli_id, "claude", "provider 打标 cliId");
    }

    #[test]
    fn scan_cwd_exists_false_for_missing_dir() {
        // cwd 指向不存在的目录 → cwd_exists=false（孤儿会话）
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        let ghost = proj.join("已被删除");
        let content = serde_json::json!({
            "type": "summary",
            "summary": "孤儿会话",
            "leafUuid": "x",
        })
        .to_string()
            + "\n"
            + &serde_json::json!({
                "type": "user",
                "cwd": ghost.to_string_lossy(),
                "message": { "content": "q" },
            })
            .to_string();
        std::fs::write(proj.join(format!("{uuid}.jsonl")), content).unwrap();

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        assert!(!sessions[0].cwd_exists, "cwd 目录不存在 → cwd_exists=false");
        assert!(sessions[0].cwd.is_some());
    }

    // ── env 覆盖端到端（scan 每次调用读 env，进程继承即可生效） ──

    #[test]
    fn scan_respects_env_override_during_scan() {
        // 先设 env 再 scan → 命中 tempdir 会话（依赖 --test-threads=1 门禁）
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        write_valid_session(&proj, uuid);

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, uuid);
    }

    // ── file_mtime_ms ──

    #[test]
    fn mtime_ms_positive_for_existing_file() {
        let (_dir, _root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        let path = proj.join(format!("{uuid}.jsonl"));
        std::fs::write(&path, "{}").unwrap();
        let m = file_mtime_ms(&path);
        assert!(m > 0, "现有文件 mtime 应为正毫秒时间戳");
    }

    #[test]
    fn mtime_ms_zero_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("不存在.jsonl");
        assert_eq!(file_mtime_ms(&missing), 0);
    }

    // ── 尾部标题端到端（大文件场景经 scan 落地） ──

    #[test]
    fn scan_tail_custom_title_overrides_head_summary() {
        // 尾部 64KB 内 custom-title → 覆盖头部 summary（决策 22）
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        let path = proj.join(format!("{uuid}.jsonl"));
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"summary","summary":"旧摘要","leafUuid":"x"}}"#
        )
        .unwrap();
        // 填充 >512KB，使尾部窗口与头部窗口分离
        let line = b"{\"type\":\"pad\",\"payload\":\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"}\n";
        while f.metadata().unwrap().len() < 600 * 1024 {
            f.write_all(line).unwrap();
        }
        writeln!(
            f,
            r#"{{"type":"custom-title","customTitle":"重命名后的标题","sessionId":"{uuid}"}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        let s = &sessions[0];
        assert_eq!(s.title.as_deref(), Some("重命名后的标题"));
        assert_eq!(s.title_source, TitleSource::CustomTitle.as_str());
    }

    // ── BE-19 进程内缓存（键 = 目录内容指纹 CP-007；force=true 绕过） ──

    #[test]
    fn scan_cache_invalidated_when_session_file_deleted() {
        // 指纹口径（CP-007）：目录内会话文件删除 → 文件从指纹清单消失 → 指纹变
        // → 缓存失效 → 重扫立即反映删除。旧键 (mtime, file_count) 做不到（目录级
        // 失效，删除文件不影响根键——曾由前端恒 force=true 兜底，见 CP-007）。
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        write_valid_session(&proj, uuid);
        let _guard = ScanRootGuard::set(&root);

        let first = scan_sessions();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].session_id, uuid);

        // 删除会话文件（文件从指纹清单消失）→ 缓存失效重扫为空
        std::thread::sleep(std::time::Duration::from_millis(5)); // 跨毫秒保险（时序稳健）
        std::fs::remove_file(proj.join(format!("{uuid}.jsonl"))).unwrap();
        let second = scan_sessions();
        assert!(second.is_empty(), "文件删除应使指纹变化 → 缓存失效重扫为空");
    }

    #[test]
    fn scan_cache_invalidated_when_file_count_changes() {
        // 新增一级编码目录（条目数 +1 入指纹）→ 指纹变化 → 缓存失效 → 重扫全量
        let (_dir, root, proj) = make_scan_root();
        let uuid1 = "123e4567-e89b-12d3-a456-426614174001";
        write_valid_session(&proj, uuid1);
        let _guard = ScanRootGuard::set(&root);

        let first = scan_sessions();
        assert_eq!(first.len(), 1);

        // 新增第二个编码目录 + 会话
        let proj2 = root.join("D--other-app");
        std::fs::create_dir_all(&proj2).unwrap();
        let uuid2 = "123e4567-e89b-12d3-a456-426614174002";
        write_valid_session(&proj2, uuid2);

        let second = scan_sessions();
        let mut ids: Vec<&str> = second.iter().map(|s| s.session_id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, [uuid1, uuid2], "指纹变化应失效缓存并重扫全量");
    }

    #[test]
    fn scan_cache_invalidated_when_session_file_modified() {
        // 指纹口径新增用例（CP-007）：目录内会话文件内容修改（追加 custom-title）→
        // 文件自身 mtime/len 变化 → 指纹变 → 缓存失效 → 重扫取到新标题。旧键对目录内
        // 变更不敏感（曾由前端恒 force=true 兜底），此用例锁死失效精度提升。
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        write_valid_session(&proj, uuid);
        let _guard = ScanRootGuard::set(&root);

        let first = scan_sessions();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].title.as_deref(), Some("修复登录 bug"));

        // 追加 custom-title（尾部 last-wins 覆写摘要标题）——文件 mtime 需跨毫秒
        // 才保证键变化（首扫键采集时刻与追加时刻不得同毫秒）。夹具末行无收尾
        // 换行——先补换行再追加，防两行粘连成损坏行
        std::thread::sleep(std::time::Duration::from_millis(5));
        let path = proj.join(format!("{uuid}.jsonl"));
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        f.write_all(b"\n").unwrap();
        writeln!(
            f,
            r#"{{"type":"custom-title","customTitle":"重命名后的标题","sessionId":"{uuid}"}}"#
        )
        .unwrap();
        f.flush().unwrap();
        // 元数据提交可能滞后于读（杀软/高负载下实测可至数十 ms）——轮询 len 变化
        // 等落定（append 只增不减，len 必变；干净环境首次检查即过，最多等 500ms）
        let base_len = std::fs::metadata(&path).unwrap().len();
        for _ in 0..50 {
            if std::fs::metadata(&path).unwrap().len() != base_len {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        std::thread::sleep(std::time::Duration::from_millis(5)); // 跨毫秒保险

        let second = scan_sessions();
        assert_eq!(
            second[0].title.as_deref(),
            Some("重命名后的标题"),
            "会话文件修改应使指纹变化 → 缓存失效重扫取到新标题"
        );
        assert_eq!(second[0].title_source, TitleSource::CustomTitle.as_str());
    }

    #[test]
    fn scan_force_true_bypasses_cache() {
        // force=true 绕过缓存强制重扫：键未变也重读磁盘
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        write_valid_session(&proj, uuid);
        let _guard = ScanRootGuard::set(&root);

        let first = scan_sessions();
        assert_eq!(first.len(), 1);

        // 删除文件后 force 重扫 → 应直接读盘得空（不命中缓存旧结果）
        std::fs::remove_file(proj.join(format!("{uuid}.jsonl"))).unwrap();
        let forced = scan_sessions_with_force(true);
        assert!(forced.is_empty(), "force=true 应绕过缓存直接重扫磁盘");
    }

    #[test]
    fn scan_cache_key_tracks_dir_content_fingerprint() {
        // 契约键 = 目录内容指纹（CP-007）：root 一级目录名清单 + 会话文件条目的
        // (file_name, mtime_ms, len)，排序后 FNV-1a 64 + 条目数。失效精度核心用例——
        // 目录内会话文件增删改亦改变指纹，这是旧键 (mtime, file_count) 目录级失效
        // 做不到的（旧键只读 root 自身 mtime，目录内变更不敏感）。
        let (_dir, root, proj) = make_scan_root();
        let key1 = cache_key_of(&root).unwrap();

        // 目录内追加会话文件：文件清单 +1 条目 → 指纹变（文件级 mtime/len 入键）。
        // Windows NTFS 时间精度 100ns，as_millis() 截断到毫秒——sleep 保证键采集
        // 与文件写入跨毫秒，规避同毫秒截断偶合
        std::thread::sleep(std::time::Duration::from_millis(5));
        write_valid_session(&proj, "123e4567-e89b-12d3-a456-426614174001");
        let key2 = cache_key_of(&root).unwrap();
        assert_ne!(
            key1.fingerprint, key2.fingerprint,
            "目录内文件新增应改变指纹（文件条目入键）"
        );

        // 改名一级目录：file_name 入指纹 → 指纹变（条目数不变也失效）
        std::thread::sleep(std::time::Duration::from_millis(5));
        let proj2 = root.join("D--renamed");
        std::fs::rename(&proj, &proj2).unwrap();
        let key3 = cache_key_of(&root).unwrap();
        assert_ne!(key2.fingerprint, key3.fingerprint, "改名应改变指纹");

        // 新增一级目录：条目数 +1（混入指纹）→ 指纹变
        let key4 = cache_key_of(&root).unwrap();
        std::fs::create_dir_all(root.join("E--new-app")).unwrap();
        let key5 = cache_key_of(&root).unwrap();
        assert_ne!(key4.fingerprint, key5.fingerprint, "新增目录应改变指纹");

        // 指纹稳定：内容无变化时重复计算同值（命中前提）
        let key6 = cache_key_of(&root).unwrap();
        assert_eq!(key5.fingerprint, key6.fingerprint, "内容未变指纹应稳定");
    }

    #[test]
    fn scan_cache_isolated_per_root() {
        // 不同扫描根互不污染（缓存键含 root）：根 B 不命中根 A 的缓存，换根即重扫
        let (_dir_a, root_a, proj_a) = make_scan_root();
        let uuid_a = "123e4567-e89b-12d3-a456-42661417400a";
        write_valid_session(&proj_a, uuid_a);
        let (_dir_b, root_b, proj_b) = make_scan_root();
        let uuid_b = "123e4567-e89b-12d3-a456-42661417400b";
        write_valid_session(&proj_b, uuid_b);

        let g1 = ScanRootGuard::set(&root_a);
        let a1 = scan_sessions();
        assert_eq!(a1[0].session_id, uuid_a);
        drop(g1);

        let g2 = ScanRootGuard::set(&root_b);
        let b1 = scan_sessions();
        assert_eq!(
            b1[0].session_id, uuid_b,
            "根 B 应扫自己的根而非命中根 A 缓存"
        );
        drop(g2);

        // 切回根 A：单槽缓存已被根 B 顶替 → 重扫（结果仍正确，不串扰）
        let g3 = ScanRootGuard::set(&root_a);
        let a2 = scan_sessions();
        assert_eq!(a2[0].session_id, uuid_a);
        drop(g3);
    }
}

// ── CP-007 全扫性能基准（决策门槛写死处；两分支共有，常驻全量门禁） ──

#[cfg(test)]
mod scan_bench {
    use super::*;
    use crate::agent_history::claude::ScanRootGuard;
    use std::time::{Duration, Instant};

    /// CP-007: 1000 会话目录全扫性能门槛——写死,防「无缓存」回归慢化
    ///
    /// 构造:1000 个编码目录 × 每目录 1 个 UUID jsonl(head+tail 真实内容,
    /// 照 write_valid_session 夹具形态);样本 20 次取中位。
    /// 门槛演进(留痕,CP-007 决策档):9-8 首测 debug 全扫中位 180.33ms / max
    /// 186.32ms,越原决策门槛(中位 < 50ms)→ 红 → 走指纹分支(缓存保留,键改目录
    /// 内容指纹)。决策门槛使命完成后常驻门槛放宽:中位 < 200ms(依据 180.33ms)。
    /// flaky 处置(9-8 全量门禁再留痕):与 clippy 并行抢 CPU 时段样本中位
    /// 184.9ms 稳过、但 max 249.65ms 越旧 max < 200ms 尾部门槛 → 红——max 对
    /// 机器负载敏感(定向复测 max 184.9/185.6ms),故 max 不作硬断言,仅报告
    /// 留档;中位稳定(180-185ms)是唯一硬门槛——任何进一步放宽须留数字依据;
    /// 若直扫成本经优化回落 < 50ms,应重评估删除分支(CP-007)。
    /// 9-9 复核加固：L1/L2 并行抢核单轮中位 410ms 误红 → 断言改两轮制（冷却 2s
    /// 重采样复判）；持续并行负载仍不抗——L1/L2 串行回归纪律登记根 CLAUDE.md（BE-06）。
    #[test]
    fn scan_bench_1000_sessions_median_under_200ms() {
        // 夹具:1000 编码目录 × 1 会话文件(内容照 write_valid_session:summary 首行 +
        // user prompt 行——头部/尾部两窗口真实解析形态)。夹具构造耗时不计样本。
        let dir = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(dir.path()).unwrap();
        for i in 0..1000 {
            let proj = root.join(format!("D--bench-{i:04}"));
            std::fs::create_dir_all(&proj).unwrap();
            let uuid = format!("123e4567-e89b-12d3-a456-{i:012x}");
            write_bench_session(&proj, &uuid);
        }
        let _guard = ScanRootGuard::set(&root);

        // 预热一轮(OS/进程缓存落位),不计入样本
        let warm = scan_sessions_with_force(true);
        assert_eq!(warm.len(), 1000, "夹具应全量命中 1000 会话");

        // 样本 20 次:每次强制全量直扫(与前端恒 force 刷新同口径)
        let mut samples: Vec<Duration> = Vec::with_capacity(20);
        for _ in 0..20 {
            let t0 = Instant::now();
            let sessions = scan_sessions_with_force(true);
            samples.push(t0.elapsed());
            assert_eq!(sessions.len(), 1000, "每次样本应全量命中 1000 会话");
        }
        samples.sort();
        let median = samples[9]; // 20 样本取低中位(第 10 小)
        let max = samples[19];
        eprintln!("CP-007 基准(轮 1): 样本=20 中位={median:?} max={max:?}(每样本 = 1000 会话全扫)");
        let threshold = Duration::from_millis(200);
        if median >= threshold {
            // 负载加固（BE-06，2026-09-09）：并行抢 CPU 的瞬态负载曾致单轮中位 410ms 误红
            // （隔离复跑 180ms）——冷却 2s 重采样一轮再判，两轮均越门槛才红（真实回归两轮必越，
            // 瞬态尖峰第二轮自愈）。仍不抗持续并行负载——全量回归 L1/L2 串行纪律见根 CLAUDE.md。
            eprintln!("CP-007 基准(轮 1) 中位越门槛——冷却 2s 重采样复判");
            std::thread::sleep(Duration::from_secs(2));
            let mut samples2: Vec<Duration> = Vec::with_capacity(20);
            for _ in 0..20 {
                let t0 = Instant::now();
                let sessions = scan_sessions_with_force(true);
                samples2.push(t0.elapsed());
                assert_eq!(sessions.len(), 1000, "重采样每次样本应全量命中 1000 会话");
            }
            samples2.sort();
            let median2 = samples2[9];
            eprintln!("CP-007 基准(轮 2): 中位={median2:?} max={:?}", samples2[19]);
            assert!(
                median2 < threshold,
                "全扫中位两轮均越常驻门槛 200ms(轮1={median:?} 轮2={median2:?},依据 9-8 实测 180.33ms)——若回落 <50ms 重评估删除分支,放宽须留数字依据(CP-007)"
            );
        }
    }

    /// 写一个有效会话文件(UUID 文件名 + summary 首行 + user prompt 行,照 write_valid_session 形态)
    fn write_bench_session(proj: &std::path::Path, uuid: &str) {
        let content = serde_json::json!({
            "type": "summary",
            "summary": "基准会话",
            "leafUuid": "x",
        })
        .to_string()
            + "\n"
            + &serde_json::json!({
                "type": "user",
                "cwd": "C:\\bench\\app",
                "message": { "content": "帮我修 bug" },
            })
            .to_string();
        std::fs::write(proj.join(format!("{uuid}.jsonl")), content).unwrap();
    }
}
