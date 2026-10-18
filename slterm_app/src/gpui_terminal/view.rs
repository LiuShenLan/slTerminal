//! 终端视图:持有会话、处理输入与 IME、驱动重绘。

mod ai_fix;
mod broadcast;
mod completion;
mod cwd_report;
pub(super) mod layout;
pub(super) mod pointer;
/// 04 篇 M7.5:Runtime 写命令在 UI owner 上的终端侧执行入口。
pub(super) mod runtime_control;
pub(super) mod startup;
pub(super) mod typography;

pub use broadcast::TerminalInput;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Bounds, ClipboardItem, Context, EventEmitter, FocusHandle, Focusable, Font, FontStyle,
    FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, Render, ScrollWheelEvent,
    Size, Styled as _, UTF16Selection, Window, div, point, px,
};

use std::rc::Rc;
use std::sync::Arc;

use slterm_terminal::event::{Event as TermEvent, Notify as _, WindowSize};
use slterm_terminal::event_loop::Msg;
use slterm_terminal::grid::Dimensions as _;
use slterm_terminal::grid::Scroll;
use slterm_terminal::index::{Column, Line, Point as TermPoint, Side};
use slterm_terminal::render::{CellMetrics, TerminalViewport, ViewportTracker};
use slterm_terminal::selection::{Selection, SelectionType};
use slterm_terminal::term::TermMode;
use vte::ansi::CursorStyle;

use slterm_split::PaneId;

use super::colors::Palette;
use super::copy_feedback::CopyFeedback;
use super::element::TerminalElement;
use super::keymap;
use super::mouse_protocol;
use super::session::{self, TerminalSession};
use super::{KEY_CONTEXT, TerminalBackTab, TerminalTab};
use crate::app_shell::config::Settings;
use crate::i18n::Message;
use typography::{M3_CELL_WIDTH_MODE, M3_FONT_OFFSET, mono_font};

/// 拖选越过网格上/下边界后的自动回滚:tick 间隔与「每远离 20px 加一行」的
/// 速度分档取自旧壳常量(GPUI 的 `Pixels` 本就是逻辑像素,这里直接是逻辑口径)。
pub(super) const SELECTION_SCROLL_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(15);
const SELECTION_SCROLL_STEP: f32 = 20.0;

/// 一次自动回滚该滚几行:指针在网格上方为正(往回滚历史)、下方为负,网格
/// 内为 0。贴边即 1 行,之后每远离 `SELECTION_SCROLL_STEP` 像素加一档。
///
/// `max_lines` 是每 tick 的上限:GPUI 在 Windows 上按下即 `SetCapture`,
/// 指针甩到屏幕外时 y 可以是很大的负数,不封顶就会「一抖到底」。
fn selection_scroll_lines(y: f32, top: f32, bottom: f32, max_lines: i32) -> i32 {
    let step = SELECTION_SCROLL_STEP.max(1.0);
    let max = max_lines.max(1);
    if y < top {
        (1 + ((top - y) / step) as i32).min(max)
    } else if y >= bottom {
        -((1 + ((y - bottom) / step) as i32).min(max))
    } else {
        0
    }
}

fn cursor_blink_allowed(
    terminal_blinking: bool,
    vi_mode: bool,
    show_cursor: bool,
    ime_preedit: bool,
    window_active: bool,
    pane_focused: bool,
) -> bool {
    terminal_blinking && (vi_mode || show_cursor) && !ime_preedit && window_active && pane_focused
}

fn restart_cursor_blink_phase(cursor_visible: &mut bool, cursor_blink_epoch: &mut u64) -> u64 {
    *cursor_visible = true;
    *cursor_blink_epoch = cursor_blink_epoch.wrapping_add(1);
    *cursor_blink_epoch
}

/// 终端视图对宿主(Workspace)暴露的状态变化。M3 白集(13 篇改造节);
/// FocusRequested/RequestClose/Bell/Notification/Progress/菜单等臂归各自
/// 分篇回填。
pub enum TerminalViewEvent {
    /// OSC 标题/cwd 变化,宿主应刷新 Tab 标题。
    TitleChanged,
    /// 会话结束(子进程退出或 PTY 故障),只发一次。
    Exited,
    /// 用户语义输入。宿主可按 tab 的广播状态扇出(广播本体归 M5);
    /// 接收 pane 必须重新编码,不能复用发送方已受终端 mode 影响的字节。
    #[allow(dead_code)] // 载荷消费 = M5 广播扇出;M3 宿主只收事件名。
    UserInput(TerminalInput),
    /// Enter 提交了非空命令行:宿主把 agent 生命周期记到本 pane。
    #[allow(dead_code)] // 载荷消费 = 后续 agent 判定;当前只作事件名。
    CommandSubmitted(String),
    /// OSC 133;C:命令开始。宿主要求 AgentActivity 进入 submitted。
    CommandStarted,
    /// OSC 133;D:命令结束。宿主收尾 AgentActivity 并发布快照。
    CommandFinished,
}

