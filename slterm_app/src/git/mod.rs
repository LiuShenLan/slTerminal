//! git 执行体 facade(D07-1 已裁「子进程统一」)。
//!
//! 旧栈 `src-tauri/src/git/` 是 git2(libgit2 vendored)封装;新世界不移植
//! git2,操作面统一走 git CLI 子进程(上游子进程形态)。本步(M8.1)只立只读面:
//! 仓库根解析 + HEAD 文件字节——`gitshow` 只读臂的数据源。status/diff/hunks/
//! rollback/unstage 四方法执行归 M8.5 联调;本模块保留统一执行器形状:
//! 单构造口 [`command`],每命令 `--no-optional-locks`(边界条 7,
//! 禁触发 index 锁与后台 gc),argv 不经 shell,Windows 挂 CREATE_NO_WINDOW。
//!
//! 旧 git2 语义资产(五份因果档案)转测试保留,按步骤迁移:**HEAD 不存在
//! 错误契约**——`UnbornBranch`(尚无提交)与「文件不在 HEAD tree」统一报
//! [`GitError::NotInHead`],消息含「HEAD 中不存在」;权限/损坏经
//! [`classify_git_failure`] 细分为 [`GitError::AccessDenied`]/
//! [`GitError::Corrupt`]。UI 占位文案归 i18n,不解析具体错误内容。
//! 只读消费面的短 TTL 缓存见 [`status_cached`]/[`repository_root_cached`];
//! 不扫 ignored(CP-008)/rename 双检/rollback 三方字节一致归 M8.5 对照用例。

use std::collections::HashMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// git 执行体错误。消息保持用户可读(经 i18n `{error}` 占位展示)。
#[derive(Debug)]
pub(crate) enum GitError {
    /// git 可执行文件缺失或无法启动。
    Unavailable(String),
    /// 路径不在任何 Git worktree 内。
    NotRepository(String),
    /// HEAD 不存在(尚无提交)或文件不在 HEAD tree 中;消息含「HEAD 中不存在」。
    NotInHead(String),
    /// 权限不足:文件系统拒绝读取仓库/对象(stderr 分类所得)。
    AccessDenied(String),
    /// 仓库/对象损坏:corrupt、bad/loose object、object file 等(stderr 分类所得)。
    Corrupt(String),
    /// 其他命令失败(M8.5 四方法执行复用此臂)。
    Command(String),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(message)
            | Self::NotRepository(message)
            | Self::NotInHead(message)
            | Self::AccessDenied(message)
            | Self::Corrupt(message)
            | Self::Command(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for GitError {}

/// 子进程统一构造口:全局 flag 必须位于子命令之前。
///
/// `pub(crate)`:`side_panel` 的 git 状态快照与文件树过滤索引共用本
/// 构造口(D07-1 子进程统一,无第二执行体)。
pub(crate) fn command(cwd: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("--no-optional-locks").arg("-C").arg(cwd);
    crate::platform::process::hidden_command(&mut command);
    command
}

/// 只读 git 快照的短 TTL:同一 root 的连续/并发读取在窗口内共享一次子进程。
///
/// 500ms 只影响 gutter/文件树的最多半个窗口陈旧度;写路径(rollback、runtime
/// revision 校验)一律绕过缓存,正确性不依赖本值。
const STATUS_CACHE_TTL: Duration = Duration::from_millis(500);

/// 仓库根解析缓存的 TTL(与状态快照同窗)。
const REPOSITORY_ROOT_CACHE_TTL: Duration = Duration::from_millis(500);

/// 仓库根解析缓存容量上限:超出时先清过期,仍超则淘汰最早捕获项。
const REPOSITORY_ROOT_CACHE_CAPACITY: usize = 16;

/// 状态快照缓存槽条目:单 root 槽,root 不匹配即视为未命中。
struct StatusCacheEntry {
    root: PathBuf,
    captured: Instant,
    snapshot: Arc<StatusSnapshot>,
}

/// 仓库根缓存条目(键为起始目录)。
struct RootCacheEntry {
    captured: Instant,
    root: PathBuf,
}

/// 路径所在 worktree 的根(`git rev-parse --show-toplevel`)。
///
/// `verify_repository(pane_cwd, repo_root)` 的 `repo_root` 数据源:
/// M8.5 的 `git.*` UI 执行器以本函数重查仓库根,与 `expected_cwd` 核对。
/// 写路径/runtime 校验用本函数(实时);只读消费面用
/// [`repository_root_cached`]。
pub(crate) fn repository_root(path: &Path) -> Result<PathBuf, GitError> {
    resolve_repository_root(&repository_root_anchor(path))
}

/// 在给定起始目录上执行 `rev-parse`(缓存读取器与实时路径共用)。
fn resolve_repository_root(cwd: &Path) -> Result<PathBuf, GitError> {
    let output = command(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || root.is_empty() {
        return Err(GitError::NotRepository(format!(
            "不在 Git 仓库内: {}",
            cwd.display()
        )));
    }
    Ok(PathBuf::from(root))
}

/// 仓库根解析的起始目录:目录路径原样,文件路径取父目录。
fn repository_root_anchor(path: &Path) -> PathBuf {
    if path.is_dir() {
        return path.to_path_buf();
    }
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

/// 仓库根解析的只读短 TTL 缓存。
///
/// 键 = 起始目录(见 [`repository_root_anchor`]);上限 16 条,过期与超限
/// 都清理。解析失败不写缓存,避免把一次瞬时失败固化成 TTL 内的错误答案。
pub(crate) fn repository_root_cached(path: &Path) -> Result<PathBuf, GitError> {
    repository_root_cached_with(path, Instant::now(), resolve_repository_root)
}

fn repository_root_cached_with<R>(path: &Path, now: Instant, reader: R) -> Result<PathBuf, GitError>
where
    R: FnOnce(&Path) -> Result<PathBuf, GitError>,
{
    let anchor = repository_root_anchor(path);
    let mut cache = repository_root_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(entry) = cache.get(&anchor)
        && now.saturating_duration_since(entry.captured) < REPOSITORY_ROOT_CACHE_TTL
    {
        return Ok(entry.root.clone());
    }
    let root = reader(&anchor)?;
    store_repository_root(
        &mut cache,
        anchor,
        RootCacheEntry {
            captured: now,
            root: root.clone(),
        },
    );
    Ok(root)
}

fn repository_root_cache() -> &'static Mutex<HashMap<PathBuf, RootCacheEntry>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, RootCacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 写入前清理过期条目;仍超上限时移除最早捕获的一条。
fn store_repository_root(
    cache: &mut HashMap<PathBuf, RootCacheEntry>,
    anchor: PathBuf,
    entry: RootCacheEntry,
) {
    cache.retain(|_, cached| {
        entry.captured.saturating_duration_since(cached.captured) < REPOSITORY_ROOT_CACHE_TTL
    });
    while cache.len() >= REPOSITORY_ROOT_CACHE_CAPACITY {
        let Some(oldest) = cache
            .iter()
            .min_by_key(|(_, cached)| cached.captured)
            .map(|(anchor, _)| anchor.clone())
        else {
            break;
        };
        cache.remove(&oldest);
    }
    cache.insert(anchor, entry);
}

/// 读取文件在工作树当前 HEAD 中的字节(上限 `limit`,溢出置 truncated)。
///
/// 只读数据面:不做写回,不碰 index。`limit + 1` 探到溢出后截断并终止
/// 子进程,避免把超大 blob 全量读进内存。对象存在性先用 `cat-file -e`
/// 预检:失败按 stderr 分类,权限/损坏不再误报「文件不在 HEAD」。
pub(crate) fn head_file(path: &Path, limit: usize) -> Result<(Vec<u8>, bool), GitError> {
    let root = repository_root(path)?;
    let relative = relative_to_root(&root, path)?;
    // HEAD 存在性是独立判定:unborn 仓与「文件不在 HEAD tree」在 UI 上
    // 同属占位文案,但前者连 show 的输入都没有。
    verify_head(&root)?;
    let spec = format!("HEAD:{relative}");
    // 预检只判对象存在性;识别不出原因时仍归「不在 HEAD」默认臂。
    let precheck = command(&root)
        .args(["cat-file", "-e", &spec])
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    if !precheck.status.success() {
        let stderr = String::from_utf8_lossy(&precheck.stderr);
        return Err(classify_git_failure(&stderr)
            .unwrap_or_else(|| GitError::NotInHead(format!("文件在 HEAD 中不存在: {relative}"))));
    }
    let mut child = command(&root)
        .args(["show", &spec])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| GitError::Command("无法打开 git show 管道".to_owned()))?;
    let mut bytes = Vec::new();
    stdout
        .by_ref()
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| GitError::Command(format!("读取 HEAD 字节失败: {error}")))?;
    let truncated = bytes.len() > limit;
    if truncated {
        bytes.truncate(limit);
    }
    // 截断时子进程仍可能往管道写:先杀再收尸;stdout 句柄在此前已 drop。
    drop(stdout);
    if truncated {
        let _ = child.kill();
    }
    let output = child
        .wait_with_output()
        .map_err(|error| GitError::Command(format!("等待 git show 失败: {error}")))?;
    if !truncated && !output.status.success() {
        // 预检通过后 show 仍失败(如读取中途对象损坏):同一分类臂收口。
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(classify_git_failure(&stderr)
            .unwrap_or_else(|| GitError::NotInHead(format!("文件在 HEAD 中不存在: {relative}"))));
    }
    Ok((bytes, truncated))
}

/// HEAD 可解析性(尚无提交 = `UnbornBranch` 的 CLI 等价物)。
fn verify_head(root: &Path) -> Result<(), GitError> {
    let output = command(root)
        .args(["rev-parse", "--verify", "--quiet", "HEAD"])
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(GitError::NotInHead(
            "HEAD 中不存在：尚无提交记录".to_owned(),
        ))
    }
}

/// 冲突三栏 stage 的单文件读取上限(展示与「采用」动作共用;超限即截断)。
pub(crate) const CONFLICT_STAGE_LIMIT: usize = 4 * 1024 * 1024;

/// 单个冲突文件的三种 index stage 内容(1=base、2=ours、3=theirs)。
///
/// 缺失 stage 为 `None`(add/add、modify/delete 等形态);文本按 UTF-8
/// lossy 解码仅供展示,`valid_utf8` 记录原始字节是否可无损写回,
/// `truncated` 标记该栏是否因超出上限被截断(截断内容禁止用于写盘)。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ConflictStages {
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
    /// 各栏原始字节是否合法 UTF-8(顺序 base/ours/theirs)。
    pub valid_utf8: [bool; 3],
    /// 各栏是否被截断(顺序 base/ours/theirs)。
    pub truncated: [bool; 3],
}

