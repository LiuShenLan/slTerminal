//! 侧栏文件树模型族(07 篇 M8.4):目录树快照 / git 状态 / 文件名搜索。
//!
//! 模型与渲染完全分层(照抄点 21):
//! - 本模块只持有数据与状态转换,渲染侧只按行号读 [`SidePanel::file_rows`];
//! - 目录枚举在 [`enumerate`],gitignore 过滤索引在 [`gitignore`],文件名
//!   搜索在 [`search`],git 快照在 [`vcs`];旧壳 `render.rs`/`notice.rs` 不迁;
//! - WSL guest 臂、SVN/tortoise 臂、git 写操作全部裁掉(SSH/WSL/SVN 不迁,
//!   边界条 9;写操作归 M8.5)。
//!
//! 快照流(07 篇「文件树快照流」):聚焦 pane cwd 变化 / 显式刷新
//! → `needs_refresh` 重臂 → 后台快照 worker(generation 守卫防串台)
//! → 枚举 + gitignore 过滤索引 + git 状态快照 → `sync` 收割落地。
//! 目录内容**不做定时重扫**(照上游 2026-08-21 裁定):触发点只有明确
//! 事件,终端里新建/删除文件由刷新按钮兜底。
//!
//! 搜索:非空 query → 有界文件名索引流式过滤,`rows` 换平铺结果集;
//! 清空 → `tree_rows` 立即恢复,不重新走盘。

mod enumerate;
mod gitignore;
mod icons;
mod search;
mod vcs;

#[cfg(test)]
#[path = "side_panel_tests.rs"]
mod side_panel_tests;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub(crate) use icons::{chevron_icon, file_type_icon, folder_icon};
pub(crate) use search::{EmbeddedFileIndex, FileIndexStatus, FileSearchMemory, FileSearchOptions};

use crate::i18n::{Message, UiLanguage};

/// 一条平铺后的目录树行(渲染唯一数据源)。
#[derive(Debug, Clone)]
pub(crate) struct FileRow {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
    pub expanded: bool,
    /// 切换到上级根目录的合成 `..` 行。必须显式区分,避免普通目录的
    /// 展开/双击逻辑把导航项误认为真实文件系统条目。
    pub is_parent: bool,
    /// gitignore 命中:留在树序,降色渲染(边界条 8「看得见但无状态色」)。
    pub ignored: bool,
}

/// `git log --all --date-order` 的一条提交;显示层依据对象 ID 与父提交
/// 生成连续轨道,不把 `--graph` 字符画当视觉数据。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GitCommit {
    /// 完整对象 ID 建立真实父子关系;短哈希只用于界面显示。
    pub full_hash: String,
    pub short_hash: String,
    pub decorations: String,
    pub subject: String,
    pub author: String,
    pub timestamp: i64,
    /// 按 Git 记录顺序:第一个是主线,后续父提交是合并进来的线路。
    pub parent_hashes: Vec<String>,
}

/// 解析后的 `git status --porcelain=v2 -z` 快照(SVN 臂已砍)。
///
/// 展示面消费(树着色/gutter/commit 面板)归 M8.5;本步只由 vcs 用例直读,
/// 故整体放行未读字段。
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub(crate) struct GitInfo {
    /// 当前分支(或 detached HEAD 的短描述)。
    pub branch: String,
    /// 工作区增删行数(未暂存 + 已暂存)。
    pub plus: u64,
    pub minus: u64,
    /// 领先上游的提交数——push 按钮以此判定可推送工作。
    pub ahead: u32,
    /// 工作树变更未暂存(`??` 记为 `?`)。
    pub unstaged: Vec<(char, String)>,
    /// 已进入索引、可提交的变更。
    pub staged: Vec<(char, String)>,
    /// 合并冲突(porcelain 的 U*/AA/DD)。冲突路径同时保留在
    /// staged/unstaged 中;视图按本列表单独分组并过滤,避免重复显示。
    pub conflicts: Vec<(char, String)>,
    /// 最近提交(`--date-order`)。
    pub history: Vec<GitCommit>,
}

/// 面板顶部的一句话提示(根目录不可用等)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelNotice {
    /// 用户显式指定的根已不存在。
    DirectoryUnavailable,
    /// 指定根失效后回到跟随聚焦 pane cwd。
    FollowingDirectory,
}

