//! 归一化的 provider 事实。不做终端扫屏、I/O 或 pane 变更。

use super::payload::{MESSAGE_MAX_CHARS, truncate};

/// 一条生命周期事件对 pane 回合状态的意义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiHookKind {
    /// agent 进程/会话上线;通常是 session id 的最早一条边。
    SessionStart,
    /// 用户提交了 prompt:回合进行中。
    PromptSubmit,
    /// 一件工具完成;清掉过期的权限/提问等待。
    ToolComplete,
    /// 回合结束;CLI 等下一条指令。
    TurnDone,
    /// 显式权限请求或输入提问正阻塞 CLI。
    NeedsAttention,
    /// agent 会话退出,不再持有这个 pane。
    SessionEnd,
}

/// 停下的回合不一定是成功的回答。只有 provider 显式的结果元数据可以分类;
/// 绝不扫 assistant 散文判成败。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiTurnOutcome {
    /// 旧版 Pi 桥不带结果元数据;保留它们的活性边。
    Unspecified,
    Succeeded,
    Failed,
    Cancelled,
    Incomplete,
    Unknown,
}

/// Provider 的 hook 能力并不对称。这里描述 slTerminal 当前实际安装的桥接
/// 能力,避免上层把「有生命周期 hook」误当成「也有权限上下文或事件顺序
/// 保证」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiHookCapabilities {
    /// 已安装的桥既报回合开始也报回合结束。
    pub lifecycle: bool,
    /// 桥还报显式的等待/恢复。这与回合边界是两件事:Pi 的扩展没有权限回调。
    pub attention_events: bool,
    pub attention_context: bool,
    pub background_tasks: bool,
    /// slTerminal 自己的 bridge 是否为事件盖了单调序号。**没有任何 provider
    /// 提供原生顺序字段**:opencode/pi 的序号由我们注入的 plugin/extension
    /// 生成(启动纪元 × 1e6 + 自增),claude/codex 的 hook 完全没有顺序信息,
    /// 只能依赖本地到达顺序。名字里是 bridge 而不是 provider,正是这个原因。
    pub bridge_sequence: bool,
    pub serialized_delivery: bool,
}

/// 已安装 codex 命令 hook 的版本化合同。legacy notify 没有开始/权限权威;
/// 在 native hooks 跑起来之前它一直可用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CodexHookMode {
    Turns,
    Full,
}

/// 合同覆盖的事件表:协议解析(本域)与安装器写配置共用同一张表,
/// 禁第二份清单。
pub(crate) const CODEX_TURN_EVENTS: &[&str] = &["SessionStart", "UserPromptSubmit", "Stop"];
pub(crate) const CODEX_FULL_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PermissionRequest",
    "PreToolUse",
    "PostToolUse",
    "Stop",
    "Interrupt",
    "SessionEnd",
];

impl CodexHookMode {
    pub(crate) fn events(self) -> &'static [&'static str] {
        match self {
            Self::Turns => CODEX_TURN_EVENTS,
            Self::Full => CODEX_FULL_EVENTS,
        }
    }
}

pub fn capabilities_for(source: &str) -> AiHookCapabilities {
    match source {
        "claude" => AiHookCapabilities {
            lifecycle: true,
            attention_events: true,
            attention_context: true,
            background_tasks: true,
            bridge_sequence: false,
            serialized_delivery: false,
        },
        "opencode" => AiHookCapabilities {
            lifecycle: true,
            attention_events: true,
            attention_context: true,
            background_tasks: false,
            bridge_sequence: true,
            serialized_delivery: true,
        },
        "pi" | "omp" => AiHookCapabilities {
            lifecycle: true,
            attention_events: false,
            attention_context: false,
            background_tasks: false,
            bridge_sequence: true,
            serialized_delivery: false,
        },
        // codex notify 当前只给 turn-complete;没有 permission payload,也没有
        // 可验证的 provider sequence。接收顺序只能代表本机实际到达顺序。
        "codex" => AiHookCapabilities {
            lifecycle: false,
            attention_events: false,
            attention_context: false,
            background_tasks: false,
            bridge_sequence: false,
            serialized_delivery: false,
        },
        "kimi" | "copilot" | "grok" => AiHookCapabilities {
            lifecycle: true,
            attention_events: true,
            attention_context: true,
            background_tasks: false,
            bridge_sequence: false,
            serialized_delivery: false,
        },
        "cursor" => AiHookCapabilities {
            lifecycle: true,
            attention_events: false,
            attention_context: false,
            background_tasks: false,
            bridge_sequence: false,
            serialized_delivery: false,
        },
        _ => AiHookCapabilities {
            lifecycle: false,
            attention_events: false,
            attention_context: false,
            background_tasks: false,
            bridge_sequence: false,
            serialized_delivery: false,
        },
    }
}