/// 会话种类。M3 只剩本地 shell:上游 `Ssh` 变体消亡(13 篇,06 篇按理想
/// 终态重设计时另立)。
///
/// `shell` 是 Tab 创建时已经裁定好的启动命令。不能只传 cwd 再在
/// `TerminalView::new` 里读取「此刻的默认 Shell」,否则恢复混合工作区时
/// 每个 Tab 都会被重新解释成同一个当前默认值。
pub enum TerminalLaunch {
    Local {
        cwd: Option<std::path::PathBuf>,
        shell: Option<slterm_terminal::tty::Shell>,
        /// 展示用 shell 名(欢迎/标题);M3 不参与 PTY 启动决策。
        shell_name: Option<String>,
        /// 命令来自 `-e` 显式指定(用户 CLI 直给):core 跳过 shell 白名单
        /// 并做裸名 PATH 路径化。settings 键等配置文件来源必须 `false`
        /// (SEC-01 防配置注入偷换 shell)。
        explicit_command: bool,
    },
}

pub struct TerminalView {
    pub pane_id: PaneId,
    pub session: Option<TerminalSession>,
    /// PTY 创建时冻结的环境/cwd 快照(04 篇 `pane.exec` 唯一复用面)。
    pub(crate) exec_context: Option<crate::runtime_exec::PaneExecContext>,
    pub focus_handle: FocusHandle,
    pub font: Font,
    pub font_bold: Font,
    pub font_italic: Font,
    pub font_bold_italic: Font,
    pub font_size: Pixels,
    #[allow(dead_code)] // 设置热应用换字体时重读(06 篇);M3 构造期消费后闲置。
    pub ligatures: bool,
    /// 单元格偏移取物理像素语义(对齐 crossfont 合同),在 GPUI shaped
    /// 实际字形之后施加。M3 恒 (0,0)(06 篇设置键回填)。
    font_offset_x: f32,
    font_offset_y: f32,
    /// 主题行高倍率:`None` 保留 shaped 字体度量;`Some` 作用于逻辑字号。
    line_height_multiplier: Option<f32>,
    pub palette: Arc<Palette>,
    /// 每 pane 一份的公式覆盖层状态(08 篇 M9.5;终端仍是真值)。
    pub(crate) math_overlay: super::math_overlay::MathOverlay,
    /// 补全关闭/缓存状态(08 篇 M9.5):键、抑制行、弹窗请求位与候选。
    pub(crate) suggest: crate::suggest::SuggestState,
    /// 按键镜像的当前输入行与光标(终端行编辑器不暴露 buffer)。
    pub(crate) suggest_line: String,
    pub(crate) suggest_cursor: usize,
    /// 行镜像是否仍与屏幕输入一致。光标移动/历史召回后无法证明,Enter
    /// 宁可不记也不记脏(对齐上游 screen_line 优先纪律)。
    pub(crate) suggest_line_trusted: bool,
    /// 壳侧按 pane 持有的来源缓存句柄(不复制缓存本体)。
    pub(crate) completion_session: Option<Rc<crate::completion::Session>>,
    /// 在飞的候选计算;丢弃即取消。
    pub(super) suggestion_task: Option<completion::PendingCompletion>,
    pub(crate) completion_style: crate::suggest::CompletionStyle,
    /// AI 内联 ghost 开关。
    pub(crate) ghost_enabled: bool,
    /// Enter 时捕获的提交行(OSC 133;D 触发判定与程序身份共用)。
    pub(crate) last_committed: String,
    /// runtime run 记账(C1):OSC 133 两条边推进 phase 并产出 outcome。
    pub(crate) active_run: Option<crate::runtime_api::RuntimePaneRun>,
    pub(crate) last_run: Option<crate::runtime_api::RuntimeRunOutcome>,
    /// 每 pane 一条 AI 修复建议的生命周期。
    pub(crate) ai_fix: Option<crate::ai_assistant::AiFixState>,
    pub(super) ai_fix_gate: crate::ai_assistant::TriggerGate,
    /// 测试注入的助手配置(生产读独立配置文件,不碰真实数据目录)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) ai_config_override: Option<crate::ai_assistant::AssistantConfig>,
    pub marked_text: Option<String>,
    pub ime_bounds: Bounds<Pixels>,
    pub title: String,
    /// shell 集成 `SLTERM|cwd|branch|program` 标题上报 / OSC 7 的工作目录;
    /// tab 标签的第一优先来源。
    pub cwd: String,
    /// 标题协议里的分支字段(05/04 篇 runtime 快照消费);OSC 7 不动它。
    pub branch: String,
    /// 标题协议里的前台程序名;runtime 快照的 `running_program` 来源。
    pub running_program: Option<String>,
    pub(super) error: Option<String>,
    pub(super) exited: Option<String>,
    /// 冷恢复待提交的 AI CLI resume 命令;会话恢复注入的唯一落点。
    pub(super) pending_agent_command: Option<String>,
    pub(super) origin: Point<Pixels>,
    pub(super) cell_width: Pixels,
    pub(super) line_height: Pixels,
    pub(super) cols: usize,
    pub(super) rows: usize,
    /// 子进程已被告知的几何。`cols`/`rows` 是屏幕上正在渲染的视口,交互式
    /// 拖拽期间它每帧跟手、本地网格随之 reflow,而这里停在最后一次真正
    /// 下发给 ConPTY 的边界上。判断「要不要通知子进程」只能拿这个字段比,
    /// 拿 `cols`/`rows` 比会把每一帧都当成新几何。
    pub(super) window_size: WindowSize,
    /// 启动稳定闸:开窗 resize 是异步落地的,首帧可能还是旧尺寸。闸门
    /// 关着时不向 Term/ConPTY 下发布局,直到布局命中 spawn 网格(零下发
    /// 收口)或宽限期超时(小屏收拢等真实差异,放行一次性纠正)。没有
    /// 这道闸,ConPTY 会在 shell 首屏输出中途被 目标→旧→目标 来回重排
    /// (shrink 重排具破坏性),PSReadLine 的坐标缓存随之错位——表现为
    /// 打字回显落到陈旧行。
    pub(super) grid_synced: bool,
    pub(super) spawn_at: std::time::Instant,
    pub(super) viewports: ViewportTracker,
    /// 等尾沿 resize 提交的最终可视视口。只等 PTY 那一半——网格已在
    /// 观测当帧 reflow 过。
    pub(super) pending_resize: Option<TerminalViewport>,
    /// 下一次网格变化是「结构性」的,子进程立即知道、不进尾沿去抖。
    ///
    /// 旧壳在分屏创建/关闭、tab 新建这类一次性结构变化上同步下发
    /// grid + PTY;若混进尾沿去抖路径,分屏后有 150ms 窗口期:子进程仍按
    /// 旧宽度输出,那些字节按旧几何进网格,等提交时本地 reflow 与 conhost
    /// rewrap 各折一套行数——提示符与回显从此错开几行。
    pub(super) structural_resize: bool,
    /// 新布局观测胜出时作废旧 settle 定时器。
    pub(super) resize_epoch: u64,
    pub(super) scroll_px: f32,
    pub(super) selecting: bool,
    /// 拖选越界后的自动回滚 tick 世代:停止或重开一轮就 +1,让在途的旧
    /// 定时器链自行失效(同 `cursor_blink_epoch` 的模式)。
    pub(super) selection_scroll_epoch: u64,
    /// 是否已有一条自动回滚定时器链在跑——每次 move 都开一条会叠出 N 倍速。
    pub(super) selection_scroll_active: bool,
    /// 选中即复制;关闭时复制交给右键路径。
    pub(super) copy_on_select: bool,
    /// §5.2 复制反馈时限状态机(时限实现单点)。
    pub(super) copy_feedback: CopyFeedback,
    /// 鼠标模式下最后上报的单元格:move 事件按「进入新单元格」去重。
    pub(super) last_report_point: Option<TermPoint>,
    /// GPUI 没有旧壳 scheduler 的 blink 事件,视图自己只维护可见相位;
    /// 光标是否允许闪烁仍由共享 `Term::cursor_style()` 裁定。
    cursor_visible: bool,
    cursor_blink_epoch: u64,
    /// OS 窗口前台状态与 pane 内焦点是两层独立条件。缓存它们是因为 blink
    /// timer 回调没有 `Window`,状态变化由 GPUI observer 立即重启相位。
    pub(super) cursor_window_active: bool,
    pub(super) cursor_pane_focused: bool,
    /// Workspace 呈现可见性独立于键盘/窗口焦点:隐藏 tab 的输出不触发
    /// 重绘无效化(见 `process_event` 的 Wakeup 臂)。
    pub(super) output_visible: bool,
    _cursor_blink_subscriptions: [gpui::Subscription; 3],
    /// 最近一次下发的默认光标样式。只在它真正变化时清理 shell 的
    /// DECSCUSR/DEC mode 12 覆盖,避免无关设置变更打断 vim 等程序光标。
    #[allow(dead_code)] // 下发消费点随 M6 主题/设置联动点亮;M3 恒默认。
    pub(super) default_cursor_style: CursorStyle,
}