impl PanelNotice {
    pub(crate) fn text(self, language: UiLanguage) -> &'static str {
        match self {
            Self::DirectoryUnavailable => language.text(Message::FilesRootUnavailable),
            Self::FollowingDirectory => language.text(Message::FilesFollowingDirectory),
        }
    }
}

/// 后台快照 worker 的产物:原 `refresh` 在渲染线程上同步算的全部内容。
struct PanelSnapshot {
    /// 快照所属 generation;root 变化/主动刷新都会推进,旧快照丢弃。
    generation: u64,
    root: PathBuf,
    rows: Vec<FileRow>,
    /// 本次枚举是否全部成功。失败时 `rows` 可能为空,但**不代表目录是空的**。
    enumeration_ok: bool,
    /// `None` 表示目录行先行发布、VCS 仍在读取;`Some(None)` 才表示当前
    /// 目录不在仓库中。
    git: Option<Option<GitInfo>>,
}

/// worker panic/提前返回也必须解锁;否则一次异常会让该面板今后的刷新失效。
struct SnapshotRunningGuard(Arc<AtomicBool>);

impl Drop for SnapshotRunningGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// 文件树模型。渲染侧只经 [`Self::file_rows`] 读数据。
pub(crate) struct SidePanel {
    /// 树/git 快照的构建根。
    root: Option<PathBuf>,
    /// 最近一次观测到的聚焦 pane cwd;自定义根期间保留,便于立即恢复跟随。
    followed_cwd: Option<PathBuf>,
    /// 窗口内浏览覆盖(树内点 `..` 或路径栏导航);不持久化。
    custom_root: Option<PathBuf>,
    /// 根不可用/恢复跟随的可见反馈。
    root_notice: Option<PanelNotice>,
    /// 平铺可见行。
    rows: Vec<FileRow>,
    /// 未过滤快照;清搜索立即恢复,不重新走盘。
    tree_rows: Vec<FileRow>,
    /// 展开态跨刷新存活。
    expanded: HashSet<PathBuf>,
    /// git 快照;`None` = 根不在 work tree 内。
    git: Option<Arc<GitInfo>>,
    /// 行内滚动偏移(模型层展示态;uniform_list 自成滚动容器后恒 0)。
    pub scroll: usize,
    /// 搜索过滤词;非空时切换为平铺结果集。
    pub search: String,
    /// 根范围有界文件名索引(搜索 worker 单例)。
    file_index: EmbeddedFileIndex,
    /// 上一次搜索结果的保留租约;清结果即释放。
    search_memory: Option<FileSearchMemory>,
    search_options: FileSearchOptions,
    search_generation: u64,
    search_applied_generation: u64,
    search_index_epoch: u64,
    search_index_root: Option<PathBuf>,
    search_index_refresh_requested: bool,
    search_total: usize,
    search_error: Option<String>,
    /// 持久选中行(高亮);点空白/根变化/关闭视图时清除。
    pub selected: Option<PathBuf>,
    /// 后台快照 worker 在飞;同时只允许一个。
    snapshot_running: Arc<AtomicBool>,
    /// worker 落地快照槽,由 `harvest_snapshot` 收割。
    snapshot_slot: Arc<Mutex<Option<PanelSnapshot>>>,
    /// 快照 generation:每次 refresh 推进,防旧 worker 串台。
    snapshot_generation: u64,
    /// 上一次落地枚举是否失败(读不到 ≠ 目录为空)。
    enumeration_failed: bool,
    /// 下一帧需要重建快照(显式刷新/根切换重臂)。
    needs_refresh: bool,
}

impl SidePanel {
    pub(crate) fn new() -> Self {
        Self {
            root: None,
            followed_cwd: None,
            custom_root: None,
            root_notice: None,
            rows: Vec::new(),
            tree_rows: Vec::new(),
            expanded: HashSet::new(),
            git: None,
            scroll: 0,
            search: String::new(),
            file_index: EmbeddedFileIndex::new(),
            search_memory: None,
            search_options: FileSearchOptions::default(),
            search_generation: 0,
            search_applied_generation: 0,
            search_index_epoch: 0,
            search_index_root: None,
            search_index_refresh_requested: false,
            search_total: 0,
            search_error: None,
            selected: None,
            snapshot_running: Default::default(),
            snapshot_slot: Default::default(),
            snapshot_generation: 0,
            enumeration_failed: false,
            needs_refresh: false,
        }
    }