/// 读取冲突文件的 `:1:/:2:/:3:` 三 stage。
///
/// 走既有 [`command`] 执行体(`--no-optional-locks`),每 stage 先
/// `cat-file -e` 判存在性:不存在 = 该 stage 为 `None`,不再起 `show`。
/// 缺失/非冲突文件不报错;权限/损坏按 [`classify_git_failure`] 收口。
pub(crate) fn conflict_stages(root: &Path, relative: &str) -> Result<ConflictStages, GitError> {
    conflict_stages_with_limit(root, relative, CONFLICT_STAGE_LIMIT)
}

/// 上限可注入的 stage 读取(测试截断语义用)。
fn conflict_stages_with_limit(
    root: &Path,
    relative: &str,
    limit: usize,
) -> Result<ConflictStages, GitError> {
    // 固定顺序 = base/ours/theirs,与 `ConflictStages` 的数组下标同源。
    let mut stages = ConflictStages::default();
    for (slot, stage) in [(0usize, 1u8), (1, 2), (2, 3)] {
        let spec = format!(":{stage}:{relative}");
        let Some((bytes, truncated)) = show_stage_bounded(root, &spec, limit)? else {
            continue;
        };
        stages.valid_utf8[slot] = std::str::from_utf8(&bytes).is_ok();
        stages.truncated[slot] = truncated;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        match slot {
            0 => stages.base = Some(text),
            1 => stages.ours = Some(text),
            _ => stages.theirs = Some(text),
        }
    }
    Ok(stages)
}

/// 单个 stage 的有界读取;stage 不存在返回 `Ok(None)`。
fn show_stage_bounded(
    root: &Path,
    spec: &str,
    limit: usize,
) -> Result<Option<(Vec<u8>, bool)>, GitError> {
    let precheck = command(root)
        .args(["cat-file", "-e", spec])
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    if !precheck.status.success() {
        let stderr = String::from_utf8_lossy(&precheck.stderr);
        // 权限/损坏必须上抛;「stage 不存在」/「不在 index」是正常语义。
        if let Some(error) = classify_git_failure(&stderr) {
            return Err(error);
        }
        if stderr.contains("not a git repository") {
            return Err(command_failure(root, "cat-file", &precheck.stderr));
        }
        return Ok(None);
    }
    let mut child = command(root)
        .args(["show", spec])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| GitError::Command("无法打开 git show 管道".to_owned()))?;
    let mut bytes = Vec::new();
    stdout
        .by_ref()
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| GitError::Command(format!("读取冲突 stage 失败: {error}")))?;
    let truncated = bytes.len() > limit;
    if truncated {
        bytes.truncate(limit);
    }
    // 截断时子进程仍可能继续写管道:先杀再收尸(stdout 句柄已 drop)。
    drop(stdout);
    if truncated {
        let _ = child.kill();
    }
    let output = child
        .wait_with_output()
        .map_err(|error| GitError::Command(format!("等待 git show 失败: {error}")))?;
    if !truncated && !output.status.success() {
        return Err(command_failure(root, "show", &output.stderr));
    }
    Ok(Some((bytes, truncated)))
}

/// 一条统一 diff hunk(git `@@ -a,b +c,d @@` 解析)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
}

/// 单文件状态(porcelain v2 XY 归一到 gutter/diff 六值)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileStatus {
    Untracked,
    Added,
    Modified,
    Deleted,
    Renamed,
    Conflicted,
}

/// `git status --porcelain=v2 --branch -z` 解析结果(展示面与写操作 revision
/// 共用;D07-1 子进程统一后本解析是唯一实现)。
#[derive(Clone, Debug, Default)]
pub(crate) struct StatusSnapshot {
    /// `sha256(porcelain stdout)` 的 64 位 hex——写操作防陈旧 revision。
    pub digest: String,
    pub branch: String,
    pub head: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub entries: Vec<StatusEntry>,
}

#[derive(Clone, Debug)]
pub(crate) struct StatusEntry {
    /// Git 原始路径身份(`/` 分隔,相对仓库根)。
    pub path: String,
    /// rename 的源路径(porcelain v2 `2` 记录后随 NUL 记录)。
    pub original: Option<String>,
    pub index: char,
    pub worktree: char,
    pub conflict: bool,
}

impl StatusEntry {
    pub(crate) fn staged(&self) -> bool {
        !matches!(self.index, '.' | '?')
    }