impl TerminalView {
    /// 默认画布(旧壳 `Dimensions { columns: 116, lines: 30 }` 同源):
    /// 窗口按基准字号定形,spawn 网格再按当前缩放字号反推。
    pub const DEFAULT_GRID_COLUMNS: u16 = 116;
    pub const DEFAULT_GRID_LINES: u16 = 30;

    /// 启动稳定闸的宽限期:等待开窗 resize 落地到布局的最长时间。超时
    /// 即认定差异是真实的(如小屏收拢),按当前视口放行纠正。
    const STARTUP_GRID_GRACE: std::time::Duration = std::time::Duration::from_millis(400);

    /// 尾沿去抖窗口:视口静默这么久才向 Term/ConPTY 提交一次 resize。
    /// 见 `set_layout` 内的合同注释(conhost rewrap 漂移取证)。
    const RESIZE_SETTLE_DELAY: std::time::Duration = std::time::Duration::from_millis(150);
    /// 与旧壳 `Cursor::default().blink_interval` 保持一致。
    const CURSOR_BLINK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(750);

    /// 设置热应用:字体四族/字号/连字/选中复制/调色板即时生效。
    ///
    /// 字体度量变化属结构性几何变化(`mark_structural_resize`),下一帧
    /// 网格观测直接同步 Term 与 ConPTY,不进尾沿去抖;仅调色板/复制语义
    /// 变化只重绘。
    pub fn apply_settings(&mut self, settings: &Settings, cx: &mut Context<'_, Self>) {
        let font_changed = self.font.family.as_ref() != settings.font_family.as_str()
            || (self.font_size.as_f32() - settings.font_size_px).abs() > f32::EPSILON
            || self.ligatures != settings.ligatures;
        self.font = mono_font(
            &settings.font_family,
            FontWeight::NORMAL,
            FontStyle::Normal,
            settings.ligatures,
        );
        self.font_bold = mono_font(
            &settings.font_bold_family,
            FontWeight::BOLD,
            FontStyle::Normal,
            settings.ligatures,
        );
        self.font_italic = mono_font(
            &settings.font_italic_family,
            FontWeight::NORMAL,
            FontStyle::Italic,
            settings.ligatures,
        );
        self.font_bold_italic = mono_font(
            &settings.font_bold_italic_family,
            FontWeight::BOLD,
            FontStyle::Italic,
            settings.ligatures,
        );
        self.font_size = px(settings.font_size_px);
        self.ligatures = settings.ligatures;
        self.copy_on_select = settings.copy_on_select;
        self.completion_style = settings.completion_style;
        self.ghost_enabled = settings.ghost;
        self.palette = Arc::new(settings.palette.clone());
        if font_changed {
            self.mark_structural_resize();
        }
        cx.notify();
    }