    /// 采纳聚焦 pane 的 cwd:根变化或刷新重臂时重建快照。
    ///
    /// 每次绘制帧调用一次;无变化时零成本。返回是否刷新了可见状态。
    pub(crate) fn sync(&mut self, cwd: Option<PathBuf>) -> bool {
        // 先收割落地的后台快照——旧内容在 worker 跑动期间一直显示,这里
        // 一次性换成新内容,避免刷新期间出现空白。
        let mut changed = self.harvest_snapshot();
        changed |= self.harvest_file_search();
        // 聚焦 pane 报不出位置时保留最后一个有效根;有明确新位置时恢复
        // 实时跟随,树内点 `..` 产生的浏览覆盖不能继续压住新 cwd。
        let followed_changed = cwd
            .as_ref()
            .is_some_and(|next| self.followed_cwd.as_ref() != Some(next));
        let follow_override_cleared = if followed_changed {
            self.root_notice = None;
            self.custom_root.take().is_some()
        } else {
            false
        };
        if let Some(cwd) = cwd {
            self.followed_cwd = Some(cwd);
        }
        let custom_invalidated = self.custom_root.as_ref().is_some_and(|root| !root.is_dir());
        if custom_invalidated {
            self.custom_root = None;
            self.root_notice = Some(PanelNotice::FollowingDirectory);
        }
        let next_root = self
            .custom_root
            .clone()
            .or_else(|| self.followed_cwd.clone());
        let root_changed = next_root != self.root;
        if !(root_changed || custom_invalidated || follow_override_cleared || self.needs_refresh) {
            return changed;
        }
        if root_changed {
            self.root = next_root;
            self.expanded.clear();
            self.scroll = 0;
            self.selected = None;
        }
        self.refresh();
        changed || root_changed || custom_invalidated || follow_override_cleared
    }

