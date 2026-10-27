//! 会话 schema 与持久化;同时承载会话身份校验。
//!
//! session.json 只回答「下次启动恢复成什么样」:1 Hz 快照持续写盘、启动前
//! 断路器计数、收尾最后一笔标记正常退出;不承担驻留/多窗语义。格式版本从
//! 1 起(无历史会话文件,升级链不迁),追加字段一律 `serde(default)` 免
//! 升版。同一 schema 兼作工作区导出格式——快照(紧凑写)与导出(pretty
//! 打印)只有序列化形态之别。
//!
//! 布局持久化单点:运行时树与持久化树之间的转换只有
//! `app_shell::session_restore` 的一对纯函数;本模块只定义 schema 与存取。
//! 快照泵/启动恢复归壳层。少数全局路径包装在生产链上
//! 由显式 `_at` 路径替代,逐项标注 dead_code 原因。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use slterm_settings::Rgb;
use slterm_sidebar::{SidebarRecovery, SidebarRecoveryError, SidebarState};

/// 最高格式版本。无历史会话文件,升级链不迁;追加字段走
/// `serde(default)` 免升版。
pub const VERSION: u32 = 1;

/// 连续这么多次启动都没活到首次自动保存就不再恢复(断路器)。
pub const MAX_BOOT_ATTEMPTS: u32 = 3;

/// tab 首 pane 的启动身份(可持久化的启动子集:仅默认启动与本地 Profile)。
/// Profile 内嵌完整命令而非引用配置名,导出工作区在目标机配置不同时仍可移植。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LaunchSession {
    Default,
    Profile {
        name: String,
        command: String,
        args: Vec<String>,
        cwd: Option<String>,
        #[serde(default)]
        shell_id: Option<String>,
    },
}

/// 分屏轴。与运行时 `SplitDirection` 同形,持久化层不直接依赖显示类型。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SplitAxis {
    LeftRight,
    TopBottom,
}

/// 快照瞬间仍在 pane 前台运行的 AI CLI 对话身份(字段级消费归 AI 集成域)。
/// 只存安全启动描述——来源名 + 会话 id + 原生会话文件,不存正文、不存启动
/// 参数原文。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSession {
    /// hook 直报的 CLI 名(slug)。
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// 原生会话文件;只在所属 pane 环境内解释。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_file: Option<String>,
}

impl AgentSession {
    /// 恢复时敲进 shell 的命令行;无法安全构造(未知来源、异形 id、codex
    /// 缺 id)时返回 `None`,宁可只恢复布局也不上屏可疑字节。
    ///
    /// session id 是不可信的 hook/文件输入:`AgentKind::resume_command` 先做
    /// 字符集与长度校验,绝不加引号硬拼(SEC 注入面)。命令形状与
    /// `ai_sessions::AiSession::resume_command`(手动恢复面板)保持一致。
    pub fn resume_command(&self) -> Option<String> {
        let agent = crate::ai_agents::AgentKind::parse(&self.source)?;
        if agent == crate::ai_agents::AgentKind::Codex
            && let Some(id) = self.session_file.as_deref().and_then(codex_rollout_id)
        {
            return agent.resume_command(id);
        }
        match self.session_id.as_deref() {
            // Pi 的 `pid-` 前缀是进程身份而非可恢复会话身份。
            Some(id) if agent == crate::ai_agents::AgentKind::Pi && id.starts_with("pid-") => None,
            Some(id) => agent.resume_command(id),
            // claude 无 id(hook 未装、OSC 认出的启动):`--continue` 恢复
            // 当前目录最近一次对话——pane 的 cwd 已经先被恢复,语义正好。
            None if agent == crate::ai_agents::AgentKind::Claude => {
                Some("claude --continue".to_owned())
            }
            // codex 没有按目录 continue 的形式,缺 id 只能放弃接续。
            _ => None,
        }
    }
}

/// 持久化布局树:叶带逐 pane 启动环境,Split 带轴 + 第一个孩子占比。
/// 占比用千分比整数——自动保存的变化检测与文件 diff 不被 f32 序列化噪声
/// 绊倒,与上游同因。
///
/// 变体尺寸差是线格式的事实:Pane 字段级照设计稿锚定,Box 化会改 schema
/// 签名与转换对形态,故显式豁免该 lint。
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayoutSession {
    Pane {
        cwd: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        custom_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent: Option<AgentSession>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        launch: Option<LaunchSession>,
    },
    Split {
        axis: SplitAxis,
        ratio_permille: u16,
        first: Box<LayoutSession>,
        second: Box<LayoutSession>,
    },
}

/// 恢复注入需要的叶数据(布局单点访问面)。恢复侧只消费本结构,不直接
/// 解构 `LayoutSession` 变体——持久化树形状仍只在 schema 与转换对里出现。
#[derive(Debug, Clone, Copy)]
pub struct PaneData<'a> {
    pub cwd: &'a str,
    pub custom_name: Option<&'a str>,
    pub agent: Option<&'a AgentSession>,
    pub launch: Option<&'a LaunchSession>,
}