    pub(crate) fn unstaged(&self) -> bool {
        self.worktree != '.'
    }
}

/// 采集仓库状态快照;每条命令都带 `--no-optional-locks`。
///
/// `--untracked-files=all`:未跟踪目录展开为单文件条目,单文件状态可从
/// 共享快照直接派生(不再为路径单独 spawn 一次 status)。写路径与 revision
/// 校验使用本函数(实时)。
pub(crate) fn status(root: &Path) -> Result<StatusSnapshot, GitError> {
    let output = command(root)
        .arg("--literal-pathspecs")
        .args([
            "status",
            "--porcelain=v2",
            "--branch",
            "-z",
            "--untracked-files=all",
        ])
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    if !output.status.success() {
        return Err(command_failure(root, "status", &output.stderr));
    }
    let raw = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut snapshot =
        parse_status(&raw).map_err(|code| GitError::Command(format!("status 解析失败: {code}")))?;
    snapshot.digest = sha256_hex(&output.stdout);
    Ok(snapshot)
}

/// 只读消费面的状态快照:进程级单槽短 TTL 缓存。
///
/// 同 root 的连续/并发读取共享一次 `git status` 子进程;调用期间持锁,后到
/// 者等待后直接命中新快照。缓存只准 gutter/文件树等只读面使用;rollback 与
/// runtime revision 校验必须走 [`status`]/[`file_status_uncached`]。
pub(crate) fn status_cached(root: &Path) -> Result<Arc<StatusSnapshot>, GitError> {
    status_cached_with(root, Instant::now(), status)
}

fn status_cached_with<R>(
    root: &Path,
    now: Instant,
    reader: R,
) -> Result<Arc<StatusSnapshot>, GitError>
where
    R: FnOnce(&Path) -> Result<StatusSnapshot, GitError>,
{
    static CACHE: Mutex<Option<StatusCacheEntry>> = Mutex::new(None);
    let mut cache = CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(entry) = cache.as_ref()
        && entry.root == root
        && now.saturating_duration_since(entry.captured) < STATUS_CACHE_TTL
    {
        return Ok(Arc::clone(&entry.snapshot));
    }
    let snapshot = Arc::new(reader(root)?);
    *cache = Some(StatusCacheEntry {
        root: root.to_path_buf(),
        captured: now,
        snapshot: Arc::clone(&snapshot),
    });
    Ok(snapshot)
}

/// porcelain v2 `--branch -z` 解析(纯函数;截断/坏记录整体拒绝)。
pub(crate) fn parse_status(output: &str) -> Result<StatusSnapshot, &'static str> {
    if !output.ends_with('\0') {
        return Err("git_invalid_status");
    }
    let mut result = StatusSnapshot::default();
    let mut records = output.split_terminator('\0');
    while let Some(record) = records.next() {
        if let Some(header) = record.strip_prefix("# ") {
            if let Some(value) = header.strip_prefix("branch.head ") {
                result.branch = value.into();
            } else if let Some(value) = header.strip_prefix("branch.oid ") {
                result.head = value.into();
            } else if let Some(value) = header.strip_prefix("branch.upstream ") {
                result.upstream = Some(value.into());
            } else if let Some(value) = header.strip_prefix("branch.ab ") {
                let (ahead, behind) = value.split_once(' ').ok_or("git_invalid_status")?;
                result.ahead = ahead
                    .strip_prefix('+')
                    .ok_or("git_invalid_status")?
                    .parse()
                    .map_err(|_| "git_invalid_status")?;
                result.behind = behind
                    .strip_prefix('-')
                    .ok_or("git_invalid_status")?
                    .parse()
                    .map_err(|_| "git_invalid_status")?;
            }
            continue;
        }
        if record.starts_with("! ") {
            continue;
        }
        let kind = record
            .as_bytes()
            .first()
            .copied()
            .ok_or("git_invalid_status")?;
        let entry = if kind == b'?' {
            StatusEntry {
                path: record
                    .strip_prefix("? ")
                    .ok_or("git_invalid_status")?
                    .into(),
                original: None,
                index: '?',
                worktree: '?',
                conflict: false,
            }
        } else {
            let count = match kind {
                b'1' => 9,
                b'2' => 10,
                b'u' => 11,
                _ => return Err("git_invalid_status"),
            };
            let fields: Vec<_> = record.splitn(count, ' ').collect();
            if fields.len() != count || fields[1].len() != 2 || !fields[1].is_ascii() {
                return Err("git_invalid_status");
            }
            let xy = fields[1].as_bytes();
            StatusEntry {
                path: fields[count - 1].into(),
                original: if kind == b'2' {
                    Some(records.next().ok_or("git_invalid_status")?.into())
                } else {
                    None
                },
                index: xy[0] as char,
                worktree: xy[1] as char,
                conflict: kind == b'u',
            }
        };
        if entry.path.is_empty() || entry.original.as_ref().is_some_and(|path| path.is_empty()) {
            return Err("git_invalid_status");
        }
        result.entries.push(entry);
    }
    if result.branch.is_empty() || result.head.is_empty() {
        return Err("git_invalid_status");
    }
    Ok(result)
}

/// 单文件状态(只读消费面):从共享短 TTL 快照派生,不再单独 spawn
/// `git status -- path`。无记录 = `Ok(None)`。
pub(crate) fn file_status(root: &Path, relative: &str) -> Result<Option<FileStatus>, GitError> {
    let snapshot = status_cached(root)?;
    Ok(file_status_from_snapshot(&snapshot, relative))
}

/// 写路径专用的实时单文件状态:绕过短 TTL 缓存(rollback 判定)。
fn file_status_uncached(root: &Path, relative: &str) -> Result<Option<FileStatus>, GitError> {
    let snapshot = status(root)?;
    Ok(file_status_from_snapshot(&snapshot, relative))
}

/// 纯函数:从状态快照条目中查相对路径并归一为六值(无记录 = `None`)。
pub(crate) fn file_status_from_snapshot(
    snapshot: &StatusSnapshot,
    relative: &str,
) -> Option<FileStatus> {
    snapshot
        .entries
        .iter()
        .find(|entry| entry.path == relative)
        .map(classify_entry)
}

/// 纯映射:porcelain 记录 → 六值状态(与 [`parse_status`] 同源语义)。
///
/// 生产单文件状态已改从快照派生(见 [`file_status_from_snapshot`]);本函数
/// 保留为解析语义对照测试的唯一入口。
#[cfg(test)]
pub(crate) fn file_status_from_porcelain(output: &str, relative: &str) -> Option<FileStatus> {
    if output.trim_matches('\0').is_empty() {
        return None;
    }
    let mut records = output.split_terminator('\0');
    while let Some(record) = records.next() {
        if record.starts_with("# ") || record.starts_with("! ") || record.is_empty() {
            continue;
        }
        let kind = record.as_bytes()[0];
        let (path, xy, conflict) = if kind == b'?' {
            (record.strip_prefix("? ")?, ['?', '?'], false)
        } else {
            let count = match kind {
                b'1' => 9,
                b'2' => 10,
                b'u' => 11,
                _ => return None,
            };
            let fields: Vec<_> = record.splitn(count, ' ').collect();
            if fields.len() != count {
                return None;
            }
            if kind == b'2' {
                // rename 记录后随源路径 NUL 记录,消费掉保持记录对齐。
                let _original = records.next()?;
            }
            (fields[count - 1], parse_xy(fields[1])?, kind == b'u')
        };
        if path != relative {
            continue;
        }
        return Some(classify_status(kind, xy, conflict));
    }
    None
}