    /// 测试辅助:等后台快照落地并收割。生产路径永不阻塞。
    #[cfg(test)]
    fn wait_snapshot(&mut self) {
        for _ in 0..1000 {
            if !self.snapshot_running.load(Ordering::Acquire) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        self.harvest_snapshot();
    }

    /// 收割后台快照。渲染外也允许调用(测试/通知泵)。
    fn harvest_snapshot(&mut self) -> bool {
        let snapshot = match self.snapshot_slot.lock() {
            Ok(mut slot) => slot.take(),
            Err(error) => error.into_inner().take(),
        };
        let Some(snapshot) = snapshot else {
            return false;
        };
        // 工人跑动期间根又变了:这份快照已过期,丢弃并为新根重臂。
        // 返回 true 让通知泵再排一帧——否则事件驱动壳里可能没有下一次
        // 绘制,重臂的刷新要等到用户碰巧触发重绘才跑。
        if snapshot.generation != self.snapshot_generation
            || self.root.as_ref() != Some(&snapshot.root)
        {
            self.needs_refresh = true;
            return true;
        }
        self.tree_rows = snapshot.rows;
        if self.search.trim().is_empty() {
            self.rows.clone_from(&self.tree_rows);
        }
        self.enumeration_failed = !snapshot.enumeration_ok;
        if let Some(git) = snapshot.git {
            self.git = git.map(Arc::new);
        }
        true
    }

    fn harvest_file_search(&mut self) -> bool {
        let Some(result) = self.file_index.take_result() else {
            return false;
        };
        if result.epoch != self.search_index_epoch
            || self.search_index_root != self.current_index_root()
            || result.generation != self.search_generation
            || result.query != self.search
            || result.options != self.search_options
        {
            return false;
        }
        self.rows = result.rows;
        self.search_memory = Some(result.memory);
        self.search_total = result.total;
        self.search_error = result.error;
        if self.search_applied_generation != result.generation {
            self.scroll = 0;
        }
        self.search_applied_generation = result.generation;
        true
    }

    /// 通知泵单点:收割快照与搜索结果,返回是否有可见变化。
    pub(crate) fn harvest_pending(&mut self) -> bool {
        let snapshot = self.harvest_snapshot();
        let search = self.harvest_file_search();
        snapshot || search
    }

    /// 是否仍有后台工作(快照工人/搜索 worker)在飞。
    pub(crate) fn pending(&self) -> bool {
        self.snapshot_pending() || self.file_search_pending()
    }

    /// 覆盖聚焦 pane cwd,只影响本面板实例(D05-3:不持久化)。
    pub(crate) fn set_custom_root(&mut self, root: PathBuf) -> bool {
        if !root.is_dir() {
            self.root_notice = Some(PanelNotice::DirectoryUnavailable);
            return false;
        }
        let changed = self.custom_root.as_ref() != Some(&root) || self.root.as_ref() != Some(&root);
        self.custom_root = Some(root.clone());
        self.root_notice = None;
        if self.root.as_ref() != Some(&root) {
            self.root = Some(root);
            self.expanded.clear();
            self.scroll = 0;
            self.selected = None;
            self.refresh();
        }
        changed
    }

    /// 恢复跟随最近一次观测到的聚焦 pane cwd。
    pub(crate) fn clear_custom_root(&mut self) -> bool {
        if self.custom_root.take().is_none() {
            return false;
        }
        self.root_notice = None;
        if self.root != self.followed_cwd {
            self.root = self.followed_cwd.clone();
            self.expanded.clear();
            self.scroll = 0;
            self.selected = None;
            self.refresh();
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn custom_root_active(&self) -> bool {
        self.custom_root.is_some()
    }

    #[cfg(test)]
    pub(crate) fn root_notice(&self) -> Option<&PanelNotice> {
        self.root_notice.as_ref()
    }

    pub(crate) fn localized_root_notice(&self, language: UiLanguage) -> Option<&'static str> {
        self.root_notice.map(|notice| notice.text(language))
    }

    pub(crate) fn request_refresh(&mut self) {
        self.needs_refresh = true;
        self.search_index_refresh_requested = true;
    }

    fn current_index_root(&self) -> Option<PathBuf> {
        self.root.clone()
    }

    fn sync_file_index(&mut self) {
        let root = self.current_index_root();
        let refresh_requested = std::mem::take(&mut self.search_index_refresh_requested);
        if self.search_index_root == root {
            if refresh_requested {
                self.file_index.refresh(self.search_index_epoch);
            }
            return;
        }
        self.search_index_root = root.clone();
        self.search_index_epoch = self.search_index_epoch.wrapping_add(1);
        self.search_generation = self.search_generation.wrapping_add(1);
        self.search_error = None;
        self.search_total = 0;
        let active_query = if self.search.trim().is_empty() {
            None
        } else {
            self.rows = Vec::new();
            self.search_memory = None;
            Some((
                self.search_generation,
                self.search.clone(),
                self.search_options,
            ))
        };
        self.file_index
            .rebuild(root, self.search_index_epoch, active_query);
    }

    /// 重建树与 git 快照(后台 worker 一次只跑一个)。
    fn refresh(&mut self) {
        self.needs_refresh = false;
        let Some(root) = self.root.clone() else {
            // 无根:清空即时且无成本,不需要 worker。
            self.rows = Vec::new();
            self.tree_rows = Vec::new();
            self.search_memory = None;
            self.git = None;
            self.sync_file_index();
            return;
        };
        if self.snapshot_running.swap(true, Ordering::AcqRel) {
            // worker 在飞:重挂 needs_refresh,落地后的下一帧自然补跑。
            self.needs_refresh = true;
            return;
        }
        self.snapshot_generation = self.snapshot_generation.wrapping_add(1);
        let generation = self.snapshot_generation;
        let expanded = self.expanded.clone();
        self.sync_file_index();
        // git 状态跟着显式浏览根/聚焦 cwd 走(本形态二者同一 root)。
        let git_root = self.custom_root.clone().unwrap_or_else(|| root.clone());
        let running = Arc::clone(&self.snapshot_running);
        let running_fallback = Arc::clone(&self.snapshot_running);
        let slot = Arc::clone(&self.snapshot_slot);
        let spawned = std::thread::Builder::new()
            .name("slterm file tree".to_owned())
            .spawn(move || {
                let _running_guard = SnapshotRunningGuard(running);
                let rows = enumerate::tree_rows(&root, &expanded);
                // 文件列表是主结果,不能被随后可能耗时的 git 探测扣住:
                // 先发布目录行,VCS 完成后再用同一行集补全快照。
                publish_snapshot(
                    &slot,
                    PanelSnapshot {
                        generation,
                        root: root.clone(),
                        rows: rows.clone(),
                        enumeration_ok: true,
                        git: None,
                    },
                );
                let git = vcs::read_git(&git_root);
                publish_snapshot(
                    &slot,
                    PanelSnapshot {
                        generation,
                        root,
                        rows,
                        enumeration_ok: true,
                        git: Some(git),
                    },
                );
            });
        if spawned.is_err() {
            // 线程起不来:解锁并保留一次重臂,不能让面板永久停在
            // 「正在读取」。
            running_fallback.store(false, Ordering::Release);
            self.needs_refresh = true;
        }
    }

    /// 重建平铺行(树形态/过滤变化;git 快照保留)。
    fn rebuild_rows(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        if !self.search.trim().is_empty() {
            self.queue_file_search();
            return;
        }
        self.tree_rows = enumerate::tree_rows(&root, &self.expanded);
        self.rows.clone_from(&self.tree_rows);
    }

    fn queue_file_search(&mut self) {
        self.scroll = 0;
        self.search_error = None;
        self.search_total = 0;
        self.search_generation = self.search_generation.wrapping_add(1);
        if self.search.trim().is_empty() {
            self.file_index.clear_query();
            self.rows.clone_from(&self.tree_rows);
            self.search_memory = None;
            self.search_applied_generation = self.search_generation;
            return;
        }
        self.file_index.query(
            self.search_index_epoch,
            self.search_generation,
            self.search.clone(),
            self.search_options,
        );
    }

    /// 替换过滤词。输入组件的 caret/IME 归 gpui-component,模型只留净化文本。
    pub(crate) fn set_file_search_query(&mut self, mut query: String) {
        query.retain(|character| !matches!(character, '\r' | '\n'));
        if self.search == query {
            return;
        }
        self.search = query;
        self.queue_file_search();
    }

    pub(crate) fn file_search_pending(&self) -> bool {
        !self.search.trim().is_empty()
            && (self.file_index.status() == FileIndexStatus::Building
                || self.search_applied_generation != self.search_generation)
    }

    pub(crate) fn file_index_truncated(&self) -> bool {
        self.file_index.truncated()
    }

    pub(crate) fn file_search_total(&self) -> usize {
        self.search_total
    }

    pub(crate) fn file_search_error(&self) -> Option<&str> {
        self.search_error.as_deref()
    }

    /// 目录行单击:目录切换展开;`..` 行切换窗口内根;文件不响应。
    pub(crate) fn click_row(&mut self, index: usize) -> bool {
        if !self.search.trim().is_empty() {
            return false;
        }
        let Some(row) = self.rows.get(index).cloned() else {
            return false;
        };
        if !row.is_dir {
            return false;
        }
        if row.is_parent {
            // 返回上级只改窗口内浏览根,不能连带修改终端 cwd。
            return self.set_custom_root(row.path);
        }
        if !self.expanded.remove(&row.path) {
            self.expanded.insert(row.path);
        }
        // 只重排行(不重跑 git):树形态变了,内容没变。
        self.rebuild_rows();
        true
    }

    /// 路径栏/搜索目录行共用的窗口内根切换入口。
    pub(crate) fn browse_directory(&mut self, path: PathBuf) -> bool {
        self.set_custom_root(path)
    }

    pub(crate) fn file_rows(&self) -> &[FileRow] {
        &self.rows
    }

    /// 首份目录快照是否仍在后台读取(区分「正在读取」与真空目录)。
    pub(crate) fn snapshot_pending(&self) -> bool {
        self.snapshot_running.load(Ordering::Acquire)
    }

    /// 上一次落地的枚举是否失败;为真且行为空时 UI 必须提示可重试。
    pub(crate) fn enumeration_failed(&self) -> bool {
        self.enumeration_failed
    }

    /// 消费点 = M8.5 树着色/gutter/commit 面板;本步只由 vcs 用例直读。
    #[allow(dead_code)]
    pub(crate) fn git(&self) -> Option<&GitInfo> {
        self.git.as_deref()
    }

    /// 消费点 = M8.5 树着色/提交面板的快照订阅(Arc 共享,不复制)。
    #[allow(dead_code)]
    pub(crate) fn git_snapshot(&self) -> Option<Arc<GitInfo>> {
        self.git.clone()
    }

    pub(crate) fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }
}

/// 一条路径落进 PTY 时该写出的字节。`None` = 这条路径不能安全地写进去。
///
/// 「什么样的路径可以写、写成什么形状」只有这一处定义:少了控制字符那道
/// 闸,一个换行就能让「粘贴路径」变成「执行命令」。参数是字符串而非
/// `Path`:要写的是 shell 里的拼写,转一圈宿主路径语义可能改写它。
///
/// 默认引用形态 = PowerShell(产品默认 shell);多路径/跨 shell 形态走
/// [`drop_text_for_paths`](07 篇交界 43)。
pub(crate) fn drop_text_for_path(path: &str) -> Option<Vec<u8>> {
    drop_text_for_paths(&[path.to_owned()], PathQuote::PowerShell).map(String::into_bytes)
}

/// 目标 shell 的引用规则(多 shell 形态归 02 篇交界 43 消费;
/// 本步默认 PowerShell,其余变体由用例锁定)。
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(crate) enum PathQuote {
    CommandPrompt,
    PowerShell,
    Posix,
}

/// 仅粘贴、不提交的路径参数。拒绝控制字节与 CMD 展开符,避免文件名在用户
/// 最终回车时变成第二条命令。
pub(crate) fn drop_text_for_paths(paths: &[String], shell: PathQuote) -> Option<String> {
    if paths.is_empty() || paths.len() > 32 {
        return None;
    }
    let mut output = String::new();
    for path in paths {
        if path.is_empty() || path.chars().any(char::is_control) {
            return None;
        }
        let safe = path.chars().all(|ch| {
            ch.is_alphanumeric()
                || "/._-:".contains(ch)
                || matches!(shell, PathQuote::CommandPrompt | PathQuote::PowerShell) && ch == '\\'
        });
        match shell {
            PathQuote::CommandPrompt => {
                if path.contains(['"', '%', '!']) {
                    return None;
                }
                if safe {
                    output.push_str(path);
                } else {
                    output.push('"');
                    output.push_str(path);
                    output.push('"');
                }
            }
            PathQuote::PowerShell if !safe => {
                output.push('\'');
                output.push_str(&path.replace('\'', "''"));
                output.push('\'');
            }
            PathQuote::Posix if !safe => {
                output.push('\'');
                output.push_str(&path.replace('\'', "'\\''"));
                output.push('\'');
            }
            _ => output.push_str(path),
        }
        output.push(' ');
        if output.len() > 64 * 1024 {
            return None;
        }
    }
    Some(output)
}

fn publish_snapshot(slot: &Mutex<Option<PanelSnapshot>>, snapshot: PanelSnapshot) {
    match slot.lock() {
        Ok(mut slot) => *slot = Some(snapshot),
        Err(error) => *error.into_inner() = Some(snapshot),
    }
}

#[cfg(test)]
mod dropped_path_tests {
    use super::{PathQuote, drop_text_for_paths};