    pub(super) fn cell_width_for_advance(&self, raw_width: f32, scale: f32) -> Pixels {
        typography::effective_cell_width(raw_width, M3_CELL_WIDTH_MODE, scale, self.font_offset_x)
    }

    pub(super) fn line_height_for_metrics(&self, natural_height: f32, scale: f32) -> Pixels {
        typography::line_height_for_view(
            self.font_size,
            self.line_height_multiplier,
            natural_height,
            self.font_offset_y,
            scale,
        )
    }

    /// 首帧前的当前单元格度量(与 element prepaint 同一公式:shape "M" 的
    /// advance / ascent / descent + 配置 offset)。让 spawn 网格与首帧布局
    /// 一致,避免启动即触发一次 ConPTY resize(DA 探询与回显竞态的温床)。
    pub fn cell_metrics(window: &Window, cx: &App) -> (Pixels, Pixels) {
        typography::cell_metrics(window, cx)
    }

    /// 窗口定形使用配置基准字号,不使用持久化缩放;缩放后的字号只影响
    /// 最终能容纳的行列数。否则放大一级就会把 116 列全部加到窗宽上。
    pub fn startup_cell_metrics(window: &Window, cx: &App) -> (Pixels, Pixels) {
        typography::startup_cell_metrics(window, cx)
    }

    pub(crate) fn startup_cell_metrics_at_scale(scale: f32, cx: &App) -> (Pixels, Pixels) {
        typography::startup_cell_metrics_at_scale(scale, cx)
    }

    /// 终端 mode 的一次锁读取(无会话 = 空 mode,所有协议判定落空)。
    pub(super) fn term_mode(&self) -> TermMode {
        self.session
            .as_ref()
            .map(|s| *s.term.lock().mode())
            .unwrap_or_default()
    }