/// `XY` 两字符状态位(非两字符即坏记录;仅供测试专用解析入口)。
#[cfg(test)]
fn parse_xy(field: &str) -> Option<[char; 2]> {
    let mut chars = field.chars();
    let xy = [chars.next()?, chars.next()?];
    if chars.next().is_some() {
        return None;
    }
    Some(xy)
}

/// 快照条目 → 六值状态:按记录种类与 XY 复用 [`classify_status`] 语义。
fn classify_entry(entry: &StatusEntry) -> FileStatus {
    let kind = if entry.conflict {
        b'u'
    } else if entry.original.is_some() {
        b'2'
    } else if entry.index == '?' && entry.worktree == '?' {
        b'?'
    } else {
        b'1'
    };
    classify_status(kind, [entry.index, entry.worktree], entry.conflict)
}

/// XY/记录类型 → 六值状态。重命名与冲突优先于普通修改。
fn classify_status(kind: u8, xy: [char; 2], conflict: bool) -> FileStatus {
    let contains = |needle: char| xy[0] == needle || xy[1] == needle;
    if conflict || kind == b'u' || xy == ['A', 'A'] || xy == ['D', 'D'] {
        return FileStatus::Conflicted;
    }
    if kind == b'2' || contains('R') {
        return FileStatus::Renamed;
    }
    if kind == b'?' || xy == ['?', '?'] {
        return FileStatus::Untracked;
    }
    if contains('A') {
        return FileStatus::Added;
    }
    if contains('D') {
        return FileStatus::Deleted;
    }
    FileStatus::Modified
}

/// 文件相对 HEAD 的 hunk 集(`--unified=0`;staged + unstaged 合并)。
///
/// HEAD 不存在(尚无提交)时返回空集:新文件的行级标记由文件级状态承接
/// (gutter 值集不变),不伪造 hunk。
pub(crate) fn file_hunks(root: &Path, relative: &str) -> Result<Vec<FileHunk>, GitError> {
    if verify_head(root).is_err() {
        return Ok(Vec::new());
    }
    let output = command(root)
        .arg("--literal-pathspecs")
        .args([
            "diff",
            "--unified=0",
            "--no-color",
            "--no-ext-diff",
            "HEAD",
            "--",
            relative,
        ])
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    if !output.status.success() {
        return Err(command_failure(root, "diff", &output.stderr));
    }
    Ok(parse_hunks(&String::from_utf8_lossy(&output.stdout)))
}