/// agent 当前的权限档位。**这不是「正在等你批准」**,两者必须分开:
///
/// * `BypassPermissions` 是一个持续状态——用户用 `--dangerously-skip-permissions`
///   起的会话根本不会来问,把它当成 awaiting 会让徽标永远误亮;
/// * `NeedsAttention` 是一次瞬时事件,只有真的卡住等人时才发。
///
/// 合起来才能正确回答「这个 pane 现在是不是在无人监督地改我的仓库」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiPermissionMode {
    /// 每次动作都会问。
    Default,
    /// 自动同意文件编辑,其余仍会问。
    AcceptEdits,
    /// 全部跳过:不会有任何权限请求抵达。
    BypassPermissions,
    /// 只读规划,不落盘。
    Plan,
}

impl AiPermissionMode {
    /// claude 的 hook 载荷直接带 `permission_mode`,这比读 agent 进程的 argv
    /// 可靠得多——会话中途切档、包装脚本启动、`npx claude` 都不会反映在命令行
    /// 里。Windows 上读别的进程命令行还要 WMI 或 PEB 遍历,代价与收益完全不成
    /// 比例,所以这里只认 provider 自己声明的值,读不到就是 `None`。
    pub(super) fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "default" => Some(Self::Default),
            "acceptEdits" | "accept_edits" => Some(Self::AcceptEdits),
            "bypassPermissions" | "bypass_permissions" => Some(Self::BypassPermissions),
            "plan" => Some(Self::Plan),
            _ => None,
        }
    }

    /// 这一档会不会产生权限请求。用来判断一个没有明确类型的通知该不该被
    /// 解释成「等你批准」。
    pub fn can_ask_for_permission(self) -> bool {
        !matches!(self, Self::BypassPermissions)
    }
}

/// claude `Stop` 会携带本回合的后台 task 列表。主回合停止不等于后台
/// subagent 已经停止;至少一个 task 仍 running 时,pane 必须保持 Working。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AiBackgroundTasks {
    pub active: u32,
    pub total: u32,
}

/// 权限/等待输入事件的可行动上下文。`raw_context` 是经过字段脱敏、深度和
/// 体积限制的副本;它绝不能等同于 provider 的原始 stdin,也不能进 Debug 日志。
#[derive(Clone)]
pub struct AttentionContext {
    pub source: String,
    pub pane_id: Option<u64>,
    pub session_id: Option<String>,
    pub event_kind: AiHookKind,
    pub event_id: Option<String>,
    pub bridge_sequence: Option<u64>,
    /// provider 声明的 Unix 时间戳(毫秒);没有可靠字段时保持 None。
    pub occurred_at_ms: Option<u64>,
    /// slTerminal 完成 envelope 解析的 Unix 时间戳(毫秒)。
    pub received_at_ms: u64,
    pub cwd: Option<String>,
    pub project: Option<String>,
    pub git_branch: Option<String>,
    pub permission_or_tool: Option<String>,
    /// 事件抵达时 agent 声明的权限档位。`BypassPermissions` 时这条 attention
    /// 一定不是「等你批准」(那种会话不会来问),只可能是等你输入——UI 的文案
    /// 必须据此区分,否则用户会以为有个批准按钮在等他。
    pub permission_mode: Option<AiPermissionMode>,
    pub message: Option<String>,
    pub selection: Option<String>,
    pub raw_context: Option<String>,
}

impl std::fmt::Debug for AttentionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AttentionContext")
            .field("source", &self.source)
            .field("pane_id", &self.pane_id)
            .field("session_id", &self.session_id)
            .field("event_kind", &self.event_kind)
            .field("event_id", &self.event_id)
            .field("bridge_sequence", &self.bridge_sequence)
            .field("occurred_at_ms", &self.occurred_at_ms)
            .field("received_at_ms", &self.received_at_ms)
            .field("cwd", &self.cwd)
            .field("project", &self.project)
            .field("git_branch", &self.git_branch)
            .field("permission_or_tool", &self.permission_or_tool)
            .field("permission_mode", &self.permission_mode)
            .field("message", &self.message)
            .field(
                "selection_chars",
                &self.selection.as_ref().map(|s| s.chars().count()),
            )
            .field(
                "raw_context_bytes",
                &self.raw_context.as_ref().map(String::len),
            )
            .finish()
    }
}