    /// 事件白集(M3,13 篇锚定):每条臂的归篇见 13 篇改造节;新增事件
    /// 消费须先查归篇表,不在此自由发挥。
    pub(super) fn process_event(&mut self, event: TermEvent, cx: &mut Context<'_, Self>) {
        let cwd_changed = cwd_report::apply(&mut self.cwd, &event);
        // 标题协议的第二/三段(branch/program)与 cwd 同一信封;OSC 7 只
        // 携带目录,不得清掉这两项。
        cwd_report::apply_title_metadata(&mut self.branch, &mut self.running_program, &event);
        match event {
            TermEvent::Wakeup => {
                if self.output_visible {
                    cx.notify();
                }
            }
            TermEvent::MouseCursorDirty => {
                cx.notify();
            }
            TermEvent::CursorBlinkingChange => self.restart_cursor_blink(cx),
            TermEvent::Title(title) => {
                // `SLTERM|cwd|branch` 标题协议:cwd 已由上面的 cwd_report
                // 摘走;协议串不是窗口标题,更不是 tab 名。branch 字段的
                // 消费归 05 篇 chrome,M3 不存。
                if cwd_changed {
                    crate::completion::record_directory(&self.cwd);
                }
                if !title.starts_with("SLTERM|") {
                    self.title = title;
                }
                cx.emit(TerminalViewEvent::TitleChanged);
                cx.notify();
            }
            TermEvent::ResetTitle => {
                self.title = String::from("shell");
                cx.emit(TerminalViewEvent::TitleChanged);
                cx.notify();
            }
            TermEvent::PtyWrite(text) => self.write_bytes(text.into_bytes()),
            // OSC 52 写剪贴板(core Osc52 默认 OnlyCopy:选择器 c 已在
            // 引擎侧放行)。
            TermEvent::ClipboardStore(_, text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
            TermEvent::ClipboardLoad(_, formatter) => {
                let text = cx
                    .read_from_clipboard()
                    .and_then(|item| item.text())
                    .unwrap_or_default();
                self.write_bytes(formatter(&text).into_bytes());
            }
            TermEvent::ColorRequest(index, formatter) => {
                let reply = self.session.as_ref().map(|session| {
                    let term = session.term.lock();
                    self.palette.query_reply(index, term.colors())
                });
                if let Some(rgb) = reply {
                    self.write_bytes(formatter(rgb).into_bytes());
                }
            }
            TermEvent::TextAreaSizeRequest(formatter) => {
                self.write_bytes(formatter(self.window_size).into_bytes());
            }
            TermEvent::ChildExit(code) => {
                self.mark_exited(format!("进程已退出({code:?})"), cx);
            }
            TermEvent::PtyFailure(reason) => {
                self.mark_exited(format!("PTY 故障:{reason}"), cx);
            }
            TermEvent::Exit => {
                self.mark_exited(String::from("会话已结束"), cx);
            }
            TermEvent::CwdReport(_) => {
                // 标准 OSC 7 / 9;9 的目录上报:cwd 变了必须发 TitleChanged,
                // `on_terminal_event` 才会 `cx.notify()` 重绘 tab 标题。
                if cwd_changed {
                    // 目录 frecency 只记本机终端真实上报的目录。
                    crate::completion::record_directory(&self.cwd);
                    cx.emit(TerminalViewEvent::TitleChanged);
                    cx.notify();
                }
            }
            // 命令边界(清单 C1):OSC 133;C/D 接 active_run/last_run 记账与
            // AI assistant 触发链;agent 生命周期由宿主经事件收尾。屏幕采样
            // 仍走 workspace 的 1 Hz 看门狗(输出洪流下每字节 detect 不成立)。
            TermEvent::CommandStart => {
                if !self.last_committed.trim().is_empty() {
                    if self.running_program.is_none() {
                        self.running_program =
                            crate::suggest::extract_program(&self.last_committed);
                    }
                    if let Some(run) = &mut self.active_run
                        && run.phase == crate::runtime_api::RuntimeRunPhase::Submitted
                    {
                        run.phase = crate::runtime_api::RuntimeRunPhase::Started;
                    }
                    cx.emit(TerminalViewEvent::CommandStarted);
                }
            }
            TermEvent::CommandDone { exit_code } => {
                let program = self.running_program.take();
                if let Some(run) = self.active_run.take() {
                    self.last_run = Some(crate::runtime_api::RuntimeRunOutcome::command_done(
                        run, exit_code,
                    ));
                }
                cx.emit(TerminalViewEvent::CommandFinished);
                // 裸第三方 133;D(exit_code=None)静默。
                if let Some(code) = exit_code {
                    self.maybe_request_ai_fix(code, program.as_deref(), cx);
                }
            }
            TermEvent::Notify(_) => {}
            // 视觉闪烁与声音归 09 篇。
            TermEvent::Bell => {}
            TermEvent::Progress { .. } => {}
            TermEvent::InlineImage { .. } => {}
            TermEvent::UserVar { .. } => {}
        }
    }

    /// `Exited` 只对宿主发一次;重复的退出信号(ChildExit 之后必然跟 Exit)
    /// 只更新文案。
    fn mark_exited(&mut self, message: String, cx: &mut Context<'_, Self>) {
        if self.exited.is_none() {
            self.exited = Some(message);
            cx.emit(TerminalViewEvent::Exited);
        }
        cx.notify();
    }

    /// 让 EventLoop 退出并回收 ConPTY/子进程。幂等:重复调用只会得到
    /// 发送失败;线程 join 兜底在会话持有的 EventLoopHandle Drop 上。
    pub fn shutdown(&self) {
        if let Some(session) = &self.session {
            let _ = session.notifier.0.send(Msg::Shutdown);
        }
    }

    fn write_bytes(&self, bytes: Vec<u8>) {
        if let Some(session) = &self.session {
            session.notifier.notify(bytes);
        }
    }

    /// 恢复注入:按保存的 [`crate::session::AgentSession`] 重建 resume 命令。
    ///
    /// 异形 id / 无 resume 语法的来源由 `AgentSession::resume_command` 拒收
    /// (SEC 注入面),此时只恢复布局、不上屏任何命令。命令在 pane spawn 后
    /// 立即写进 ConPTY 输入缓冲:shell 启动期输入由控制台缓冲保留,不依赖
    /// 额外的 prompt 检测状态机。
    pub(crate) fn restore_agent(
        &mut self,
        agent: &crate::session::AgentSession,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(command) = agent.resume_command() else {
            log::debug!(
                "agent restore: no safe resume command for source {}",
                agent.source
            );
            return;
        };
        self.pending_agent_command = Some(command);
        self.flush_pending_agent_command(cx);
    }

    /// 提交待恢复命令(幂等;无会话/会话已退出时保留待提交态)。
    fn flush_pending_agent_command(&mut self, cx: &mut Context<'_, Self>) {
        if self.pending_agent_command.is_none() || self.exited.is_some() || self.session.is_none() {
            return;
        }
        let command = self
            .pending_agent_command
            .take()
            .expect("pending command checked above");
        self.write_input(format!("{command}\r").into_bytes(), cx);
    }

    /// 测试专用:待提交的恢复命令(无 = 已提交或未构造)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn pending_agent_command(&self) -> Option<&str> {
        self.pending_agent_command.as_deref()
    }

    /// 文件树拖放落点(07 篇 M8.4 交界 43):路径文本直写 PTY。
    ///
    /// 不走 bracketed-paste 哨兵:载荷是「路径 + 尾随空格」的粘贴形态,
    /// 没有换行,也不该被当成一段粘贴文本(哨兵会改变部分 shell 的行编辑
    /// 行为)。引用/控制字符规则在 `side_panel::drop_text_for_path` 单点。
    pub(crate) fn drop_path_text(&mut self, bytes: Vec<u8>, cx: &mut Context<'_, Self>) {
        self.write_input(bytes, cx);
    }

    /// 输入后回到底部并请求重绘。
    fn write_input(&mut self, bytes: Vec<u8>, cx: &mut Context<'_, Self>) {
        if let Some(session) = &self.session {
            {
                let mut term = session.term.lock();
                // 原生提示符旁路:编辑控制键不使已盖章的提示符失效
                // (preserves_native_prompt 判定在入队前,与解析器同锁序)。
                let preserves_prompt = super::event_mailbox::preserves_native_prompt(&bytes);
                session.native_prompt.observe_input(
                    session.native_prompt.snapshot().input_epoch.wrapping_add(1),
                    preserves_prompt,
                );
                term.scroll_display(Scroll::Bottom);
                term.selection = None;
            }
            session.notifier.notify(bytes);
        }
        self.restart_cursor_blink(cx);
        cx.notify();
    }