    /// 目标 shell 引用形态;尾随空格即「粘贴后不提交」。
    #[test]
    fn paths_are_quoted_for_the_target_shell_without_submitting() {
        assert_eq!(
            drop_text_for_paths(&["D:\\研究资料\\a b.txt".into()], PathQuote::PowerShell).unwrap(),
            "'D:\\研究资料\\a b.txt' "
        );
        assert_eq!(
            drop_text_for_paths(&["/tmp/it's $(touch nope).txt".into()], PathQuote::Posix).unwrap(),
            "'/tmp/it'\\''s $(touch nope).txt' "
        );
        assert_eq!(
            drop_text_for_paths(
                &["C:\\a&b.txt".into(), "C:\\two.txt".into()],
                PathQuote::CommandPrompt
            )
            .unwrap(),
            "\"C:\\a&b.txt\" C:\\two.txt "
        );
    }

    /// 控制字节/CMD 展开符/超长列表一律拒绝。
    #[test]
    fn unsafe_or_unbounded_path_lists_are_rejected() {
        for path in ["/tmp/a\ncommand", "/tmp/a\u{1b}[200~", ""] {
            assert!(drop_text_for_paths(&[path.into()], PathQuote::Posix).is_none());
        }
        assert!(
            drop_text_for_paths(&["C:\\%PATH%.txt".into()], PathQuote::CommandPrompt).is_none()
        );
        assert!(drop_text_for_paths(&vec!["a".into(); 33], PathQuote::PowerShell).is_none());
    }
}