impl AttentionContext {
    /// 通知正文只放定位和请求摘要;选区正文、raw context 从不进入系统通知。
    pub fn summary_for_pane(&self, pane_id: u64) -> String {
        let mut parts = Vec::with_capacity(4);
        if let Some(project) = self.project.as_deref().or(self.cwd.as_deref()) {
            parts.push(truncate(project, 120));
        }
        parts.push(format!("Pane {pane_id}"));
        if let Some(request) = self.permission_or_tool.as_deref() {
            parts.push(truncate(request, 100));
        }
        if let Some(message) = self.message.as_deref() {
            let message = truncate(message, MESSAGE_MAX_CHARS);
            if !parts.iter().any(|part| part == &message) {
                parts.push(message);
            }
        }
        if self.selection.is_some() {
            parts.push("selection context".to_owned());
        }
        parts.join(" · ")
    }
}

/// 一条类型化的 AI-CLI 生命周期事件,解析自一次管道连接。
#[derive(Debug, Clone)]
pub struct AiHookEvent {
    /// 旧版 claude 通知不带类型。它们可以在工作中途表示等待,但不能重开
    /// 一个空闲/已完成的回合。
    pub(super) legacy_attention: bool,
    pub(crate) codex_hooks: Option<CodexHookMode>,
    pub(crate) turn_id: Option<String>,
    /// compaction 在既有回合内触发 hook;它不是一条空闲边。
    pub(crate) session_compacted: bool,
    pub answer: Option<crate::assistant_answer::AssistantAnswer>,
    pub answer_cwd: Option<std::path::PathBuf>,
    /// 承载 CLI 的 pane(来自 `SLTERM_PANE_ID`);`None` 回退到焦点 pane
    /// (只在环境变量半路被剥掉时才发生)。
    pub pane: Option<u64>,
    /// AI CLI 身份,用作 toast 标题。
    pub source: String,
    pub kind: AiHookKind,
    pub turn_outcome: AiTurnOutcome,
    /// 事件携带的人类可读文本(claude 的通知文案、codex 的最后一条回答)。
    pub message: Option<String>,
    /// 原生可恢复会话身份。codex 用 transcript 的 thread UUID 或 legacy
    /// notify 的 `thread-id`,不是 hook 的 session/group id。
    pub session_id: Option<String>,
    /// 精确的原生文件与属主寿命,持久化会话恢复使用。
    pub session_file: Option<String>,
    pub bridge_instance: Option<String>,
    /// provider 给出的幂等身份;只在同一 source/session/pane 内去重。
    pub event_id: Option<String>,
    /// slTerminal bridge 盖的单调序号(不是 provider 原生顺序)。没有时只承诺
    /// 本地接收顺序。
    pub bridge_sequence: Option<u64>,
    pub occurred_at_ms: Option<u64>,
    pub received_at_ms: u64,
    /// 进程内严格单调的接收序号,用于稳定批处理;不冒充 provider 顺序。
    pub received_sequence: u64,
    /// 写入命名管道的那个进程的真实 pid,由内核回答
    /// (`GetNamedPipeClientProcessId`),不是载荷里自报的。路由的第二因子:
    /// 校验它是否真的跑在声明的 pane 进程树内。
    pub client_pid: Option<u32>,
    /// 沿 [`Self::client_pid`] 祖先链找到的最近 AI CLI 进程的 pid。这是 agent 的
    /// **进程身份**,用来区分嵌套子代理:`claude -p` 起的子代理有自己的 pid,
    /// 它的 session id 却是短命的——一旦被当成 pane 的会话身份,就会把真正活着
    /// 的那个顶掉。
    pub agent_pid: Option<u32>,
    /// agent 自己声明的权限档位。与 [`AiHookKind::NeedsAttention`] 是两件事:
    /// 前者是持续状态,后者是瞬时事件。
    pub permission_mode: Option<AiPermissionMode>,
    pub background_tasks: Option<AiBackgroundTasks>,
    pub attention: Option<AttentionContext>,
}

impl AiHookEvent {
    pub fn capabilities(&self) -> AiHookCapabilities {
        let mut capabilities = capabilities_for(&self.source);
        if let Some(mode) = self.codex_hooks {
            capabilities.lifecycle = true;
            capabilities.attention_events = mode == CodexHookMode::Full;
            capabilities.attention_context = mode == CodexHookMode::Full;
        }
        capabilities
    }

    pub fn active_background_tasks(&self) -> u32 {
        self.background_tasks.map_or(0, |tasks| tasks.active)
    }
}