    fn cursor_should_blink(&self) -> bool {
        self.session.as_ref().is_some_and(|session| {
            let term = session.term.lock();
            let mode = term.mode();
            cursor_blink_allowed(
                term.cursor_style().blinking,
                mode.contains(TermMode::VI),
                mode.contains(TermMode::SHOW_CURSOR),
                self.marked_text.is_some(),
                self.cursor_window_active,
                self.cursor_pane_focused,
            )
        })
    }

    pub(super) fn restart_cursor_blink(&mut self, cx: &mut Context<'_, Self>) {
        let epoch =
            restart_cursor_blink_phase(&mut self.cursor_visible, &mut self.cursor_blink_epoch);
        self.schedule_cursor_blink_tick(epoch, cx);
        cx.notify();
    }

    fn schedule_cursor_blink_tick(&self, epoch: u64, cx: &mut Context<'_, Self>) {
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            executor.timer(Self::CURSOR_BLINK_INTERVAL).await;
            let _ = this.update(cx, |view, cx| {
                if view.cursor_blink_epoch != epoch {
                    return;
                }
                if !view.cursor_should_blink() {
                    if !view.cursor_visible {
                        view.cursor_visible = true;
                        cx.notify();
                    }
                    return;
                }
                view.cursor_visible = !view.cursor_visible;
                view.schedule_cursor_blink_tick(epoch, cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    pub fn grid_rows(&self) -> usize {
        self.rows
    }

    pub fn grid_cols(&self) -> usize {
        self.cols
    }

    /// 屏幕观察窗口:终端网格末尾最多 24 行(与屏幕规则注意力区窗口同源)。
    ///
    /// 消费点 = workspace 的 1 Hz agent 看门狗(M4.5);刻意不挂在每个
    /// `Wakeup` 上——规则匹配随输出洪流放大,屏幕证据只需要拍帧级频率。
    /// 无会话/零几何(尚未布局)返回 `None`。
    pub(crate) fn agent_screen_sample(&self) -> Option<String> {
        let session = self.session.as_ref()?;
        let term = session.term.lock();
        let lines = term.screen_lines();
        let columns = term.columns();
        if lines == 0 || columns == 0 {
            return None;
        }
        let start = TermPoint::new(Line((lines - lines.min(24)) as i32), Column(0));
        let end = TermPoint::new(Line(lines as i32 - 1), Column(columns.saturating_sub(1)));
        Some(term.bounds_to_string(start, end))
    }

    /// 复制选区并返回是否实际写入剪贴板。`notify` 表示显式复制
    /// (Ctrl+Shift+C/右键):成功后清除选区并给 §5.2 对勾反馈;
    /// copy_on_select 刻意静默且保留选区,避免鼠标抬手后立刻失去视觉反馈。
    pub fn copy_selection(
        &mut self,
        notify: bool,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(session) = &self.session else {
            return false;
        };
        let text = session.term.lock().selection_to_string();
        let Some(text) = text.filter(|text| !text.is_empty()) else {
            return false;
        };

        cx.write_to_clipboard(ClipboardItem::new_string(text));
        if notify {
            // 原始按键可能紧接着再次到来;先清除选区,下一次 Copy 才能按
            // 「未处理」传播回终端,而不是重复复制并再次弹反馈。
            session.term.lock().selection = None;
            self.copy_feedback
                .mark_copied(cx, |view| &mut view.copy_feedback);
            cx.notify();
        }
        true
    }

    /// 粘贴入口。M3 无多行确认(归 03/09 篇):文本直入;bracketed-paste
    /// 哨兵与注入过滤在 `paste_now_impl`。
    pub fn paste(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        self.paste_now_impl(&text, true, cx);
    }

    fn paste_now_impl(&mut self, text: &str, emit: bool, cx: &mut Context<'_, Self>) {
        let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
        let bytes = if self.term_mode().contains(TermMode::BRACKETED_PASTE) {
            let mut b = b"\x1b[200~".to_vec();
            // 过滤粘贴内容里的收尾哨兵,防止注入截断。
            b.extend_from_slice(normalized.replace("\x1b[201~", "").as_bytes());
            b.extend_from_slice(b"\x1b[201~");
            b
        } else {
            normalized.into_bytes()
        };
        if emit {
            self.note_text_input(text, cx);
            self.write_user_text(text.to_owned(), true, bytes, cx);
        } else {
            self.write_input(bytes, cx);
        }
    }

    /// M3 键位分发:白名单按 13 篇改造节 3。分发判定唯一点 = 本函数的
    /// 快捷键层;编码器(`keymap`)零特例——Ctrl+C 不在快捷键层出现,
    /// ctrl_char('c')=0x03 经编码器恒透传(定位约束 7)。
    fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.exited.is_some() {
            return;
        }
        // IME 组合和 Windows 原生窗口快捷键不编码、不拦截。GPUI Windows
        // 在 `stop_propagation` 后会跳过 `TranslateMessage`/`DispatchMessage`,
        // 必须保留输入法组合、窗口关闭和系统菜单的默认处理。
        if self.marked_text.is_some() || keymap::is_native_window_shortcut(&event.keystroke) {
            return;
        }
        let ks = &event.keystroke;
        let mods = &ks.modifiers;

        // 终端惯例快捷键优先于编码器。
        if mods.control && mods.shift {
            match ks.key.as_str() {
                "c" => {
                    // D13-1 默认语义:无选区也 stop_propagation 静默吞——
                    // 复制键永不入 PTY;负反馈/透传中断的权衡见 13 篇待沉淀。
                    self.copy_selection(true, window, cx);
                    cx.stop_propagation();
                    return;
                }
                "v" => {
                    self.paste(window, cx);
                    cx.stop_propagation();
                    return;
                }
                // 应用级快捷键(新建/关闭 Tab、分屏、缩放等):不编码、不拦截,
                // 让按键冒泡到 workspace 的 action 绑定。少写一个键位,症状就是
                // 「快捷键没反应」外加一串 CSI 序列被打进 PTY——下面的回滚翻页
                // 分支只吃**不带 ctrl** 的 shift+pageup,所以 ctrl+shift 这一组
                // 必须在这里放行。
                "t" | "w" | "b" | "p" | "f" | "d" | "s" | "g" | "o" | "enter" | "pageup"
                | "pagedown" => return,
                _ => {}
            }
        }
        // 08 篇 M9.5:AI 建议条键语义优先于补全(Esc 先撤条),再是补全
        // 导航/接受。两条路径消费按键时都不进入编码器。
        if self.handle_ai_fix_key(ks, cx) {
            cx.stop_propagation();
            return;
        }
        if self.handle_completion_key(&ks.key, mods, cx) {
            cx.stop_propagation();
            return;
        }
        // 分屏焦点导航(ctrl+alt+方向)同样冒泡给 workspace,不编码成
        // CSI 1;7 序列。
        if mods.control && mods.alt && matches!(ks.key.as_str(), "left" | "right" | "up" | "down") {
            return;
        }
        // 工作区切 tab(ctrl+tab / ctrl+shift+tab):不编码进 PTY。
        if mods.control && !mods.alt && ks.key.as_str() == "tab" {
            return;
        }

        let mode = self.term_mode();

        // 回滚快捷键(仅主屏):Shift+PageUp/PageDown 翻页、Shift+Home/End
        // 到顶/到底;备用屏下不拦截,交给编码器把带修饰的键序发给应用
        // (less 自己处理 Shift+PageUp)。
        if mods.shift && !mods.control && !mods.alt && !mode.contains(TermMode::ALT_SCREEN) {
            let scroll = match ks.key.as_str() {
                "pageup" => Some(Scroll::PageUp),
                "pagedown" => Some(Scroll::PageDown),
                "home" => Some(Scroll::Top),
                "end" => Some(Scroll::Bottom),
                _ => None,
            };
            if let Some(scroll) = scroll {
                if let Some(session) = &self.session {
                    session.term.lock().scroll_display(scroll);
                }
                cx.notify();
                cx.stop_propagation();
                return;
            }
        }

        // 编码器路径:Some(bytes) → 写 PTY + 上报语义输入;None(纯文本键)
        // 落 IME 管道(`replace_text_in_range` → write_user_text)照旧。
        // `running_program` 恒 None——agent 检测归 03 篇,签名留位。
        if let Some(bytes) = keymap::encode_for_program(ks, &mode, None) {
            keymap::trace_enter(ks, &mode, &bytes);
            // 按键镜像只更新输入快照;候选重算在后台,不占按键热路径。
            self.note_text_key(ks, cx);
            self.write_user_key(ks.clone(), bytes, cx);
            cx.stop_propagation();
        }
    }

    /// 截住组件 Root 的 Tab 焦点遍历后回灌既有终端按键路径:合成的 tab
    /// 按键仍走 `on_key_down` 全链(同一编码器,无第二套按键语义)。
    fn dispatch_terminal_tab(
        &mut self,
        shift: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let modifiers = gpui::Modifiers {
            shift,
            ..Default::default()
        };
        self.on_key_down(
            &KeyDownEvent {
                keystroke: gpui::Keystroke {
                    modifiers,
                    key: "tab".to_owned(),
                    key_char: None,
                },
                is_held: false,
                prefer_character_input: false,
            },
            window,
            cx,
        );
    }

    fn on_terminal_tab(
        &mut self,
        _: &TerminalTab,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.dispatch_terminal_tab(false, window, cx);
    }

    fn on_terminal_back_tab(
        &mut self,
        _: &TerminalBackTab,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.dispatch_terminal_tab(true, window, cx);
    }

    fn marked_utf16_len(&self) -> usize {
        self.marked_text
            .as_deref()
            .map(|t| t.encode_utf16().count())
            .unwrap_or(0)
    }
}

impl EventEmitter<TerminalViewEvent> for TerminalView {}

impl Drop for TerminalView {
    fn drop(&mut self) {
        // 兜底清理:无论视图以何种路径销毁(关 Tab、关窗口、退出应用),
        // 都保证 PTY 线程和子进程被回收。
        self.shutdown();
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        _range: std::ops::Range<usize>,
        _adjusted_range: &mut Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<UTF16Selection> {
        // 终端没有文本框语义的选区;返回折叠选区让 IME 把组合文本插在光标处。
        let len = self.marked_utf16_len();
        Some(UTF16Selection {
            range: len..len,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<std::ops::Range<usize>> {
        self.marked_text
            .as_ref()
            .map(|_| 0..self.marked_utf16_len())
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.marked_text.take().is_some() {
            self.restart_cursor_blink(cx);
        } else {
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let had_marked_text = self.marked_text.take().is_some();
        if !text.is_empty() && self.exited.is_none() {
            self.note_text_input(text, cx);
            self.write_user_text(text.to_owned(), false, text.as_bytes().to_vec(), cx);
        } else if had_marked_text {
            self.restart_cursor_blink(cx);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<std::ops::Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let marked_text = if new_text.is_empty() {
            None
        } else {
            Some(new_text.to_string())
        };
        if self.marked_text == marked_text {
            cx.notify();
        } else {
            self.marked_text = marked_text;
            self.restart_cursor_blink(cx);
        }
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: std::ops::Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(self.ime_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<usize> {
        None
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mut root = div()
            .id("slterm-terminal")
            .debug_selector(|| "slterm-terminal".to_owned())
            .key_context(KEY_CONTEXT)
            .size_full()
            .relative()
            .overflow_hidden()
            // 卡内呼吸边距,对齐上游网格 reserve 的换算值:上下 8、左右 12。
            // 网格因此不贴边;padding 区点击由 grid_point 的钳制兜底。
            // 注意:startup_geometry 的 chrome 垂直量(34+16+16+2)与本
            // padding 同链——改这里必须同步改 windowing/startup_geometry.rs。
            .py(px(8.0))
            .px(px(12.0))
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_terminal_tab))
            .on_action(cx.listener(Self::on_terminal_back_tab))
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_middle_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            // 拖出终端范围松手的兜底(含拖到窗口外):见 on_mouse_up_out。
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up_out))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_right_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_middle_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll));

        if let Some(error) = &self.error {
            root = root.child(div().p_4().child(error.clone()));
        } else {
            root = root.child(TerminalElement::new(cx.entity()));
            // 补全弹窗(08 篇 M9.5):与 ghost 同源候选;渲染只读状态。
            if !self.suggest.completion_items.is_empty() {
                let mut list = div()
                    .id("slterm-completion-popup")
                    .debug_selector(|| "slterm-completion-popup".to_owned())
                    .absolute()
                    .bottom(px(64.0))
                    .left_2()
                    .w(px(420.0))
                    .max_h(px(220.0))
                    .overflow_hidden()
                    .rounded_md()
                    .border_1()
                    .border_color(self.palette.dim_foreground.opacity(0.6))
                    .bg(self.palette.background.opacity(0.96))
                    .text_sm()
                    .text_color(self.palette.foreground)
                    .flex()
                    .flex_col();
                for (index, item) in self.suggest.completion_items.iter().take(8).enumerate() {
                    let selected = self.suggest.completion_selected == Some(index);
                    list = list.child(
                        div()
                            .px_2()
                            .py_1()
                            .when(selected, |row| row.bg(self.palette.selection))
                            .child(format!(
                                "{}  [{}]",
                                item.label,
                                completion_kind_tag(item.kind)
                            )),
                    );
                }
                root = root.child(list);
            }
            // AI 建议条(08 篇 M9.5):Pending 显 analyzing;Ready 是图标 +
            // 命令 + 解释 + 键位提示;一律只贴不执行。
            if let Some(bar) = self.ai_fix_bar() {
                let language = cx
                    .try_global::<Settings>()
                    .map(|settings| settings.language)
                    .unwrap_or(crate::i18n::UiLanguage::EnUs);
                let text = if bar.pending {
                    language.text(Message::TerminalAiAnalyzing).to_owned()
                } else {
                    let mut text =
                        format!("{} {}", if bar.danger { "⚠" } else { "✦" }, bar.command);
                    if !bar.explain.is_empty() {
                        text.push_str("  ·  ");
                        text.push_str(&bar.explain);
                    }
                    text.push_str("  ·  ");
                    text.push_str(language.text(Message::TerminalAiFixHint));
                    text
                };
                root = root.child(
                    div()
                        .id("slterm-ai-fix-bar")
                        .debug_selector(|| "slterm-ai-fix-bar".to_owned())
                        .absolute()
                        .bottom_2()
                        .left_2()
                        .right_2()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .border_1()
                        .border_color(self.palette.dim_foreground.opacity(0.6))
                        .bg(self.palette.background.opacity(0.96))
                        .text_sm()
                        .text_color(self.palette.foreground)
                        .child(text),
                );
            }
            if let Some(exited) = &self.exited {
                root = root.child(
                    div()
                        .absolute()
                        .bottom_2()
                        .left_2()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(gpui::opaque_grey(0.2, 0.9))
                        .text_sm()
                        .child(exited.clone()),
                );
            }
        }
        root
    }
}

/// 弹窗候选来源 tag(与 M9.1 呈现条目四态一一对应)。
fn completion_kind_tag(kind: crate::suggest::SltermCompletionKind) -> &'static str {
    match kind {
        crate::suggest::SltermCompletionKind::History => "history",
        crate::suggest::SltermCompletionKind::Command => "command",
        crate::suggest::SltermCompletionKind::Dir => "dir",
        crate::suggest::SltermCompletionKind::File => "file",
    }
}

#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "gpui-test-support"))]
mod feedback_tests;
#[cfg(all(test, feature = "gpui-test-support"))]
mod key_tests;