impl LayoutSession {
    /// 缺省布局的规范形态:单 pane at `cwd`(`TabSession.layout = None`
    /// 的恢复路径不得自行拼变体)。
    pub fn single_pane(cwd: impl Into<String>) -> Self {
        Self::Pane {
            cwd: cwd.into(),
            custom_name: None,
            agent: None,
            launch: None,
        }
    }

    /// 叶数据;`Split` 节点返回 `None`。与 [`Self::leaves`] 配对使用:
    /// `leaves()` 给出 DFS 序,`pane_data()` 给出逐叶启动环境。
    pub fn pane_data(&self) -> Option<PaneData<'_>> {
        match self {
            Self::Pane {
                cwd,
                custom_name,
                agent,
                launch,
            } => Some(PaneData {
                cwd,
                custom_name: custom_name.as_deref(),
                agent: agent.as_ref(),
                launch: launch.as_ref(),
            }),
            Self::Split { .. } => None,
        }
    }

    /// 树内 pane(叶)数。
    pub fn pane_count(&self) -> usize {
        match self {
            Self::Pane { .. } => 1,
            Self::Split { first, second, .. } => first.pane_count() + second.pane_count(),
        }
    }

    /// 深度优先首个叶的目录——恢复后首个 pane 的种子目录。
    pub fn first_cwd(&self) -> &str {
        match self {
            Self::Pane { cwd, .. } => cwd,
            Self::Split { first, .. } => first.first_cwd(),
        }
    }

    /// 深度优先叶序。恢复注入按同一顺序逐叶 spawn,索引 i 与第 i 个活叶
    /// 一一配对。
    pub fn leaves(&self) -> Vec<&LayoutSession> {
        fn walk<'tree>(node: &'tree LayoutSession, out: &mut Vec<&'tree LayoutSession>) {
            match node {
                LayoutSession::Pane { .. } => out.push(node),
                LayoutSession::Split { first, second, .. } => {
                    walk(first, out);
                    walk(second, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(self, &mut out);
        out
    }
}

/// `Rgb` 值模型零 serde(设置契约),落盘形态固定为三元数组,由本适配层
/// 表达;`None` 落为 null。
mod rgb_triplet {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use slterm_settings::Rgb;

    pub(super) fn serialize<S: Serializer>(
        value: &Option<Rgb>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(rgb) => [rgb.0, rgb.1, rgb.2].serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Rgb>, D::Error> {
        let value = Option::<[u8; 3]>::deserialize(deserializer)?;
        Ok(value.map(|[red, green, blue]| Rgb(red, green, blue)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TabSession {
    /// 焦点 pane 的目录。树重建前 boot 路径用它种子首 pane。
    pub cwd: String,
    /// 用户重命名;`None` 沿用目录/标题派生标签。
    #[serde(default)]
    pub custom_name: Option<String>,
    /// 用户色标;`None` 跟随当前主题。
    #[serde(default, with = "rgb_triplet")]
    pub color: Option<Rgb>,
    /// 首 pane 的启动身份;`None` 表示默认本地启动。
    #[serde(default)]
    pub launch: Option<LaunchSession>,
    /// 完整分屏树;`None` = 单 pane at `cwd`。
    #[serde(default)]
    pub layout: Option<LayoutSession>,
    /// 焦点叶在 `layout` 中的深度优先下标;越界回退首叶。
    #[serde(default)]
    pub active_pane: usize,
}

/// 上次窗口的逻辑像素尺寸与最大化态。已裁回放:字段语义 = 启动
/// 行为输入(记住上次窗口大小),不再只是诊断数据。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub maximized: bool,
}

/// session 内嵌的 core 侧栏原始 JSON；生产快照写为规范形态。
///
/// 保留原始对象而非二次定义字段：恢复时仍能检查任意层级重复字段，严格 DTO
/// 与容错恢复都只经 `slterm_sidebar`。
#[derive(Debug, Clone)]
pub struct SidebarSession(Box<RawValue>);

impl PartialEq for SidebarSession {
    fn eq(&self, other: &Self) -> bool {
        self.0.get() == other.0.get()
    }
}

impl SidebarSession {
    pub(crate) fn from_state(state: &SidebarState) -> Self {
        let json = state
            .to_canonical_json()
            .expect("有效侧栏状态必须可编码为规范 JSON");
        Self(RawValue::from_string(json).expect("core 规范 JSON 必须可作为 RawValue"))
    }

    pub(crate) fn recover(&self) -> Result<SidebarRecovery, SidebarRecoveryError> {
        SidebarRecovery::from_json(self.0.get())
    }
}

impl Default for SidebarSession {
    fn default() -> Self {
        Self::from_state(&SidebarState::default())
    }
}

impl Serialize for SidebarSession {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SidebarSession {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Box::<RawValue>::deserialize(deserializer).map(Self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub version: u32,
    /// 断路器计数:活到首次自动保存即归零。
    #[serde(default)]
    pub boot_attempts: u32,
    /// 活跃 tab 的位置下标(稳定身份是运行时 TabId,不进 schema)。
    pub active_tab: usize,
    pub tabs: Vec<TabSession>,
    #[serde(default)]
    pub window: Option<WindowState>,
    /// core 侧栏组织状态的原始 JSON；缺整段时写默认值，恢复警告由启动链聚合。
    #[serde(default)]
    pub sidebar: SidebarSession,
    /// 1 Hz 自动保存恒 false,只有正常收尾的最后一笔写 true;老文件缺省
    /// false(按异常退出多提示一次,恢复行为不变)。
    #[serde(default)]
    pub clean_exit: bool,
}

impl Session {
    /// 新快照的规范形态:当前版本、断路器归零、未走收尾、侧栏默认。
    pub fn new(active_tab: usize, tabs: Vec<TabSession>) -> Self {
        Self {
            version: VERSION,
            boot_attempts: 0,
            active_tab,
            tabs,
            window: None,
            sidebar: SidebarSession::default(),
            clean_exit: false,
        }
    }
}

/// 数据目录里的规范落点(目录解析归 `platform::dirs`)。
pub(crate) fn session_path() -> PathBuf {
    crate::platform::dirs::local_dir().join("session.json")
}

/// 解析会话/工作区文档。版本不符即拒收——无历史格式,不做就地升级。
fn parse(data: &str) -> Option<Session> {
    let session: Session = serde_json::from_str(data).ok()?;
    (session.version == VERSION).then_some(session)
}

/// 从数据目录读取上次会话;缺失、损坏或版本不符都视为无会话。
#[allow(dead_code)] // 设计稿锚定 API;生产启动走显式路径(测试可注入 tempdir)。
pub fn load() -> Option<Session> {
    load_from(&session_path())
}

/// 从显式路径读取会话/工作区文件(工作区导入同一 schema)。
pub fn load_from(path: &Path) -> Option<Session> {
    parse(&std::fs::read_to_string(path).ok()?)
}

/// 紧凑写(机读快照形态);临时文件 + rename 的原子性归 `atomic_file`。
pub fn try_save(session: &Session) -> std::io::Result<()> {
    save_compact_to(&session_path(), session)
}

fn save_compact_to(path: &Path, session: &Session) -> std::io::Result<()> {
    let json = serde_json::to_string(session).map_err(std::io::Error::other)?;
    crate::atomic_file::write(path, json.as_bytes())
}

/// 以命名工作区文件写出:pretty 打印,供人读、可 diff。快照与导出共用
/// 同一 schema,只是序列化形态不同。
pub fn save_to(path: &Path, session: &Session) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(session).map_err(std::io::Error::other)?;
    crate::atomic_file::write(path, json.as_bytes())
}

/// 是否恢复:断路器未跳闸且确有 tab。空会话(一路关标签关干净)不恢复。
pub fn should_restore(session: &Session) -> bool {
    session.boot_attempts < MAX_BOOT_ATTEMPTS && !session.tabs.is_empty()
}

/// 收尾时的最后一笔:先打 `clean_exit = true` 再落盘。所有正常退出路径
/// 都必须走这里,否则下次启动会把这次退出误判成崩溃。
#[allow(dead_code)] // 设计稿锚定 API;生产收尾走 SessionPersistence(需无变化跳过)。
pub fn save_final(session: &mut Session) {
    if let Err(error) = save_final_at(&session_path(), session) {
        log::warn!("Could not persist final terminal session: {error}");
    }
}

fn save_final_at(path: &Path, session: &mut Session) -> std::io::Result<()> {
    session.clean_exit = true;
    save_compact_to(path, session)
}

/// 上次是异常退出(崩溃/强杀/断电),而不是正常收尾。空会话不算。
pub fn was_crash(session: &Session) -> bool {
    !session.clean_exit && !session.tabs.is_empty()
}

/// 断路器跳闸(连续 [`MAX_BOOT_ATTEMPTS`] 次启动都没活到首次自动保存)时,
/// 把这份会话挪到 `session.crashed.json` 再走干净启动。
///
/// 必须挪走而不是留在原地:启动一成功,一秒后的自动保存就会把
/// `session.json` 盖掉,那份「一恢复就崩」的现场是唯一的诊断材料。
#[allow(dead_code)] // 设计稿锚定 API;生产启动走显式路径(测试可注入 tempdir)。
pub fn quarantine() -> Option<PathBuf> {
    quarantine_at(&session_path())
}

pub(crate) fn quarantine_at(path: &Path) -> Option<PathBuf> {
    let to = path.with_file_name("session.crashed.json");
    std::fs::copy(path, &to).ok()?;
    let _ = std::fs::remove_file(path);
    Some(to)
}

/// 保存的 cwd 转 `PathBuf`,目录已消失则返回 `None`。不存在的启动目录会让
/// ConPTY 直接失败,调用方据此回退默认 cwd。
pub fn valid_dir(cwd: &str) -> Option<PathBuf> {
    let cwd = cwd.trim();
    if cwd.is_empty() {
        return None;
    }
    let path = PathBuf::from(cwd);
    path.is_dir().then_some(path)
}

/// 恢复尝试前先把计数落盘,这样恢复过程中崩溃会被断路器记到。
#[allow(dead_code)] // 设计稿锚定 API;生产启动走显式路径(测试可注入 tempdir)。
pub fn mark_boot_attempt(session: &mut Session) {
    if let Err(error) = mark_boot_attempt_at(&session_path(), session) {
        log::warn!("Could not persist boot attempt count: {error}");
    }
}

pub(crate) fn mark_boot_attempt_at(path: &Path, session: &mut Session) -> std::io::Result<()> {
    session.boot_attempts = session.boot_attempts.saturating_add(1);
    save_compact_to(path, session)
}

/// 同时校验 Windows 与来宾绝对路径,不查宿主文件系统——真实存在性与原生
/// 身份归宿主侧校验。
pub(crate) fn valid_native_session_file(path: &str) -> bool {
    path.len() <= 4096
        && path.ends_with(".jsonl")
        && !path.chars().any(char::is_control)
        && (path.starts_with('/')
            || path.starts_with(r"\\")
            || (path.as_bytes().get(1) == Some(&b':')
                && path.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                && matches!(path.as_bytes().get(2), Some(b'/' | b'\\'))))
}

/// codex rollout 文件名尾部的 thread uuid 是 `codex resume` 认的会话身份。
/// 形状钉死:`rollout-<日期时间>-<uuid>.jsonl`,uuid 36 字符、连字符位固定。
pub(crate) fn codex_rollout_id(path: &str) -> Option<&str> {
    if !valid_native_session_file(path) {
        return None;
    }
    let filename = path.rsplit(['/', '\\']).next()?;
    let stem = filename.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
    let start = stem.len().checked_sub(36)?;
    let id = stem.get(start..)?;
    if start == 0 || stem.as_bytes().get(start - 1) != Some(&b'-') {
        return None;
    }
    id.bytes()
        .enumerate()
        .all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
        .then_some(id)
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn native_session_file_accepts_absolute_jsonl_paths_only() {
        for path in [
            r"C:\Users\user\.codex\sessions\2026\09\21\rollout-x.jsonl",
            "C:/Users/user/.claude/projects/p/x.jsonl",
            "/home/user/.codex/sessions/rollout-x.jsonl",
            r"\\wsl$\distro\home\user\x.jsonl",
        ] {
            assert!(valid_native_session_file(path), "{path}");
        }
        for path in [
            "relative/rollout-x.jsonl",
            "x.jsonl.txt",
            r"C:\Users\user\x.json",
            "",
        ] {
            assert!(!valid_native_session_file(path), "{path}");
        }
        assert!(
            !valid_native_session_file("C:/x/evil\n.jsonl"),
            "控制字符拒收"
        );
    }

    #[test]
    fn rollout_id_comes_only_from_a_well_shaped_filename_tail() {
        const THREAD: &str = "0199a213-c2a4-7cf5-8f6b-d746fbb6e86c";
        for root in [
            r"C:\Users\user\.codex\sessions",
            "/home/user/.codex/sessions",
        ] {
            let path = format!("{root}/2026/09/21/rollout-2026-09-21T10-00-00-{THREAD}.jsonl");
            assert_eq!(codex_rollout_id(&path), Some(THREAD));
        }
        for path in [
            format!("/sessions/rollout-{THREAD}.jsonl"),
            format!("/sessions/rollout-2026-09-21-{THREAD}extra.jsonl"),
            "/sessions/x.jsonl".to_owned(),
        ] {
            assert_eq!(codex_rollout_id(&path), None, "{path}");
        }
    }
}

#[cfg(test)]
mod schema_tests {
    use super::*;

    /// JSON 实例即夹具:快照形态 = 导出形态(同一 schema)。
    const WORKSPACE: &str = r#"{
      "version": 1,
      "boot_attempts": 0,
      "active_tab": 1,
      "tabs": [
        {
          "cwd": "D:/work/api",
          "custom_name": "后端",
          "color": [97, 175, 239],
          "launch": { "kind": "profile", "name": "pwsh-elevated",
                      "command": "pwsh.exe", "args": ["-NoLogo"], "cwd": "D:/work/api" },
          "layout": {
            "kind": "split",
            "axis": "left_right",
            "ratio_permille": 618,
            "first": {
              "kind": "pane", "cwd": "D:/work/api",
              "agent": { "source": "claude", "session_id": "0199a213-c2a4-7cf5-8f6b-d746fbb6e86c" }
            },
            "second": {
              "kind": "split",
              "axis": "top_bottom",
              "ratio_permille": 500,
              "first": { "kind": "pane", "cwd": "D:/work/api/logs" },
              "second": { "kind": "pane", "cwd": "" }
            }
          },
          "active_pane": 2
        },
        { "cwd": "D:/work/web", "custom_name": null, "active_pane": 0,
          "layout": { "kind": "pane", "cwd": "D:/work/web" } }
      ],
      "window": { "width": 1280, "height": 720, "maximized": false },
      "clean_exit": false
    }"#;

    #[test]
    fn workspace_instance_round_trips_field_identically() {
        let session = parse(WORKSPACE).expect("实例必须可解析");
        assert_eq!(session.version, VERSION);
        assert_eq!(session.active_tab, 1);
        assert_eq!(session.tabs.len(), 2);
        assert_eq!(session.tabs[0].custom_name.as_deref(), Some("后端"));
        assert_eq!(session.tabs[0].color, Some(Rgb(97, 175, 239)));
        assert_eq!(session.tabs[0].active_pane, 2);
        let layout = session.tabs[0].layout.as_ref().expect("split 树");
        assert_eq!(layout.pane_count(), 3);
        assert_eq!(layout.first_cwd(), "D:/work/api");
        assert_eq!(
            session.tabs[0].launch,
            Some(LaunchSession::Profile {
                name: "pwsh-elevated".into(),
                command: "pwsh.exe".into(),
                args: vec!["-NoLogo".into()],
                cwd: Some("D:/work/api".into()),
                shell_id: None,
            })
        );

        let reprinted = serde_json::to_string_pretty(&session).expect("必须可序列化");
        assert_eq!(parse(&reprinted), Some(session), "字段级往返相等");
    }

    #[test]
    fn version_mismatch_is_rejected_instead_of_upgraded() {
        assert_eq!(VERSION, 1);
        for version in [0, 2, 4] {
            let json = format!(r#"{{"version":{version},"active_tab":0,"tabs":[]}}"#);
            assert_eq!(parse(&json), None, "版本 {version} 必须拒收");
        }
        assert_eq!(parse("not json"), None, "损坏文件视为无会话");
    }

    /// 追加字段走 `serde(default)`:只带必需字段的形态照常解析。
    #[test]
    fn missing_optional_fields_take_serde_defaults() {
        let json = r#"{"version":1,"active_tab":0,"tabs":[{"cwd":"D:/work"}]}"#;
        let session = parse(json).expect("缺省字段必须解析");
        assert_eq!(session.boot_attempts, 0);
        assert_eq!(session.window, None);
        assert!(!session.clean_exit);
        let recovery = session.sidebar.recover().expect("默认侧栏必须有效");
        assert_eq!(recovery.state, SidebarState::default());
        assert!(recovery.warnings.is_empty());
        let tab = &session.tabs[0];
        assert_eq!(tab.custom_name, None);
        assert_eq!(tab.color, None);
        assert_eq!(tab.launch, None);
        assert_eq!(tab.layout, None);
        assert_eq!(tab.active_pane, 0);
    }

    /// core 侧栏状态经 session 原始 JSON 往返，不经过第二套字段 DTO。
    #[test]
    fn sidebar_state_round_trips_through_the_session_schema() {
        use slterm_sidebar::SideSlot;
        use slterm_workspace::SideViewId;

        let mut state = SidebarState::default();
        state.click_button(SideViewId::Explorer).unwrap();
        state
            .move_button_to_slot(SideViewId::AgentFiles, SideSlot::Lower, 0)
            .unwrap();
        state.click_button(SideViewId::AgentFiles).unwrap();
        state.set_width(312);
        state.set_ratio(0.618).unwrap();
        state.toggle_sidebar();

        let mut session = Session::new(0, Vec::new());
        session.sidebar = SidebarSession::from_state(&state);
        let json = serde_json::to_string_pretty(&session).expect("必须可序列化");
        let parsed = parse(&json).expect("侧栏段必须可解析");
        let recovery = parsed.sidebar.recover().expect("规范侧栏必须可恢复");
        assert_eq!(recovery.state, state);
        assert!(recovery.warnings.is_empty());

        let reprinted = serde_json::to_string_pretty(&parsed).expect("必须可序列化");
        assert_eq!(parse(&reprinted), Some(parsed), "字段级往返相等");
    }

    /// 段内脏数据由 core 容错恢复逐字段处理并聚合警告，不拒收整份会话。
    #[test]
    fn dirty_sidebar_entries_are_recovered_with_aggregated_warnings() {
        let json = r#"{
          "version": 1,
          "active_tab": 0,
          "tabs": [],
          "sidebar": {
            "version": 1,
            "kind": "sidebar_state",
            "buttons": [
              {"slot":"upper","view":"nav"},
              {"slot":"upper","view":"explorer"},
              {"slot":"upper","view":"commit"},
              {"slot":"upper","view":"agent_files"}
            ],
            "hidden_buttons": [],
            "open_views": [{"slot":"upper","view":"obsolete"}],
            "visible_intent": false,
            "width": 9999,
            "ratio": 2.0
          }
        }"#;
        let session = parse(json).expect("脏侧栏段必须留给恢复转换");
        let recovery = session.sidebar.recover().expect("字段级恢复必须成功");
        assert_eq!(
            recovery.state.open_view(slterm_sidebar::SideSlot::Upper),
            None
        );
        assert_eq!(recovery.state.width(), SidebarState::DEFAULT_WIDTH);
        assert_eq!(recovery.state.ratio(), SidebarState::DEFAULT_RATIO);
        for warning in [
            slterm_sidebar::SidebarRecoveryWarning::UnknownView,
            slterm_sidebar::SidebarRecoveryWarning::InvalidWidth,
            slterm_sidebar::SidebarRecoveryWarning::InvalidRatio,
        ] {
            assert!(recovery.warnings.contains(&warning), "缺少警告 {warning:?}");
        }
    }

    /// 缺比例只回退该字段并报告，其余有效组织状态保留。
    #[test]
    fn a_sidebar_section_without_a_ratio_takes_the_default() {
        let json = r#"{
          "version": 1,
          "active_tab": 0,
          "tabs": [],
          "sidebar": {
            "version": 1,
            "kind": "sidebar_state",
            "buttons": [
              {"slot":"upper","view":"nav"},
              {"slot":"upper","view":"explorer"},
              {"slot":"upper","view":"commit"},
              {"slot":"upper","view":"agent_files"}
            ],
            "hidden_buttons": [],
            "open_views": [{"slot":"upper","view":"nav"}],
            "visible_intent": true,
            "width": 250
          }
        }"#;
        let session = parse(json).expect("缺比例字段必须留给恢复转换");
        let recovery = session.sidebar.recover().expect("字段级恢复必须成功");
        assert_eq!(recovery.state.ratio(), SidebarState::DEFAULT_RATIO);
        assert_eq!(
            recovery.state.open_view(slterm_sidebar::SideSlot::Upper),
            Some(slterm_workspace::SideViewId::Nav)
        );
        assert_eq!(
            recovery.warnings,
            [slterm_sidebar::SidebarRecoveryWarning::InvalidRatio]
        );
    }

    /// null 是侧栏结构损坏；是否回退及如何提示由启动恢复链统一裁定。
    #[test]
    fn null_sidebar_section_is_deferred_to_recovery() {
        let json = r#"{"version":1,"active_tab":0,"tabs":[],"sidebar":null}"#;
        let session = parse(json).expect("原始侧栏 JSON 必须保留");
        assert_eq!(
            session.sidebar.recover().err(),
            Some(SidebarRecoveryError::InvalidStructure)
        );
    }

    /// RawValue 不在 session 解析阶段抹掉嵌套重复字段，core 恢复必须检出。
    #[test]
    fn sidebar_duplicate_fields_survive_session_parsing_for_recovery_to_reject() {
        let json = r#"{
          "version":1,"active_tab":0,"tabs":[],
          "sidebar": {
            "version":1,"kind":"sidebar_state",
            "buttons":[
              {"slot":"upper","view":"nav"},
              {"slot":"upper","view":"explorer"},
              {"slot":"upper","view":"commit"},
              {"slot":"upper","view":"agent_files"}
            ],
            "hidden_buttons":[],"open_views":[],"visible_intent":true,
            "width":250,"width":640,"ratio":0.5
          }
        }"#;
        let session = parse(json).expect("原始侧栏 JSON 必须保留重复字段");
        assert_eq!(
            session.sidebar.recover().err(),
            Some(SidebarRecoveryError::InvalidStructure)
        );
    }

    #[test]
    fn layout_leaves_walk_depth_first_in_restore_injection_order() {
        let layout = LayoutSession::Split {
            axis: SplitAxis::LeftRight,
            ratio_permille: 500,
            first: Box::new(LayoutSession::Pane {
                cwd: "a".into(),
                custom_name: None,
                agent: None,
                launch: None,
            }),
            second: Box::new(LayoutSession::Split {
                axis: SplitAxis::TopBottom,
                ratio_permille: 500,
                first: Box::new(LayoutSession::Pane {
                    cwd: "b".into(),
                    custom_name: None,
                    agent: None,
                    launch: None,
                }),
                second: Box::new(LayoutSession::Pane {
                    cwd: "c".into(),
                    custom_name: None,
                    agent: None,
                    launch: None,
                }),
            }),
        };
        let leaves = layout.leaves();
        assert_eq!(leaves.len(), layout.pane_count());
        let cwds: Vec<_> = leaves
            .iter()
            .map(|leaf| match leaf {
                LayoutSession::Pane { cwd, .. } => cwd.as_str(),
                LayoutSession::Split { .. } => unreachable!("leaves 只返回叶"),
            })
            .collect();
        assert_eq!(cwds, ["a", "b", "c"]);
        assert_eq!(layout.first_cwd(), "a");
    }

    #[test]
    fn tab_color_persists_as_a_plain_triplet() {
        let tab = TabSession {
            cwd: "D:/work".into(),
            custom_name: None,
            color: Some(Rgb(1, 2, 3)),
            launch: None,
            layout: None,
            active_pane: 0,
        };
        let value = serde_json::to_value(&tab).expect("必须可序列化");
        assert_eq!(value["color"], serde_json::json!([1, 2, 3]));
        assert_eq!(serde_json::from_value::<TabSession>(value).unwrap(), tab);
    }

    /// 恢复命令逐来源精确构造;异形 id 一律拒收,不拼可疑字节进 shell。
    #[test]
    fn agent_resume_commands_are_exact_and_injection_safe() {
        let agent = |source: &str, id: Option<&str>, file: Option<&str>| AgentSession {
            source: source.into(),
            session_id: id.map(str::to_owned),
            session_file: file.map(str::to_owned),
        };
        assert_eq!(
            agent("claude", Some("abc-123"), None)
                .resume_command()
                .as_deref(),
            Some("claude --resume abc-123")
        );
        assert_eq!(
            agent("claude", None, None).resume_command().as_deref(),
            Some("claude --continue"),
            "claude 缺 id 时按目录 continue"
        );
        assert_eq!(
            agent("codex", Some("abc-123"), None)
                .resume_command()
                .as_deref(),
            Some("codex resume abc-123")
        );
        assert_eq!(
            agent("gemini", Some("abc"), None)
                .resume_command()
                .as_deref(),
            Some("gemini --resume abc")
        );
        assert_eq!(agent("codex", None, None).resume_command(), None);
        assert_eq!(
            agent("aider", Some("abc"), None).resume_command(),
            None,
            "无 resume 语法的来源"
        );
        assert_eq!(
            agent("pi", Some("pid-42"), None).resume_command(),
            None,
            "进程身份不是会话身份"
        );
        for id in ["abc; rm -rf /", "", "a b", "id\nnext"] {
            assert_eq!(
                agent("claude", Some(id), None).resume_command(),
                None,
                "异形 id 必须拒收:{id:?}"
            );
        }
        assert_eq!(
            agent("claude", Some(&"a".repeat(65)), None).resume_command(),
            None,
            "超长 id 拒收"
        );
        // 未知来源不猜命令。
        assert_eq!(
            agent("unknown-cli", Some("abc"), None).resume_command(),
            None
        );
    }

    /// codex 的原生 rollout 文件优先于缺 id 的 session_id 槽:thread uuid
    /// 从合法文件名尾部解析,路径本身也不进入 shell。
    #[test]
    fn codex_resume_prefers_the_native_rollout_identity() {
        const THREAD: &str = "0199a213-c2a4-7cf5-8f6b-d746fbb6e86c";
        let file = format!("/sessions/rollout-2026-09-21T10-00-00-{THREAD}.jsonl");
        let agent = AgentSession {
            source: "codex".into(),
            session_id: None,
            session_file: Some(file),
        };
        assert_eq!(
            agent.resume_command(),
            Some(format!("codex resume {THREAD}"))
        );
        let hostile = AgentSession {
            source: "codex".into(),
            session_id: None,
            session_file: Some("/sessions/rollout-x; calc.jsonl".into()),
        };
        assert_eq!(hostile.resume_command(), None, "异形文件名不得进命令");
    }

    /// 缺省布局(`TabSession.layout = None`)的规范形态:单 pane at cwd。
    #[test]
    fn a_missing_layout_normalizes_to_a_single_pane() {
        let layout = LayoutSession::single_pane("D:/work");
        assert_eq!(layout.pane_count(), 1);
        assert_eq!(layout.first_cwd(), "D:/work");
        let data = layout.pane_data().expect("单 pane 必须可读");
        assert_eq!(data.cwd, "D:/work");
        assert_eq!(data.launch, None);
    }

    /// 叶数据访问面与 `leaves()` 的 DFS 序一一配对,恢复侧不必解构变体。
    #[test]
    fn pane_data_pairs_with_the_depth_first_leaf_order() {
        let layout = LayoutSession::Split {
            axis: SplitAxis::LeftRight,
            ratio_permille: 500,
            first: Box::new(LayoutSession::Pane {
                cwd: "a".into(),
                custom_name: Some("前端".into()),
                agent: None,
                launch: Some(LaunchSession::Default),
            }),
            second: Box::new(LayoutSession::Pane {
                cwd: "b".into(),
                custom_name: None,
                agent: None,
                launch: None,
            }),
        };
        let data: Vec<_> = layout
            .leaves()
            .iter()
            .filter_map(|leaf| leaf.pane_data())
            .collect();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].cwd, "a");
        assert_eq!(data[0].custom_name, Some("前端"));
        assert_eq!(data[0].launch, Some(&LaunchSession::Default));
        assert_eq!(data[1].cwd, "b");
        assert_eq!(data[1].launch, None);
        assert!(layout.pane_data().is_none(), "Split 节点无叶数据");
    }

    #[test]
    fn profile_launch_embeds_the_full_command() {
        let launch: LaunchSession = serde_json::from_str(
            r#"{"kind":"profile","name":"pwsh","command":"pwsh.exe","args":["-NoLogo"]}"#,
        )
        .expect("Profile 必须可解析");
        assert_eq!(
            launch,
            LaunchSession::Profile {
                name: "pwsh".into(),
                command: "pwsh.exe".into(),
                args: vec!["-NoLogo".into()],
                cwd: None,
                shell_id: None,
            }
        );
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use std::fs;

    fn sample() -> Session {
        let tab = TabSession {
            cwd: "D:/work".into(),
            custom_name: Some("后端".into()),
            color: Some(Rgb(97, 175, 239)),
            launch: Some(LaunchSession::Profile {
                name: "pwsh".into(),
                command: "pwsh.exe".into(),
                args: vec!["-NoLogo".into()],
                cwd: Some("D:/work".into()),
                shell_id: None,
            }),
            layout: Some(LayoutSession::Split {
                axis: SplitAxis::LeftRight,
                ratio_permille: 618,
                first: Box::new(LayoutSession::Pane {
                    cwd: "D:/work".into(),
                    custom_name: None,
                    agent: Some(AgentSession {
                        source: "claude".into(),
                        session_id: Some("abc-1".into()),
                        session_file: None,
                    }),
                    launch: None,
                }),
                second: Box::new(LayoutSession::Pane {
                    cwd: String::new(),
                    custom_name: Some("日志".into()),
                    agent: None,
                    launch: None,
                }),
            }),
            active_pane: 1,
        };
        let mut session = Session::new(0, vec![tab]);
        session.window = Some(WindowState {
            width: 1280,
            height: 720,
            maximized: true,
        });
        session
    }

    /// 快照 = 导出:pretty 导出文件经同一 schema 回读,字段级相等。
    #[test]
    fn pretty_export_reads_back_field_identical() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("slterm-workspace.json");
        let session = sample();
        save_to(&path, &session).unwrap();
        assert_eq!(load_from(&path), Some(session));
    }

    #[test]
    fn compact_snapshot_and_pretty_export_share_one_schema() {
        let dir = tempfile::tempdir().unwrap();
        let snapshot = dir.path().join("session.json");
        let export = dir.path().join("export.json");
        let session = sample();
        save_compact_to(&snapshot, &session).unwrap();
        save_to(&export, &session).unwrap();
        assert_eq!(load_from(&snapshot), Some(session.clone()));
        assert_eq!(load_from(&snapshot), load_from(&export));
    }

    #[test]
    fn missing_and_corrupt_files_load_as_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_from(&dir.path().join("absent.json")), None);
        let corrupt = dir.path().join("corrupt.json");
        fs::write(&corrupt, b"{not json").unwrap();
        assert_eq!(load_from(&corrupt), None);
    }

    #[test]
    fn should_restore_stops_at_the_breaker_and_ignores_empty_sessions() {
        let mut session = sample();
        assert!(should_restore(&session));
        session.boot_attempts = MAX_BOOT_ATTEMPTS;
        assert!(!should_restore(&session), "断路器跳闸后不再恢复");
        assert!(
            !should_restore(&Session::new(0, Vec::new())),
            "空会话不恢复"
        );
    }

    #[test]
    fn only_an_unfinished_teardown_counts_as_a_crash() {
        let mut session = sample();
        assert!(was_crash(&session), "1 Hz 快照 = 还没走到收尾");
        session.clean_exit = true;
        assert!(!was_crash(&session));
        assert!(!was_crash(&Session::new(0, Vec::new())), "空会话是正常退出");
    }

    #[test]
    fn save_final_marks_the_last_snapshot_as_clean() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let mut session = sample();
        assert!(!session.clean_exit);
        save_final_at(&path, &mut session).unwrap();
        let restored = load_from(&path).expect("收尾快照必须可回读");
        assert!(restored.clean_exit);
        assert!(!was_crash(&restored));
    }

    #[test]
    fn mark_boot_attempt_counts_on_disk_before_restore() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let mut session = sample();
        mark_boot_attempt_at(&path, &mut session).unwrap();
        assert_eq!(session.boot_attempts, 1);
        assert_eq!(load_from(&path).unwrap().boot_attempts, 1);
        // 首次成功快照的规范形态恒为 0。
        assert_eq!(Session::new(0, Vec::new()).boot_attempts, 0);
    }

    /// 挪不删:断路隔离后原路径清空,诊断材料原样保留。
    #[test]
    fn quarantine_moves_the_crashed_session_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let session = sample();
        save_compact_to(&path, &session).unwrap();
        let quarantined = quarantine_at(&path).expect("必须产出隔离文件");
        assert_eq!(quarantined, dir.path().join("session.crashed.json"));
        assert!(!path.exists(), "原路径必须清空");
        assert_eq!(load_from(&quarantined), Some(session), "诊断材料原样保留");
        assert_eq!(quarantine_at(&path), None, "原文件不存在时无操作");
    }

    /// 半写现场不可见:临时文件崩在换名前,规范路径只会是上一笔完整快照。
    #[test]
    fn atomic_save_leaves_no_partial_or_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let session = sample();
        save_compact_to(&path, &session).unwrap();
        save_compact_to(&path, &session).unwrap();
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("slterm-tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "临时文件必须被换名或回收:{leftovers:?}"
        );

        // 崩在换名前留下的半写临时文件不可能被 load 看见。
        fs::write(
            dir.path().join(".session.json.slterm-tmp-999-1"),
            b"{\"version\":1",
        )
        .unwrap();
        assert_eq!(load_from(&path), Some(session));
    }

    #[test]
    fn valid_dir_requires_a_real_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            valid_dir(dir.path().to_str().unwrap()),
            Some(dir.path().to_path_buf())
        );
        assert_eq!(valid_dir("   "), None);
        assert_eq!(
            valid_dir(&dir.path().join("missing").to_string_lossy()),
            None,
            "消失的目录必须回退"
        );
    }
}