/// 文件相对 HEAD 的统一 diff 文本(协议面 `git.diff` 的展示载荷)。
pub(crate) fn diff_text(root: &Path, relative: &str) -> Result<String, GitError> {
    if verify_head(root).is_err() {
        return Ok(String::new());
    }
    let output = command(root)
        .arg("--literal-pathspecs")
        .args([
            "diff",
            "--unified=3",
            "--no-color",
            "--no-ext-diff",
            "HEAD",
            "--",
            relative,
        ])
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    if !output.status.success() {
        return Err(command_failure(root, "diff", &output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// 解析 `@@ -old_start,old_lines +new_start,new_lines @@` 行(缺省行数 = 1)。
pub(crate) fn parse_hunks(diff: &str) -> Vec<FileHunk> {
    let mut hunks = Vec::new();
    for line in diff.lines() {
        let Some(rest) = line.strip_prefix("@@ ") else {
            continue;
        };
        let Some(end) = rest.find(" @@") else {
            continue;
        };
        let mut parts = rest[..end].split(' ');
        let (Some(old), Some(new)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Some(old) = old.strip_prefix('-').and_then(parse_hunk_range) else {
            continue;
        };
        let Some(new) = new.strip_prefix('+').and_then(parse_hunk_range) else {
            continue;
        };
        hunks.push(FileHunk {
            old_start: old.0,
            old_lines: old.1,
            new_start: new.0,
            new_lines: new.1,
        });
    }
    hunks
}

fn parse_hunk_range(value: &str) -> Option<(u32, u32)> {
    let (start, lines) = match value.split_once(',') {
        Some((start, lines)) => (start, lines),
        None => (value, "1"),
    };
    Some((start.parse().ok()?, lines.parse().ok()?))
}

/// 回滚单文件到 HEAD 字节(索引与工作区同时复位 = 三方字节一致)。
///
/// 未跟踪文件 = 删除工作区新文件;目录回滚拒绝(运行时路径是文件粒度)。
pub(crate) fn rollback(root: &Path, relative: &str) -> Result<(), GitError> {
    let target = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    if target.is_dir() {
        return Err(GitError::Command("目录不支持回滚".to_owned()));
    }
    // 写路径不得吃缓存:回滚前必须用实时状态判定文件当前所属分类。
    match file_status_uncached(root, relative)? {
        Some(FileStatus::Untracked) => std::fs::remove_file(&target).map_err(|error| {
            GitError::Command(format!("删除未跟踪文件失败 {}: {error}", target.display()))
        }),
        None => Ok(()),
        Some(_) => {
            verify_head(root)?;
            let output = command(root)
                .arg("--literal-pathspecs")
                .args(["checkout", "HEAD", "--", relative])
                .output()
                .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
            if output.status.success() {
                Ok(())
            } else {
                Err(command_failure(root, "checkout", &output.stderr))
            }
        }
    }
}

/// 取消暂存(索引 ← HEAD;工作区字节不动);unborn HEAD 回退 `git rm --cached`。
pub(crate) fn unstage(root: &Path, relative: &str) -> Result<(), GitError> {
    let args: &[&str] = if verify_head(root).is_ok() {
        &["restore", "--staged", "--", relative]
    } else {
        &["rm", "--cached", "--ignore-unmatch", "--", relative]
    };
    let output = command(root)
        .arg("--literal-pathspecs")
        .args(args)
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(command_failure(root, args[0], &output.stderr))
    }
}

/// 路径 → 仓库根相对路径(gutter/diff 的 file-scoped 命令参数)。
pub(crate) fn relative_path(root: &Path, path: &Path) -> Result<String, GitError> {
    relative_to_root(root, path)
}

/// 纯函数:git stderr → 细分错误类别;识别不出返回 `None`。
///
/// 大小写不敏感。权限标记优先于损坏标记(同一 stderr 两者都出现时,权限
/// 是根因);`None` 由调用方走默认臂——`head_file` 归 `NotInHead`。
pub(crate) fn classify_git_failure(stderr: &str) -> Option<GitError> {
    let lowered = stderr.to_ascii_lowercase();
    let reason = stderr.lines().next().unwrap_or("").trim();
    let permission_markers = [
        "permission denied",
        "access is denied",
        "operation not permitted",
        "read-only file system",
    ];
    if permission_markers
        .iter()
        .any(|marker| lowered.contains(marker))
    {
        return Some(GitError::AccessDenied(format!("git 访问被拒绝: {reason}")));
    }
    let corrupt_markers = [
        "corrupt",
        "bad object",
        "loose object",
        "object file",
        "unable to read",
        "invalid object",
        "missing object",
        "broken object",
    ];
    if corrupt_markers
        .iter()
        .any(|marker| lowered.contains(marker))
    {
        return Some(GitError::Corrupt(format!("git 对象/仓库损坏: {reason}")));
    }
    None
}

/// 命令失败的统一错误:stderr 首行进消息,便于 i18n `{error}` 展示。
fn command_failure(root: &Path, operation: &str, stderr: &[u8]) -> GitError {
    let text = String::from_utf8_lossy(stderr);
    let reason = text.lines().next().unwrap_or("unknown error");
    if text.contains("not a git repository") {
        GitError::NotRepository(format!("不在 Git 仓库内: {}", root.display()))
    } else {
        GitError::Command(format!("git {operation} 失败: {reason}"))
    }
}

/// `sha256` 64 位 hex(写操作 revision 摘要)。
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let mut text = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        use std::fmt::Write as _;
        let _ = write!(text, "{byte:02x}");
    }
    text
}

/// 绝对路径 → 仓库根相对路径(`/` 分隔,git tree 路径形态)。
fn relative_to_root(root: &Path, path: &Path) -> Result<String, GitError> {
    let relative = match path.strip_prefix(root) {
        Ok(relative) => relative.to_path_buf(),
        Err(_) => {
            // 8.3 短名/长名或大小写差异:规范化后再比一次(旧 git2 资产同因)。
            let root = std::fs::canonicalize(root).map_err(|error| {
                GitError::Command(format!("无法规范化仓库根 {}: {error}", root.display()))
            })?;
            let path = std::fs::canonicalize(path)
                .map_err(|error| GitError::Command(format!("无法规范化路径: {error}")))?;
            path.strip_prefix(&root)
                .map_err(|_| GitError::NotInHead(format!("文件不在仓库根内: {}", path.display())))?
                .to_path_buf()
        }
    };
    if relative.as_os_str().is_empty() {
        return Err(GitError::NotInHead(
            "文件在 HEAD 中不存在: 路径为空".to_owned(),
        ));
    }
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod git_tests {
    use super::*;
    use std::cell::Cell;
    use std::path::Path;

    /// 命令形态守卫:每条 git 都带 `--no-optional-locks`(边界条 7 不可谈判)。
    #[test]
    fn every_command_disables_optional_locks() {
        let command = command(Path::new("D:/repo"));
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            ["--no-optional-locks", "-C", "D:/repo"],
            "全局 flag 必须先于子命令"
        );
    }

    /// 仓库根解析:子目录路径上溯到 worktree 根。
    #[test]
    fn repository_root_resolves_the_worktree_top() {
        let repository = test_repository();
        let nested = repository.path().join("src");
        std::fs::create_dir_all(&nested).unwrap();
        let root = repository_root(&nested).expect("子目录必须解析出仓库根");
        assert_eq!(
            canonical(&root),
            canonical(repository.path()),
            "必须上溯到 worktree 根"
        );
    }

    /// 缓存命中/过期(注入时钟):TTL 内 reader 只跑一次,过期后重跑。
    #[test]
    fn status_cache_shares_reads_within_ttl_and_refreshes_after() {
        let root = Path::new("D:/slterm-test/status-cache/root");
        let start = Instant::now();
        let calls = Cell::new(0u32);
        let read = |_: &Path| {
            calls.set(calls.get() + 1);
            Ok(StatusSnapshot {
                digest: calls.get().to_string(),
                branch: "main".into(),
                head: "abc".into(),
                ..StatusSnapshot::default()
            })
        };

        let first = status_cached_with(root, start, read).expect("首次读取");
        let second =
            status_cached_with(root, start + Duration::from_millis(400), read).expect("TTL 内");
        assert_eq!(calls.get(), 1, "TTL 内必须共享一次子进程");
        assert!(Arc::ptr_eq(&first, &second), "命中必须复用同一 Arc");

        let third =
            status_cached_with(root, start + Duration::from_millis(600), read).expect("过期");
        assert_eq!(calls.get(), 2, "过期后必须重新读取");
        assert!(!Arc::ptr_eq(&first, &third));
        assert_eq!(third.digest, "2");
    }

    /// 仓库根缓存(注入时钟):同一起始目录 TTL 内只解析一次。
    #[test]
    fn repository_root_cache_shares_reads_within_ttl_and_refreshes_after() {
        let file = Path::new("D:/slterm-test/root-cache/nested/file.txt");
        let start = Instant::now();
        let calls = Cell::new(0u32);
        let read = |anchor: &Path| {
            calls.set(calls.get() + 1);
            Ok(anchor.join("..").join("root"))
        };

        let first = repository_root_cached_with(file, start, read).expect("首次解析");
        let second = repository_root_cached_with(file, start + Duration::from_millis(400), read)
            .expect("TTL 内");
        assert_eq!(calls.get(), 1, "TTL 内必须共享一次 rev-parse");
        assert_eq!(first, second);

        let third = repository_root_cached_with(file, start + Duration::from_millis(600), read)
            .expect("过期");
        assert_eq!(calls.get(), 2, "过期后必须重新解析");
        assert_eq!(third, first);
    }

    /// 解析失败不写缓存:下一次读取必须能重新尝试并成功。
    #[test]
    fn repository_root_failure_is_not_cached() {
        let file = Path::new("D:/slterm-test/root-failure/file.txt");
        let start = Instant::now();
        let calls = Cell::new(0u32);
        let failing = |_: &Path| {
            calls.set(calls.get() + 1);
            Err(GitError::NotRepository("不在仓库内".to_owned()))
        };
        assert!(
            repository_root_cached_with(file, start, failing).is_err(),
            "首次解析失败"
        );
        let succeeding = |_: &Path| {
            calls.set(calls.get() + 1);
            Ok(PathBuf::from("D:/slterm-test/resolved-root"))
        };
        let root = repository_root_cached_with(file, start, succeeding).expect("失败不得固化");
        assert_eq!(root, PathBuf::from("D:/slterm-test/resolved-root"));
        assert_eq!(calls.get(), 2, "失败后必须重新解析");
    }

    /// 超限清理:先清过期条目,仍满则淘汰最早捕获项。
    #[test]
    fn repository_root_cache_prunes_expired_and_caps_entries() {
        let start = Instant::now();
        let mut expired = HashMap::new();
        expired.insert(
            PathBuf::from("D:/slterm-test/root-cap/stale"),
            RootCacheEntry {
                captured: start,
                root: PathBuf::from("D:/root"),
            },
        );
        store_repository_root(
            &mut expired,
            PathBuf::from("D:/slterm-test/root-cap/fresh"),
            RootCacheEntry {
                captured: start + REPOSITORY_ROOT_CACHE_TTL,
                root: PathBuf::from("D:/root"),
            },
        );
        assert_eq!(expired.len(), 1, "过期条目必须清理");
        assert!(expired.contains_key(Path::new("D:/slterm-test/root-cap/fresh")));

        let mut full = HashMap::new();
        for index in 0..REPOSITORY_ROOT_CACHE_CAPACITY {
            full.insert(
                PathBuf::from(format!("D:/slterm-test/root-cap/{index:02}")),
                RootCacheEntry {
                    captured: start + Duration::from_millis(index as u64),
                    root: PathBuf::from("D:/root"),
                },
            );
        }
        store_repository_root(
            &mut full,
            PathBuf::from("D:/slterm-test/root-cap/new"),
            RootCacheEntry {
                captured: start + Duration::from_millis(REPOSITORY_ROOT_CACHE_CAPACITY as u64),
                root: PathBuf::from("D:/root"),
            },
        );
        assert_eq!(full.len(), REPOSITORY_ROOT_CACHE_CAPACITY, "容量上限");
        assert!(
            !full.contains_key(Path::new("D:/slterm-test/root-cap/00")),
            "最早捕获项必须被淘汰"
        );
        assert!(full.contains_key(Path::new("D:/slterm-test/root-cap/new")));
    }

    /// 快照派生:已跟踪修改/未跟踪/无记录/rename 按既有分类语义。
    #[test]
    fn file_status_snapshot_lookup_covers_tracked_missing_untracked_and_rename() {
        let snapshot = parse_status(
            "# branch.oid abc\0# branch.head main\0\
             1 .M N... 0 0 0 0 0 tracked.txt\0? new.txt\0\
             2 R. N... 0 0 0 0 0 R100 renamed.txt\0old.txt\0",
        )
        .expect("合法快照");
        assert_eq!(
            file_status_from_snapshot(&snapshot, "tracked.txt"),
            Some(FileStatus::Modified)
        );
        assert_eq!(
            file_status_from_snapshot(&snapshot, "new.txt"),
            Some(FileStatus::Untracked)
        );
        assert_eq!(
            file_status_from_snapshot(&snapshot, "renamed.txt"),
            Some(FileStatus::Renamed)
        );
        assert_eq!(file_status_from_snapshot(&snapshot, "clean.txt"), None);
    }

    /// 共享 status 必须展开未跟踪目录:快照含目录内单文件条目。
    #[test]
    fn status_snapshot_expands_untracked_directories_to_files() {
        let repository = test_repository();
        let root = repository.path();
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("nested/one.txt"), b"one\n").unwrap();
        std::fs::write(root.join("nested/two.txt"), b"two\n").unwrap();

        let snapshot = status(root).unwrap();
        assert!(
            snapshot
                .entries
                .iter()
                .any(|entry| entry.path == "nested/one.txt" && entry.index == '?'),
            "目录内文件必须单列: {:?}",
            snapshot.entries
        );
        assert!(
            !snapshot.entries.iter().any(|entry| entry.path == "nested/"),
            "不得折叠为目录条目"
        );
        assert_eq!(
            file_status_from_snapshot(&snapshot, "nested/two.txt"),
            Some(FileStatus::Untracked)
        );
    }

    /// 写路径绕过缓存:缓存锁定的旧状态不得左右 rollback 的实时判定。
    #[test]
    fn rollback_reads_realtime_status_and_bypasses_the_snapshot_cache() {
        let repository = test_repository();
        let root = repository.path();

        // 方向一:缓存锁定「无记录」,实时是未跟踪 → rollback 必须删除文件。
        let late = root.join("late-untracked.txt");
        let polluted = status_cached_with(root, Instant::now() + Duration::from_secs(3600), status)
            .expect("锁定无记录快照");
        assert_eq!(
            file_status_from_snapshot(&polluted, "late-untracked.txt"),
            None
        );
        std::fs::write(&late, "fresh\n").unwrap();
        rollback(root, "late-untracked.txt").expect("未跟踪回滚");
        assert!(!late.exists(), "实时未跟踪状态必须驱动删除");

        // 方向二:缓存锁定「未跟踪」,实时已提交 → rollback 不得删文件。
        let settled = root.join("settled.txt");
        std::fs::write(&settled, "settled\n").unwrap();
        let polluted = status_cached_with(root, Instant::now() + Duration::from_secs(7200), status)
            .expect("锁定未跟踪快照");
        assert_eq!(
            file_status_from_snapshot(&polluted, "settled.txt"),
            Some(FileStatus::Untracked)
        );
        assert!(git(root, ["add", "settled.txt"]).status.success());
        let commit = git(
            root,
            [
                "-c",
                "user.name=slTerminal Test",
                "-c",
                "user.email=slterm@example.invalid",
                "commit",
                "-m",
                "settle",
            ],
        );
        assert!(
            commit.status.success(),
            "commit: {}",
            String::from_utf8_lossy(&commit.stderr)
        );
        rollback(root, "settled.txt").expect("实时已跟踪且干净 = 空操作");
        assert_eq!(std::fs::read(&settled).unwrap(), b"settled\n");
    }

    /// HEAD 字节 ≠ 工作区字节:gitshow 数据面读的是已提交内容。
    #[test]
    fn head_file_returns_the_committed_blob_not_the_worktree_copy() {
        let repository = test_repository();
        let path = repository.path().join("tracked.txt");
        std::fs::write(&path, "worktree edit\n").unwrap();
        let (bytes, truncated) = head_file(&path, 1024 * 1024).expect("已提交文件必须可读");
        assert_eq!(bytes, b"tracked\n");
        assert!(!truncated);
    }

    /// 上限截断:只读前 limit 字节并置 truncated。
    #[test]
    fn head_file_bounds_reads_and_reports_truncation() {
        let repository = test_repository();
        let path = repository.path().join("tracked.txt");
        let (bytes, truncated) = head_file(&path, 4).expect("HEAD 读取");
        assert_eq!(bytes, b"trac");
        assert!(truncated);
    }

    /// UnbornBranch:尚无提交 → 错误消息含「HEAD 中不存在」。
    #[test]
    fn unborn_repository_reports_head_missing() {
        let directory = tempfile::tempdir().unwrap();
        assert!(
            git(directory.path(), ["init", "--initial-branch=main"])
                .status
                .success()
        );
        std::fs::write(directory.path().join("new.txt"), "new\n").unwrap();
        let error = head_file(&directory.path().join("new.txt"), 1024).expect_err("尚无提交");
        assert!(matches!(error, GitError::NotInHead(_)));
        assert!(
            error.to_string().contains("HEAD 中不存在"),
            "UI 占位契约: {}",
            error
        );
    }

    /// 文件在 HEAD tree 中不存在 → 同一错误契约(消息含相对路径)。
    #[test]
    fn files_absent_from_head_report_the_same_contract() {
        let repository = test_repository();
        let path = repository.path().join("untracked.txt");
        std::fs::write(&path, "untracked\n").unwrap();
        let error = head_file(&path, 1024).expect_err("未跟踪文件不在 HEAD");
        assert!(matches!(error, GitError::NotInHead(_)));
        let message = error.to_string();
        assert!(message.contains("HEAD 中不存在"), "{message}");
        assert!(message.contains("untracked.txt"), "{message}");
    }

    /// 真冲突仓(modify/modify):三 stage 分别读到 base/ours/theirs 字节。
    #[test]
    fn conflict_stages_read_base_ours_and_theirs_from_a_merge_conflict() {
        let repository = conflict_repository();
        let stages = conflict_stages(repository.path(), "conflicted.txt").expect("stage 读取");
        assert_eq!(stages.base.as_deref(), Some("line1\nbase\nline3\n"));
        assert_eq!(stages.ours.as_deref(), Some("line1\nours\nline3\n"));
        assert_eq!(stages.theirs.as_deref(), Some("line1\ntheirs\nline3\n"));
        assert_eq!(stages.valid_utf8, [true; 3]);
        assert_eq!(stages.truncated, [false; 3]);
    }

    /// 非冲突文件没有 stage 1/2/3:三栏全 None,不报错。
    #[test]
    fn conflict_stages_are_all_none_for_a_non_conflicted_file() {
        let repository = test_repository();
        let stages = conflict_stages(repository.path(), "tracked.txt").expect("非冲突文件");
        assert!(stages.base.is_none(), "{stages:?}");
        assert!(stages.ours.is_none(), "{stages:?}");
        assert!(stages.theirs.is_none(), "{stages:?}");
    }

    /// add/add 冲突没有 base stage:2/3 存在,1 缺失。
    #[test]
    fn add_add_conflict_reports_a_missing_base_stage() {
        let repository = test_repository();
        let root = repository.path();
        assert!(git(root, ["checkout", "-b", "other"]).status.success());
        std::fs::write(root.join("added.txt"), "theirs\n").unwrap();
        assert!(git(root, ["add", "added.txt"]).status.success());
        let theirs = git(
            root,
            [
                "-c",
                "user.name=slTerminal Test",
                "-c",
                "user.email=slterm@example.invalid",
                "commit",
                "-m",
                "theirs",
            ],
        );
        assert!(
            theirs.status.success(),
            "{}",
            String::from_utf8_lossy(&theirs.stderr)
        );
        assert!(git(root, ["checkout", "main"]).status.success());
        std::fs::write(root.join("added.txt"), "ours\n").unwrap();
        assert!(git(root, ["add", "added.txt"]).status.success());
        let ours = git(
            root,
            [
                "-c",
                "user.name=slTerminal Test",
                "-c",
                "user.email=slterm@example.invalid",
                "commit",
                "-m",
                "ours",
            ],
        );
        assert!(
            ours.status.success(),
            "{}",
            String::from_utf8_lossy(&ours.stderr)
        );
        let merge = git(root, ["merge", "other"]);
        assert!(!merge.status.success(), "夹具必须制造 add/add 冲突");

        let stages = conflict_stages(root, "added.txt").expect("stage 读取");
        assert!(stages.base.is_none(), "{stages:?}");
        assert_eq!(stages.ours.as_deref(), Some("ours\n"));
        assert_eq!(stages.theirs.as_deref(), Some("theirs\n"));
    }

    /// 上限截断:只读 limit 字节并标记 truncated(截断内容禁止写盘)。
    #[test]
    fn conflict_stage_reads_are_bounded_and_report_truncation() {
        let repository = conflict_repository();
        let stages =
            conflict_stages_with_limit(repository.path(), "conflicted.txt", 4).expect("stage 读取");
        assert_eq!(stages.base.as_deref(), Some("line"));
        assert_eq!(stages.ours.as_deref(), Some("line"));
        assert_eq!(stages.theirs.as_deref(), Some("line"));
        assert_eq!(stages.truncated, [true; 3]);
    }

    /// 非仓库路径:NotRepository(不 panic、不误报 HEAD)。
    #[test]
    fn paths_outside_a_worktree_are_not_repositories() {
        let directory = tempfile::tempdir().unwrap();
        let ghost = directory.path().join("ghost").join("file.txt");
        let error = repository_root(&ghost).expect_err("非仓库目录");
        assert!(matches!(error, GitError::NotRepository(_)));
    }

    /// porcelain v2:rename 身份、部分暂存与中文/换行文件名都按 NUL 记录原样保留。
    #[test]
    fn status_parser_preserves_rename_identity_and_partial_staging() {
        let status = parse_status(
            "# branch.oid abc\0# branch.head main\0# branch.ab +2 -1\0\
             2 RM N... 100644 100644 100644 abc def R100 新 name\n.txt\0old name.txt\0? -new.txt\0",
        )
        .expect("合法 porcelain v2");
        assert_eq!(status.branch, "main");
        assert_eq!(status.ahead, 2);
        assert_eq!(status.behind, 1);
        assert_eq!(status.entries.len(), 2);
        let renamed = &status.entries[0];
        assert_eq!(renamed.path, "新 name\n.txt");
        assert_eq!(renamed.original.as_deref(), Some("old name.txt"));
        assert!(renamed.staged() && renamed.unstaged());
        assert_eq!(status.entries[1].path, "-new.txt");
    }

    /// 截断/坏记录必须整体拒绝,不能产出半份状态。
    #[test]
    fn status_parser_rejects_truncated_or_malformed_output() {
        assert!(parse_status("# branch.oid abc\0# branch.head main").is_err());
        assert!(parse_status("? file\0").is_err(), "缺分支头");
        assert!(parse_status("# branch.oid abc\0# branch.head main\0? \0").is_err());
    }

    /// 六值映射:未跟踪/新增/修改/删除/重命名/冲突各归其位。
    #[test]
    fn file_status_maps_porcelain_records_to_the_gutter_values() {
        let cases = [
            ("? new.txt\0", FileStatus::Untracked),
            ("1 A. N... 0 0 0 0 0 added.txt\0", FileStatus::Added),
            ("1 .M N... 0 0 0 0 0 tracked.txt\0", FileStatus::Modified),
            ("1 .D N... 0 0 0 0 0 gone.txt\0", FileStatus::Deleted),
            (
                "2 R. N... 0 0 0 0 0 R100 new.txt\0old.txt\0",
                FileStatus::Renamed,
            ),
            (
                "u UU N... 100644 100644 100644 100644 0000 0000 0000 conflict.txt\0",
                FileStatus::Conflicted,
            ),
        ];
        for (record, expected) in cases {
            assert_eq!(
                file_status_from_porcelain(record, record_path(record)),
                Some(expected),
                "{record:?}"
            );
        }
        assert_eq!(
            file_status_from_porcelain("? other.txt\0", "tracked.txt"),
            None,
            "路径不匹配不得串台"
        );
    }

    /// 真仓库:status 摘要 64 hex;修改/暂存/回滚/取消暂存全链字节正确。
    #[test]
    fn repository_status_and_file_operations_round_trip() {
        let repository = test_repository();
        let root = repository.path();
        let path = root.join("tracked.txt");
        let relative = relative_path(root, &path).expect("仓库相对路径");

        std::fs::write(&path, "worktree edit\nmore\n").unwrap();
        let snapshot = status(root).expect("status 快照");
        assert_eq!(snapshot.digest.len(), 64, "写操作 revision 必须是 64 hex");
        assert!(snapshot.digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
        // 写操作后的断言必须读实时状态:短 TTL 缓存可能仍是操作前的快照。
        assert_eq!(
            file_status_uncached(root, &relative).unwrap(),
            Some(FileStatus::Modified)
        );
        let hunks = file_hunks(root, &relative).unwrap();
        assert_eq!(hunks.len(), 1, "单处修改应解析出一条 hunk: {hunks:?}");
        assert_eq!((hunks[0].old_start, hunks[0].old_lines), (1, 1));
        assert_eq!((hunks[0].new_start, hunks[0].new_lines), (1, 2));

        // 回滚:工作区与索引都回到 HEAD 字节。
        rollback(root, &relative).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"tracked\n");
        assert_eq!(file_status_uncached(root, &relative).unwrap(), None);

        // 暂存后 unstage:索引清空、工作区保留修改。
        std::fs::write(&path, "staged edit\n").unwrap();
        assert!(
            git(root, ["add", "tracked.txt"]).status.success(),
            "git add"
        );
        assert_eq!(
            file_status_uncached(root, &relative).unwrap(),
            Some(FileStatus::Modified)
        );
        unstage(root, &relative).unwrap();
        let after = status(root).unwrap();
        assert!(
            after.entries.iter().all(|entry| !entry.staged()),
            "unstage 后索引不得再有该文件: {:?}",
            after.entries
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"staged edit\n");

        // 未跟踪文件回滚 = 删除工作区新文件。
        let fresh = root.join("fresh.txt");
        std::fs::write(&fresh, b"new\n").unwrap();
        let fresh_relative = relative_path(root, &fresh).unwrap();
        assert_eq!(
            file_status_uncached(root, &fresh_relative).unwrap(),
            Some(FileStatus::Untracked)
        );
        rollback(root, &fresh_relative).unwrap();
        assert!(!fresh.exists(), "未跟踪文件回滚必须删除");
    }

    /// stderr 分类:权限/损坏各自归位;普通失败与空 stderr 返回 None。
    #[test]
    fn git_failure_classification_separates_permission_corruption_and_unknown() {
        let permission =
            classify_git_failure("fatal: cannot open '.git/objects/ab/cd': Permission denied")
                .expect("权限失败必须分类");
        assert!(
            matches!(permission, GitError::AccessDenied(_)),
            "{permission:?}"
        );
        assert!(permission.to_string().contains("Permission denied"));

        let windows = classify_git_failure("error: Access is denied.").expect("Windows 权限拒绝");
        assert!(matches!(windows, GitError::AccessDenied(_)), "{windows:?}");

        for stderr in [
            "fatal: loose object 5d9f is corrupt",
            "fatal: bad object HEAD",
            "error: object file .git/objects/ab/cd is empty",
            "fatal: unable to read tree 1234",
        ] {
            let corrupt = classify_git_failure(stderr).expect(stderr);
            assert!(
                matches!(corrupt, GitError::Corrupt(_)),
                "{stderr}: {corrupt:?}"
            );
        }

        assert!(
            classify_git_failure("fatal: path 'missing.txt' exists on disk, but not in 'HEAD'")
                .is_none(),
            "普通失败交给调用方默认臂"
        );
        assert!(classify_git_failure("").is_none(), "空 stderr 不得误分类");
    }

    /// unborn HEAD:unstage 清索引条目,工作区文件保留并回到未跟踪。
    #[test]
    fn unstage_unborn_head_untracks_without_touching_the_worktree() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        assert!(
            git(root, ["init", "--initial-branch=main"])
                .status
                .success(),
            "git init"
        );
        let path = root.join("new.txt");
        std::fs::write(&path, b"new bytes\n").unwrap();
        assert!(git(root, ["add", "new.txt"]).status.success(), "git add");

        unstage(root, "new.txt").unwrap();

        let after = status(root).unwrap();
        assert!(
            after.entries.iter().all(|entry| !entry.staged()),
            "索引不得再有暂存条目: {:?}",
            after.entries
        );
        assert_eq!(
            file_status_uncached(root, "new.txt").unwrap(),
            Some(FileStatus::Untracked)
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"new bytes\n");
    }

    /// 已提交仓库:staged 新文件 unstage 后回到未跟踪且字节不变。
    #[test]
    fn unstage_staged_new_file_keeps_worktree_bytes_and_untracks() {
        let repository = test_repository();
        let root = repository.path();
        let path = root.join("staged-new.txt");
        let bytes = b"staged new file\n";
        std::fs::write(&path, bytes).unwrap();
        assert!(
            git(root, ["add", "staged-new.txt"]).status.success(),
            "git add"
        );
        assert_eq!(
            file_status_uncached(root, "staged-new.txt").unwrap(),
            Some(FileStatus::Added)
        );

        unstage(root, "staged-new.txt").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(
            file_status_uncached(root, "staged-new.txt").unwrap(),
            Some(FileStatus::Untracked)
        );
        let after = status(root).unwrap();
        let entry = after
            .entries
            .iter()
            .find(|entry| entry.path == "staged-new.txt")
            .expect("未跟踪条目必须可见");
        assert_eq!((entry.index, entry.worktree), ('?', '?'));
    }

    /// hunk 解析:缺省行数与多 hunk 行号保持精确。
    #[test]
    fn hunk_parser_handles_omitted_counts_and_multiple_hunks() {
        let diff = "@@ -1 +1 @@\n-a\n+b\n@@ -10,3 +11,0 @@\n-x\n";
        let hunks = parse_hunks(diff);
        assert_eq!(hunks.len(), 2);
        assert_eq!(
            hunks[0],
            FileHunk {
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 1
            }
        );
        assert_eq!(
            hunks[1],
            FileHunk {
                old_start: 10,
                old_lines: 3,
                new_start: 11,
                new_lines: 0
            }
        );
    }

    fn record_path(record: &str) -> &str {
        // rename 记录后随源路径 NUL 记录;取第一条记录本身。
        let record = record.split('\0').next().unwrap_or(record);
        match record.as_bytes().first() {
            Some(b'?') => record.trim_start_matches("? "),
            Some(b'2') => record.splitn(10, ' ').last().unwrap(),
            Some(b'u') => record.splitn(11, ' ').last().unwrap(),
            _ => record.splitn(9, ' ').last().unwrap(),
        }
    }

    fn canonical(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    fn test_repository() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("create repository directory");
        assert!(
            git(directory.path(), ["init", "--initial-branch=main"])
                .status
                .success()
        );
        // 字节级回滚断言依赖仓库自身配置:全局 autocrlf 不得改写检出字节。
        assert!(
            git(directory.path(), ["config", "core.autocrlf", "false"])
                .status
                .success()
        );
        std::fs::write(directory.path().join("tracked.txt"), "tracked\n").expect("write tracked");
        assert!(
            git(directory.path(), ["add", "tracked.txt"])
                .status
                .success()
        );
        let commit = git(
            directory.path(),
            [
                "-c",
                "user.name=slTerminal Test",
                "-c",
                "user.email=slterm@example.invalid",
                "commit",
                "-m",
                "initial",
            ],
        );
        assert!(
            commit.status.success(),
            "git commit failed: {}",
            String::from_utf8_lossy(&commit.stderr)
        );
        directory
    }

    /// 真 modify/modify 冲突仓:base/ours/theirs 三版都已提交。
    fn conflict_repository() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("create conflict repository");
        let root = directory.path();
        assert!(
            git(root, ["init", "--initial-branch=main"])
                .status
                .success()
        );
        assert!(
            git(root, ["config", "core.autocrlf", "false"])
                .status
                .success()
        );
        std::fs::write(root.join("conflicted.txt"), "line1\nbase\nline3\n").unwrap();
        assert!(git(root, ["add", "conflicted.txt"]).status.success());
        let base = git(
            root,
            [
                "-c",
                "user.name=slTerminal Test",
                "-c",
                "user.email=slterm@example.invalid",
                "commit",
                "-m",
                "base",
            ],
        );
        assert!(
            base.status.success(),
            "{}",
            String::from_utf8_lossy(&base.stderr)
        );
        assert!(git(root, ["checkout", "-b", "other"]).status.success());
        std::fs::write(root.join("conflicted.txt"), "line1\ntheirs\nline3\n").unwrap();
        assert!(git(root, ["add", "conflicted.txt"]).status.success());
        let theirs = git(
            root,
            [
                "-c",
                "user.name=slTerminal Test",
                "-c",
                "user.email=slterm@example.invalid",
                "commit",
                "-m",
                "theirs",
            ],
        );
        assert!(
            theirs.status.success(),
            "{}",
            String::from_utf8_lossy(&theirs.stderr)
        );
        assert!(git(root, ["checkout", "main"]).status.success());
        std::fs::write(root.join("conflicted.txt"), "line1\nours\nline3\n").unwrap();
        assert!(git(root, ["add", "conflicted.txt"]).status.success());
        let ours = git(
            root,
            [
                "-c",
                "user.name=slTerminal Test",
                "-c",
                "user.email=slterm@example.invalid",
                "commit",
                "-m",
                "ours",
            ],
        );
        assert!(
            ours.status.success(),
            "{}",
            String::from_utf8_lossy(&ours.stderr)
        );
        let merge = git(root, ["merge", "other"]);
        assert!(!merge.status.success(), "夹具必须制造 modify/modify 冲突");
        directory
    }

    fn git<const N: usize>(cwd: &Path, args: [&str; N]) -> std::process::Output {
        // 测试二进制同样隐藏控制台;autocrlf 关闭,HEAD 字节与写入逐字节一致。
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(cwd)
            .args(["-c", "core.autocrlf=false"])
            .args(args);
        crate::platform::process::hidden_command(&mut command)
            .output()
            .expect("run git")
    }
}
