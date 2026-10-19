//! 两阶段安装 handoff 与 restore ticket。
//!
//! 事务目录里的文件是持久证据,崩溃后状态可重建:
//! ```text
//! 下载完成 ⇒ plan.json + ready.json(helper 就绪)
//! 用户授权 ⇒ commit.json(唯一安装权威)
//! 放弃/失败 ⇒ cancel.json + 终止 helper
//! ```
//! helper 没有 commit.json 就没有安装权威;未 commit 即 Drop 的
//! [`PreparedUpdate`] 自动写 cancel.json 并终止自己的 helper。
//!
//! 启动恢复:result.json 成功且版本匹配,或回滚后原版本匹配,才发
//! restore ticket;最多 3 次恢复尝试,`restored.json` 留档防重放。
//! 崩溃遗留的未 commit 事务(孤儿 plan / ready 无 commit)按 cancel 安全
//! 默认收尾——绝不因为「文件存在」就授权安装。
//!
//! **消费面。** prepare/commit/schedule 已由更新弹窗的安装授权步消费
//! ;restore_ticket/acknowledge_restore/apply_scheduled 由启动链
//! 消费。其余仅测试消费的机器可验 API 仍由整模块
//! dead_code 豁免兜住;消费点接线时逐个删除本豁免。
#![allow(dead_code)]

use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::platform::update_installation::{canonical, current_process_created};
use crate::session::Session;
use crate::update_check::UpdateAsset;

/// 事务 schema;与 `handoff.ps1` 内联校验值一致。
const PLAN_SCHEMA: u32 = 1;
/// helper 从启动到 ready 的等待预算(契约值照抄上游 Windows 分支)。
const PREPARE_TIMEOUT: Duration = Duration::from_secs(30);
/// 工作区快照上限:单窗应用远小于此,防御损坏/恶意文件。
const MAX_WORKSPACE_WINDOWS: usize = 32;
/// restore ticket 的最大尝试次数(崩溃循环上限)。
const MAX_RESTORE_ATTEMPTS: u32 = 3;

struct ActiveRestore {
    directory: PathBuf,
    expected_windows: usize,
}

static ACTIVE_RESTORE: Mutex<Option<ActiveRestore>> = Mutex::new(None);

#[derive(Serialize, Deserialize)]
struct Participant {
    pid: u32,
    /// 原生创建时间(FILETIME 十进制);PID 回收后必然不同。
    created: String,
}

/// 事务计划:helper 与恢复链共同读取的唯一持久事实。
#[derive(Serialize, Deserialize)]
struct Plan {
    schema: u32,
    asset: UpdateAsset,
    transaction: String,
    executable: PathBuf,
    installation: PathBuf,
    config_directory: PathBuf,
    installer: PathBuf,
    sha256: String,
    bytes: u64,
    version: String,
    original_version: String,
    guard_path: PathBuf,
    participants: Vec<Participant>,
}

/// helper 抽象:生产 = PowerShell 子进程;测试 = 脚本化替身(可断言
/// 就绪/终止/commit 授权时序)。
pub(crate) trait HelperProcess: Send {
    /// `true` = 仍在运行;`false` = 已退出(可能已失败)。
    fn running(&mut self) -> Result<bool, String>;
    /// 终止本进程自己的 helper;幂等。
    fn terminate(&mut self);
}

struct ChildHelper(Child);

impl HelperProcess for ChildHelper {
    fn running(&mut self) -> Result<bool, String> {
        Ok(self
            .0
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_none())
    }

    fn terminate(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// 事务路径注入:生产取当前进程/数据目录;测试用 tempdir 全隔离。
#[derive(Clone, Debug)]
pub(crate) struct PreparePaths {
    pub(crate) root: PathBuf,
    pub(crate) executable: PathBuf,
    pub(crate) installation: PathBuf,
    pub(crate) config_directory: PathBuf,
}

impl PreparePaths {
    fn current() -> Result<Self, String> {
        let executable = canonical(
            &std::env::current_exe().map_err(|error| format!("缺少当前可执行文件:{error}"))?,
        )
        .map_err(|error| error.to_string())?;
        let installation =
            crate::platform::update_installation::installation_directory(&executable)?;
        let root = crate::platform::dirs::data_dir().to_owned();
        let config_directory =
            canonical(&root).map_err(|error| format!("无法解析配置目录:{error}"))?;
        Ok(Self {
            root,
            executable,
            installation,
            config_directory,
        })
    }
}

pub(crate) struct PreparedUpdate {
    directory: PathBuf,
    transaction: String,
    helper: Box<dyn HelperProcess>,
    committed: bool,
}

impl PreparedUpdate {
    /// 原子写 commit.json —— 唯一授权动作。工作区快照先落盘,helper 再
    /// 等本进程退出后覆盖安装。
    pub(crate) fn commit(&mut self, windows: &[Session]) -> Result<(), String> {
        let snapshot = serde_json::to_vec(windows).map_err(|error| error.to_string())?;
        crate::atomic_file::write(&self.directory.join("workspace.json"), &snapshot)
            .map_err(|error| format!("无法保存更新前的工作区快照:{error}"))?;
        if !self.helper.running()? {
            return Err("更新 helper 在 commit 前已退出".into());
        }
        let commit = serde_json::to_vec(&serde_json::json!({ "transaction": self.transaction }))
            .map_err(|error| error.to_string())?;
        crate::atomic_file::write(&self.directory.join("commit.json"), &commit)
            .map_err(|error| format!("无法提交更新授权:{error}"))?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for PreparedUpdate {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let _ = crate::atomic_file::write(&self.directory.join("cancel.json"), b"{}");
        // 这是本进程自己未 commit 的 helper;它没有安装权威。
        self.helper.terminate();
    }
}

/// 生产入口:安装目录写探测 + helper 启动 + ready 等待全程在后台线程。
pub(crate) fn prepare(asset: &UpdateAsset) -> Result<PreparedUpdate, String> {
    prepare_at_paths(asset, PreparePaths::current()?)
}

fn prepare_at_paths(asset: &UpdateAsset, paths: PreparePaths) -> Result<PreparedUpdate, String> {
    prepare_with(asset, paths, |directory, plan| {
        crate::platform::update_installation::spawn_prepared_helper(directory, plan)
            .map(|child| Box::new(ChildHelper(child)) as Box<dyn HelperProcess>)
    })
}

/// 文件态机主体;`spawn` 是唯一平台/测试注入点。
pub(crate) fn prepare_with(
    asset: &UpdateAsset,
    paths: PreparePaths,
    spawn: impl FnOnce(&Path, &Path) -> Result<Box<dyn HelperProcess>, String>,
) -> Result<PreparedUpdate, String> {
    crate::platform::distribution::require_direct_update()?;
    if crate::platform::elevation::requires_isolation() {
        return Err("请在普通权限的 slTerminal 窗口中安装更新,避免提权会话混用".into());
    }
    let installer = super::ready_path_at(&paths.root, asset)?;
    let executable = canonical(&paths.executable).map_err(|error| error.to_string())?;
    let installation = canonical(&paths.installation).map_err(|error| error.to_string())?;
    let transaction = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let directory = paths.root.join("updates/handoffs").join(&transaction);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    // 在所有窗口仍可用时探测安装目录可写性;不可写则立即失败,不进入
    // helper 等待。
    let probe = installation.join(format!(".slterm-update-{transaction}"));
    let probe_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|error| format!("安装目录不可写:{error}"))?;
    drop(probe_file);
    std::fs::remove_file(&probe).map_err(|error| error.to_string())?;

    let created = current_process_created().map_err(|error| error.to_string())?;
    let base = crate::platform::update_installation::guard_base(&executable);
    let plan = Plan {
        schema: PLAN_SCHEMA,
        asset: asset.clone(),
        transaction: transaction.clone(),
        installation,
        executable,
        config_directory: canonical(&paths.config_directory).map_err(|error| error.to_string())?,
        installer: canonical(&installer).map_err(|error| error.to_string())?,
        sha256: asset
            .digest
            .clone()
            .ok_or_else(|| "安装包缺少 SHA-256".to_owned())?,
        bytes: std::fs::metadata(installer)
            .map_err(|error| error.to_string())?
            .len(),
        version: asset.version.clone(),
        original_version: env!("CARGO_PKG_VERSION").into(),
        guard_path: base.with_extension("slterm-lock"),
        participants: vec![Participant {
            pid: std::process::id(),
            created: created.to_string(),
        }],
    };
    let plan_path = directory.join("plan.json");
    crate::atomic_file::write(
        &plan_path,
        &serde_json::to_vec(&plan).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    crate::atomic_file::write(
        &paths.root.join("updates/last-handoff.json"),
        &serde_json::to_vec(&plan_path).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    let helper = match spawn(&directory, &plan_path) {
        Ok(helper) => helper,
        Err(error) => {
            let _ = crate::atomic_file::write(&directory.join("cancel.json"), b"{}");
            return Err(error);
        }
    };
    let mut prepared = PreparedUpdate {
        directory: directory.clone(),
        transaction: transaction.clone(),
        helper,
        committed: false,
    };
    let deadline = Instant::now() + PREPARE_TIMEOUT;
    loop {
        if !prepared.helper.running()? {
            let result = read_json::<serde_json::Value>(&directory.join("result.json"))
                .and_then(|result| result["error"].as_str().map(str::to_owned));
            return Err(result.unwrap_or_else(|| "更新 helper 未能准备安装".into()));
        }
        if let Some(ready) = read_json::<serde_json::Value>(&directory.join("ready.json"))
            && ready["transaction"] == prepared.transaction
        {
            return Ok(prepared);
        }
        if Instant::now() >= deadline {
            return Err("更新 helper 未在时限内就绪".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > 16 * 1024 * 1024 {
        return None;
    }
    serde_json::from_reader(file.take(16 * 1024 * 1024)).ok()
}

/// 事务目录的文件态(纯函数;崩溃后状态重建与孤儿清理共用)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransactionState {
    /// 没有 plan.json。
    Missing,
    /// 只有 plan(helper 未就绪):安全默认按 cancel 处理。
    Orphan,
    /// plan + ready,尚无 commit/cancel/result:未授权安装。
    Prepared,
    /// commit.json 已写 = 安装权威已授予。
    Committed,
    /// cancel.json 已写。
    Cancelled,
    /// result.json 已写(成功或失败终态)。
    Finished,
}

pub(crate) fn transaction_state(directory: &Path) -> TransactionState {
    if !directory.join("plan.json").is_file() {
        return TransactionState::Missing;
    }
    if directory.join("result.json").is_file() {
        return TransactionState::Finished;
    }
    if directory.join("cancel.json").is_file() {
        return TransactionState::Cancelled;
    }
    if directory.join("commit.json").is_file() {
        return TransactionState::Committed;
    }
    if directory.join("ready.json").is_file() {
        return TransactionState::Prepared;
    }
    TransactionState::Orphan
}

/// 启动清理:未 commit 的历史事务按 cancel 安全默认收尾。
///
/// 单实例定位下,启动时存在的 plan/ready 必来自已退出的进程;写 cancel
/// 让可能仍在等待的 helper 立刻走回滚路径,而不是等 90 秒超时。
pub(crate) fn recover_stale_handoffs(root: &Path) {
    let directory = root.join("updates/handoffs");
    let Ok(entries) = std::fs::read_dir(&directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if matches!(
            transaction_state(&path),
            TransactionState::Orphan | TransactionState::Prepared
        ) && let Err(error) = crate::atomic_file::write(&path.join("cancel.json"), b"{}")
        {
            log::warn!("update handoff: cannot cancel stale transaction {error}");
        }
    }
}

fn handoffs_root(root: &Path) -> Option<PathBuf> {
    canonical(&root.join("updates/handoffs")).ok()
}

fn last_handoff_path(root: &Path) -> Option<PathBuf> {
    read_json(&root.join("updates/last-handoff.json"))
}

/// 恢复 ticket:按工作区快照恢复,最多 3 次尝试;成功后留档防重放。
pub(crate) fn restore_ticket() -> Option<Vec<Session>> {
    let root = crate::platform::dirs::data_dir();
    let executable = std::env::current_exe().ok()?;
    restore_ticket_at(root, &executable, env!("CARGO_PKG_VERSION"))
}

pub(crate) fn restore_ticket_at(
    root: &Path,
    executable: &Path,
    current_version: &str,
) -> Option<Vec<Session>> {
    let path = std::env::var_os("SLTERM_UPDATE_RESTORE")
        .map(PathBuf::from)
        .or_else(|| last_handoff_path(root))?;
    let path = canonical(&path).ok()?;
    let handoffs = handoffs_root(root)?;
    if !path.starts_with(&handoffs) || path.file_name()? != "plan.json" {
        return None;
    }
    let plan: Plan = read_json(&path)?;
    if plan.schema != PLAN_SCHEMA
        || canonical(&plan.executable).ok()? != canonical(executable).ok()?
    {
        return None;
    }
    let directory = path.parent()?;
    let result: serde_json::Value = read_json(&directory.join("result.json"))?;
    let succeeded = result["success"] == true && plan.version == current_version;
    let recovered_original = result["success"] == false
        && result["recovered_original"] == true
        && plan.original_version == current_version;
    if (!succeeded && !recovered_original) || result["transaction"] != plan.transaction {
        return None;
    }
    let windows: Vec<Session> = read_json(&directory.join("workspace.json"))?;
    if !workspace_is_valid(&windows) {
        return None;
    }
    let count: u32 = read_json(&directory.join("restore-attempts.json")).unwrap_or(0);
    if count >= MAX_RESTORE_ATTEMPTS || directory.join("restored.json").exists() {
        return None;
    }
    crate::atomic_file::write(
        &directory.join("restore-attempts.json"),
        (count + 1).to_string().as_bytes(),
    )
    .ok()?;
    *ACTIVE_RESTORE
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some(ActiveRestore {
        directory: directory.to_owned(),
        expected_windows: windows.len(),
    });
    Some(windows)
}

fn workspace_is_valid(windows: &[Session]) -> bool {
    !windows.is_empty()
        && windows.len() <= MAX_WORKSPACE_WINDOWS
        && windows
            .iter()
            .all(|session| session.version == crate::session::VERSION)
}

/// 宿主侧确认恢复完成(窗口数与 ticket 预期一致才留档)。
pub(crate) fn acknowledge_restore(restored_windows: usize) {
    let mut active = ACTIVE_RESTORE
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let Some(ticket) = active.as_ref() else {
        return;
    };
    if restored_windows != ticket.expected_windows {
        return;
    }
    if let Err(error) = crate::atomic_file::write(&ticket.directory.join("restored.json"), b"{}") {
        log::warn!("Could not acknowledge update recovery: {error}");
    } else {
        *active = None;
    }
}

#[cfg(test)]
pub(crate) fn reset_restore_state() {
    *ACTIVE_RESTORE
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = None;
}

/// 最近一次 handoff 失败(启动 hydrate 的持久证据)。
pub(super) fn failed_update_at(root: &Path, executable: &Path) -> Option<(UpdateAsset, String)> {
    let path = canonical(&last_handoff_path(root)?).ok()?;
    let handoffs = handoffs_root(root)?;
    if !path.starts_with(&handoffs) {
        return None;
    }
    let plan: Plan = read_json(&path)?;
    if canonical(&plan.executable).ok()? != canonical(executable).ok()? {
        return None;
    }
    let result_path = path.parent()?.join("result.json");
    // 下载侧记录比安装结果新 → 用户已重新下载,旧失败不再重提示。
    if super::cache::supersedes(&result_path) {
        return None;
    }
    let result: serde_json::Value = read_json(&result_path)?;
    if result["success"] != false || result["transaction"] != plan.transaction {
        return None;
    }
    super::validate_asset(&plan.asset).ok()?;
    Some((plan.asset, result["error"].as_str()?.to_owned()))
}

pub(super) fn failure_unseen(root: &Path, executable: Option<&Path>, prompt_state: &Path) -> bool {
    let Some(executable) = executable else {
        return false;
    };
    if failed_update_at(root, executable).is_none() {
        return false;
    }
    let Some(directory) =
        last_handoff_path(root).and_then(|path| path.parent().map(Path::to_owned))
    else {
        return false;
    };
    let failure = std::fs::metadata(directory.join("result.json"))
        .and_then(|file| file.modified())
        .ok();
    let prompted = std::fs::metadata(prompt_state)
        .and_then(|file| file.modified())
        .ok();
    match (failure, prompted) {
        (Some(failure), Some(prompted)) => failure > prompted,
        (Some(_), None) => true,
        _ => false,
    }
}

/// 预约下次启动安装(不授予安装权限;`apply_scheduled` 启动早期消费)。
pub(crate) fn schedule(asset: &UpdateAsset) -> Result<(), String> {
    schedule_at(crate::platform::dirs::data_dir(), asset)
}

pub(crate) fn schedule_at(root: &Path, asset: &UpdateAsset) -> Result<(), String> {
    crate::platform::distribution::require_direct_update()?;
    if crate::platform::elevation::requires_isolation() {
        return Err("请在普通权限的 slTerminal 窗口中预约更新".into());
    }
    super::validate_asset(asset)?;
    if !matches!(super::status(asset), super::DownloadStatus::Ready { .. }) {
        return Err("安装包尚未就绪,无法预约安装".to_owned());
    }
    let data = serde_json::to_vec(asset).map_err(|error| error.to_string())?;
    crate::atomic_file::write(&root.join("updates/install-next.json"), &data)
        .map_err(|error| error.to_string())
}

/// 启动早期执行预约安装。成功 = 调用方必须真退出(helper 等待本进程
/// 退出后覆盖安装);失败解除预约并恢复普通启动。
pub(crate) fn apply_scheduled() -> bool {
    let Ok(paths) = PreparePaths::current() else {
        return false;
    };
    apply_scheduled_at(&paths)
}

pub(crate) fn apply_scheduled_at(paths: &PreparePaths) -> bool {
    if crate::platform::distribution::current().externally_managed() {
        return false;
    }
    // 早于首个 Inno 安装基的当前版本不允许自动安装。
    if crate::update_check::requires_manual_reinstall(env!("CARGO_PKG_VERSION")) {
        return false;
    }
    let path = paths.root.join("updates/install-next.json");
    let Some(asset) = read_json::<UpdateAsset>(&path) else {
        return false;
    };
    // 先解除预约:一次失败不得永久卡住后续启动。
    if crate::atomic_file::write(&path, b"null").is_err() {
        return false;
    }
    if !crate::update_check::can_install_version(&asset.version).unwrap_or(false) {
        return false;
    }
    super::hydrate_at(&paths.root, Some(&paths.executable));
    let result = prepare_at_paths(&asset, paths.clone()).and_then(|mut prepared| {
        let workspaces: Vec<Session> = crate::session::load_from(&paths.root.join("session.json"))
            .map(|session| vec![session])
            .unwrap_or_default();
        if workspaces.is_empty() {
            return Err("缺少可恢复的工作区快照".to_owned());
        }
        prepared.commit(&workspaces)
    });
    match result {
        Ok(()) => true,
        Err(error) => {
            log::warn!("Scheduled update failed; opening the application: {error}");
            let _ = super::cache::record_installation_failure(&paths.root, &asset, error);
            false
        }
    }
}

/// 守卫锁:安装器不动旁置锁文件;锁被占用 = 同安装基的更新正在进行。
pub(crate) fn installation_in_progress() -> io::Result<bool> {
    if crate::platform::distribution::current().externally_managed() {
        return Ok(false);
    }
    let executable = canonical(&std::env::current_exe()?)?;
    installation_in_progress_at(&executable)
}

pub(crate) fn installation_in_progress_at(executable: &Path) -> io::Result<bool> {
    let executable = canonical(executable)?;
    Ok(
        crate::atomic_file::try_lifetime_lock(&crate::platform::update_installation::guard_base(
            &executable,
        ))?
        .is_none(),
    )
}

#[cfg(test)]
mod tests;
