//! AI CLI 身份、可恢复性与屏幕状态识别。
//!
//! hook 是最高置信度的生命周期来源;本模块补齐 hook 覆盖不到的空档:包装
//! 进程身份、无 hook API 的 agent、以及粗粒度「回合完成」回调无法区分的交互
//! 式权限/提问屏幕。
//!
//! `AgentKind`、别名、命令解析与恢复／分支命令由 `slterm_agents` 单点提供。
//! 本模块保留仲裁三类型(`AgentStatus`/`AgentStatusSource`/`Detection`)、
//! 声明式 TOML 屏幕规则引擎(`detect`/`identify`/Manifest 编译)与结构判据
//! 子模块 `screen_context`。规则资源随仓(`slterm_agent_detection/*.toml`),
//! 用户覆盖落数据目录 `agent-detection/<slug>.toml`,mtime 节流 ≤2s。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant, SystemTime};

use regex::Regex;
use serde::Deserialize;

mod screen_context;

pub use slterm_agents::AgentKind;

/// hook 生命周期与兜底观察共用的 agent 语义状态。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Working,
    Blocked,
    Done,
    #[default]
    Unknown,
}

impl AgentStatus {
    /// 这个 pane 是否已有权威判定。`Unknown` 表示 hook 与屏幕规则都没认领它
    /// (普通 shell,或还没识别出 agent)。
    ///
    /// BEL 的兜底资格看这一位。响铃的含义是「有事发生」,不是「停下来等你输
    /// 入」:claude / codex 在**回合结束**时同样响铃(终端自己的通知文案就写
    /// 着「任务完成,等待输入」,两个概念当初就被焊在一起)。一旦让响铃去表示
    /// 等待输入,agent 每做完一件事都会显示成「在问你」——有权威判定时响铃
    /// 不得改写它,只在没有判定时兜底。
    pub const fn is_decided(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

/// 当前 pane 状态的取值来源。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AgentStatusSource {
    Hook,
    Screen,
    Process,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub agent: AgentKind,
    pub status: AgentStatus,
    /// TOML 规则 id,仲裁与可解释性共用;规则引擎负责匹配。
    pub rule_id: String,
}

// ─── 声明式屏幕规则引擎 ──────────────────────────────────────────────────
//
// 规则由 20 份厂商 TOML + `_shared.toml` 兜底组成,随仓为资源;用户覆盖写
// 数据目录的 `agent-detection/<slug>.toml`,按文件 mtime 节流(≤2s)热加载,
// 运行中调参不必重启。规则匹配是 AND 语义(`not` 取反、`any` 析取),同优先级
// 保留先声明者;本地规则排在共享规则之前。

/// 单厂商规则清单。
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Manifest {
    id: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    identity: Vec<Gate>,
    /// 继承规则的可选区域包装;规则自身的窗口保持不变。
    #[serde(default)]
    shared_rule_region: Option<String>,
    rules: Vec<Rule>,
}

/// `_shared.toml`:并入每份清单的兜底规则,无 agent 身份。
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct SharedManifest {
    rules: Vec<Rule>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Rule {
    id: String,
    state: RuleState,
    #[serde(default)]
    priority: i32,
    #[serde(default = "whole_recent")]
    region: String,
    #[serde(flatten)]
    gate: Gate,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
struct Gate {
    #[serde(default)]
    contains: Vec<String>,
    #[serde(default)]
    regex: Vec<String>,
    #[serde(default)]
    line_regex: Vec<String>,
    #[serde(default)]
    all: Vec<Gate>,
    #[serde(default)]
    any: Vec<Gate>,
    #[serde(default, rename = "not")]
    not_gate: Vec<Gate>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RuleState {
    Idle,
    Working,
    Blocked,
}

impl From<RuleState> for AgentStatus {
    fn from(state: RuleState) -> Self {
        match state {
            RuleState::Idle => Self::Idle,
            RuleState::Working => Self::Working,
            RuleState::Blocked => Self::Blocked,
        }
    }
}

#[derive(Debug)]
struct CompiledManifest {
    manifest: Manifest,
    identity: Vec<CompiledGate>,
    rules: Vec<CompiledGate>,
    override_mtime: Option<SystemTime>,
}

#[derive(Debug)]
struct CompiledGate {
    contains: Vec<String>,
    regex: Vec<Regex>,
    line_regex: Vec<Regex>,
    all: Vec<CompiledGate>,
    any: Vec<CompiledGate>,
    not_gate: Vec<CompiledGate>,
}

#[derive(Debug)]
struct Cache {
    manifests: HashMap<AgentKind, CompiledManifest>,
    last_override_scan: Instant,
}

/// 并入每份 manifest 的共享骨架(含用户覆盖)。
const SHARED: &str = include_str!("../../slterm_agent_detection/_shared.toml");

const BUNDLED: &[(AgentKind, &str)] = &[
    (
        AgentKind::Claude,
        include_str!("../../slterm_agent_detection/claude.toml"),
    ),
    (
        AgentKind::Codex,
        include_str!("../../slterm_agent_detection/codex.toml"),
    ),
    (
        AgentKind::Gemini,
        include_str!("../../slterm_agent_detection/gemini.toml"),
    ),
    (
        AgentKind::Cursor,
        include_str!("../../slterm_agent_detection/cursor.toml"),
    ),
    (
        AgentKind::OpenCode,
        include_str!("../../slterm_agent_detection/opencode.toml"),
    ),
    (
        AgentKind::Copilot,
        include_str!("../../slterm_agent_detection/copilot.toml"),
    ),
    (
        AgentKind::Grok,
        include_str!("../../slterm_agent_detection/grok.toml"),
    ),
    (
        AgentKind::Pi,
        include_str!("../../slterm_agent_detection/pi.toml"),
    ),
    (
        AgentKind::Amp,
        include_str!("../../slterm_agent_detection/amp.toml"),
    ),
    (
        AgentKind::Antigravity,
        include_str!("../../slterm_agent_detection/antigravity.toml"),
    ),
    (
        AgentKind::Cline,
        include_str!("../../slterm_agent_detection/cline.toml"),
    ),
    (
        AgentKind::Devin,
        include_str!("../../slterm_agent_detection/devin.toml"),
    ),
    (
        AgentKind::Droid,
        include_str!("../../slterm_agent_detection/droid.toml"),
    ),
    (
        AgentKind::Hermes,
        include_str!("../../slterm_agent_detection/hermes.toml"),
    ),
    (
        AgentKind::Kimi,
        include_str!("../../slterm_agent_detection/kimi.toml"),
    ),
    (
        AgentKind::Kiro,
        include_str!("../../slterm_agent_detection/kiro.toml"),
    ),
    (
        AgentKind::Kilo,
        include_str!("../../slterm_agent_detection/kilo.toml"),
    ),
    (
        AgentKind::Qoder,
        include_str!("../../slterm_agent_detection/qodercli.toml"),
    ),
    (
        AgentKind::Maki,
        include_str!("../../slterm_agent_detection/maki.toml"),
    ),
    (
        AgentKind::CodeBuddy,
        include_str!("../../slterm_agent_detection/codebuddy.toml"),
    ),
];

static CACHE: OnceLock<RwLock<Cache>> = OnceLock::new();

fn cache() -> &'static RwLock<Cache> {
    CACHE.get_or_init(|| {
        RwLock::new(Cache {
            manifests: build_cache(),
            last_override_scan: Instant::now(),
        })
    })
}

fn build_cache() -> HashMap<AgentKind, CompiledManifest> {
    BUNDLED
        .iter()
        .map(|(agent, source)| {
            let path = override_path(*agent);
            let disk_mtime = modified(&path);
            let bundled = compile_manifest(source, disk_mtime).unwrap_or_else(|error| {
                panic!("bundled {} agent rules are invalid: {error}", agent.slug())
            });
            let loaded = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| {
                    compile_manifest(&text, disk_mtime)
                        .map_err(|error| {
                            log::warn!("agent detection: ignored {}: {error}", path.display());
                        })
                        .ok()
                })
                .filter(|loaded| {
                    let matches = manifest_matches(&loaded.manifest, *agent);
                    if !matches {
                        log::warn!(
                            "agent detection: ignored {} because id {} does not match {}",
                            path.display(),
                            loaded.manifest.id,
                            agent.slug()
                        );
                    }
                    matches
                })
                .unwrap_or(bundled);
            (*agent, loaded)
        })
        .collect()
}

fn override_path(agent: AgentKind) -> PathBuf {
    crate::platform::dirs::data_dir()
        .join("agent-detection")
        .join(format!("{}.toml", agent.slug()))
}

fn modified(path: &std::path::Path) -> Option<SystemTime> {
    path.metadata().and_then(|meta| meta.modified()).ok()
}

fn refresh_overrides_if_needed() {
    let should_scan = cache()
        .read()
        .map(|guard| guard.last_override_scan.elapsed() >= Duration::from_secs(2))
        .unwrap_or(true);
    if !should_scan {
        return;
    }
    let Ok(mut guard) = cache().write() else {
        return;
    };
    if guard.last_override_scan.elapsed() < Duration::from_secs(2) {
        return;
    }
    guard.last_override_scan = Instant::now();
    let changed = BUNDLED.iter().any(|(agent, _)| {
        let path = override_path(*agent);
        let disk_mtime = modified(&path);
        guard
            .manifests
            .get(agent)
            .is_none_or(|loaded| loaded.override_mtime != disk_mtime)
    });
    if changed {
        guard.manifests = build_cache();
    }
}

/// 匹配一次实时屏幕快照。未命中返回 `None`:调用方保留更高置信度的 hook /
/// 进程状态,不凭空捏造 idle。
pub fn detect(program: &str, screen: &str) -> Option<Detection> {
    let agent = AgentKind::parse(program)?;
    refresh_overrides_if_needed();
    let guard = cache().read().ok()?;
    let loaded = guard.manifests.get(&agent)?;
    let attention_screen = screen_context::attention_region(agent, screen);
    let live_input = screen_context::has_live_input_controls(attention_screen);
    let mut best: Option<(&Rule, &CompiledGate)> = None;
    for (rule, gate) in loaded.manifest.rules.iter().zip(&loaded.rules) {
        let blocked = matches!(rule.state, RuleState::Blocked);
        // 结构闸:assistant 正文里的字不是事件,只有最近控制行说了算。
        if blocked && !live_input {
            continue;
        }
        let text = region(
            if blocked { attention_screen } else { screen },
            &rule.region,
        );
        if !gate.matches(text) {
            continue;
        }
        // 同优先级保留先声明者(本地规则在共享规则之前)。
        if best.is_none_or(|(previous, _)| rule.priority > previous.priority) {
            best = Some((rule, gate));
        }
    }
    best.map(|(rule, _)| Detection {
        agent,
        status: rule.state.into(),
        rule_id: rule.id.clone(),
    })
}

/// 从品牌专属终端 chrome 建立 agent 身份。
///
/// 与 [`detect`] 刻意分离:prompt 字形、中断提示这类通用状态 chrome 可以
/// 细化一个已知 agent,但绝不能凭空发明身份。宿主进程看不到 WSL 里的 Linux
/// CLI 时,这条兜底是唯一身份来源。
pub fn identify(screen: &str) -> Option<AgentKind> {
    refresh_overrides_if_needed();
    let guard = cache().read().ok()?;
    let mut found = None;
    for (agent, loaded) in &guard.manifests {
        if !loaded.identity.iter().any(|gate| gate.matches(screen)) {
            continue;
        }
        // 两个品牌同时命中即视为无法判定,不猜。
        if found.is_some() {
            return None;
        }
        found = Some(*agent);
    }
    found
}

fn compile_manifest(
    source: &str,
    override_mtime: Option<SystemTime>,
) -> Result<CompiledManifest, String> {
    let mut manifest: Manifest = toml::from_str(source).map_err(|error| error.to_string())?;
    if manifest.rules.is_empty() || manifest.rules.len() > 64 {
        return Err("manifest must contain 1..=64 rules".to_owned());
    }
    // 共享骨架并入每一份 manifest——bundled 与用户覆盖一视同仁,装了新 CLI
    // 或改了本地规则都自动带上中断提示判据。理由见 _shared.toml 顶部:
    // per-agent 的 working 规则各写各的,同一个 AND 陷阱在多份文件里重复出现。
    if manifest
        .shared_rule_region
        .as_deref()
        .is_some_and(|value| value != "after_last_prompt_row")
    {
        return Err("unsupported shared_rule_region wrapper".to_owned());
    }
    for mut rule in shared_rules().iter().cloned() {
        if let Some(wrapper) = &manifest.shared_rule_region {
            rule.region = format!("{wrapper}({})", rule.region);
        }
        manifest.rules.push(rule);
    }
    let rules = manifest
        .rules
        .iter()
        .map(|rule| {
            // 只校验本次新引入的包装;旧区域保持既有加载行为。
            if rule.region.starts_with("after_last_prompt_row") {
                let inner = rule
                    .region
                    .strip_prefix("after_last_prompt_row(")
                    .and_then(|value| value.strip_suffix(')'));
                if !inner.is_some_and(|value| {
                    value == "whole_recent"
                        || value == "after_last_horizontal_rule"
                        || region_count(value, "bottom_non_empty_lines").is_some_and(|n| n > 0)
                }) {
                    return Err(format!("invalid prompt region: {}", rule.region));
                }
            }
            compile_gate(&rule.gate)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let identity = manifest
        .identity
        .iter()
        .map(compile_gate)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CompiledManifest {
        manifest,
        identity,
        rules,
        override_mtime,
    })
}

/// 每份 agent manifest 继承的规则;只解析一次,见 `_shared.toml`。
fn shared_rules() -> &'static Vec<Rule> {
    static SHARED_RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    SHARED_RULES.get_or_init(|| {
        let parsed: SharedManifest = toml::from_str(SHARED)
            .unwrap_or_else(|error| panic!("bundled shared agent rules are invalid: {error}"));
        parsed.rules
    })
}

fn compile_gate(gate: &Gate) -> Result<CompiledGate, String> {
    let compile = |patterns: &[String]| {
        patterns
            .iter()
            .map(|pattern| Regex::new(pattern).map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()
    };
    Ok(CompiledGate {
        contains: gate
            .contains
            .iter()
            .map(|value| value.to_lowercase())
            .collect(),
        regex: compile(&gate.regex)?,
        line_regex: compile(&gate.line_regex)?,
        all: gate
            .all
            .iter()
            .map(compile_gate)
            .collect::<Result<_, _>>()?,
        any: gate
            .any
            .iter()
            .map(compile_gate)
            .collect::<Result<_, _>>()?,
        not_gate: gate
            .not_gate
            .iter()
            .map(compile_gate)
            .collect::<Result<_, _>>()?,
    })
}

impl CompiledGate {
    fn matches(&self, text: &str) -> bool {
        let lower = text.to_lowercase();
        self.contains.iter().all(|needle| lower.contains(needle))
            && self.regex.iter().all(|regex| regex.is_match(text))
            && self
                .line_regex
                .iter()
                .all(|regex| text.lines().any(|line| regex.is_match(line)))
            && self.all.iter().all(|gate| gate.matches(text))
            && (self.any.is_empty() || self.any.iter().any(|gate| gate.matches(text)))
            && !self.not_gate.iter().any(|gate| gate.matches(text))
    }
}

fn manifest_matches(manifest: &Manifest, agent: AgentKind) -> bool {
    manifest.id == agent.slug()
        || manifest
            .aliases
            .iter()
            .any(|alias| agent.aliases().contains(&alias.as_str()))
}

/// 保留最近一条空 prompt 行及其 footer(agy 1.1.27 实测形态)。空 prompt 可能
/// 带框;草稿必须有紧邻上下的横线才算边界。找不到可识别边界时保留原窗口,
/// 不静默丢弃真实的审批表单——这里刻意不猜任意输入文本。
fn from_last_empty_prompt(screen: &str) -> &str {
    let mut offset = 0;
    let mut start = 0;
    let mut lines = screen.split_inclusive('\n').peekable();
    let mut previous = "";
    while let Some(line) = lines.next() {
        let row = line.trim();
        let row = row
            .strip_prefix('│')
            .and_then(|row| row.strip_suffix('│'))
            .unwrap_or(row)
            .trim();
        let draft = row.strip_prefix(['›', '❯', '>']).is_some_and(|rest| {
            rest.starts_with(char::is_whitespace)
                && !rest
                    .trim_start()
                    .starts_with(|c: char| c.is_ascii_digit() || c == '[')
        });
        let horizontal_rule = |line: &str| {
            let row = line.trim();
            row.chars().count() >= 3 && row.chars().all(|c| c == '─')
        };
        if matches!(row, "›" | "❯" | ">")
            || (draft
                && horizontal_rule(previous)
                && lines.peek().is_some_and(|next| horizontal_rule(next)))
        {
            start = offset;
        }
        previous = line;
        offset += line.len();
    }
    &screen[start..]
}

fn whole_recent() -> String {
    "whole_recent".to_owned()
}

fn region<'a>(screen: &'a str, spec: &str) -> &'a str {
    if spec == "whole_recent" {
        return screen;
    }
    if let Some(inner) = spec
        .strip_prefix("after_last_prompt_row(")
        .and_then(|s| s.strip_suffix(')'))
    {
        return from_last_empty_prompt(region(screen, inner));
    }
    if let Some(count) = region_count(spec, "bottom_non_empty_lines") {
        let lines: Vec<&str> = screen.lines().collect();
        let Some(start) = lines
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, line)| !line.trim().is_empty())
            .take(count)
            .last()
            .map(|(index, _)| index)
        else {
            return "";
        };
        return slice_from_line(screen, &lines, start);
    }
    if spec == "after_last_horizontal_rule" {
        let mut offset = 0;
        let mut last = 0;
        for line in screen.lines() {
            offset = (offset + line.len() + 1).min(screen.len());
            let trimmed = line.trim();
            if trimmed
                .chars()
                .filter(|character| *character == '─')
                .count()
                >= 3
            {
                last = offset;
            }
        }
        return &screen[last..];
    }
    ""
}

fn region_count(spec: &str, name: &str) -> Option<usize> {
    spec.strip_prefix(name)?
        .strip_prefix('(')?
        .strip_suffix(')')?
        .parse()
        .ok()
}

fn slice_from_line<'a>(screen: &'a str, lines: &[&str], index: usize) -> &'a str {
    let offset = lines[..index.min(lines.len())]
        .iter()
        .map(|line| line.len() + 1)
        .sum::<usize>()
        .min(screen.len());
    &screen[offset..]
}

#[cfg(test)]
mod agent_status_tests {
    use super::*;

    /// 响铃只能在没人认领这个 pane 时兜底。claude / codex 在回合结束时也响铃,
    /// 一旦让响铃压过 hook / 屏幕规则的判定,「完成」会在 tab 上显示成「在问你」。
    #[test]
    fn bell_may_only_speak_for_panes_no_rule_has_claimed() {
        for decided in [
            AgentStatus::Idle,
            AgentStatus::Working,
            AgentStatus::Blocked,
            AgentStatus::Done,
        ] {
            assert!(decided.is_decided(), "{decided:?} 是权威判定,响铃不得改写");
        }
        assert!(
            !AgentStatus::Unknown.is_decided(),
            "没有判定时才轮到响铃兜底"
        );
    }
}

#[cfg(test)]
mod screen_rules_tests {
    use super::*;

    #[test]
    fn all_bundled_manifests_compile_and_declare_their_own_identity() {
        assert_eq!(BUNDLED.len(), 20, "20 厂商规则 + _shared.toml 兜底");
        for (agent, source) in BUNDLED {
            let compiled = compile_manifest(source, None)
                .unwrap_or_else(|error| panic!("{}: {error}", agent.slug()));
            assert!(
                manifest_matches(&compiled.manifest, *agent),
                "{} manifest id {} does not match",
                agent.slug(),
                compiled.manifest.id
            );
            assert!(
                compiled.manifest.rules.len() > shared_rules().len(),
                "{} 必须自带至少一条厂商规则",
                agent.slug()
            );
        }
    }

    #[test]
    fn codebuddy_screen_identity_requires_brand_and_live_footer() {
        // 转写自用户 2.150.0 Windows/WSL 截图,不是运行时 capture。只有观察到的
        // prompt/footer 才建立 idle 判定。
        let screen = "╭─ CodeBuddy Code v2.150.0 ─╮\nTips for getting started\n\
                      ────────────────\n> \n────────────────\n\
                      /agent-mode to switch · ? for shortcuts ← for agents";
        assert_eq!(identify(screen), Some(AgentKind::CodeBuddy));
        let idle = detect("codebuddy", screen).unwrap();
        assert_eq!(idle.status, AgentStatus::Idle);
        for text in [
            "CodeBuddy Code is a CLI.",
            "CodeBuddy Code v2.150.0\nuser@host:~$ ",
            "/agent-mode to switch · ? for shortcuts ← for agents",
            "> generic prompt\n? for shortcuts ← for agents",
            &format!("{screen}\nuser@host:~$ "),
        ] {
            assert_eq!(identify(text), None, "{text}");
        }
        assert!(detect("codebuddy", &format!("{screen}\nuser@host:~$ ")).is_none());
    }

    #[test]
    fn screen_rules_distinguish_working_blocked_and_idle() {
        let blocked = detect(
            "claude",
            "Do you want to proceed?\n❯ 1. Yes\n  2. No\nEsc to cancel",
        )
        .unwrap();
        assert_eq!(blocked.status, AgentStatus::Blocked);
        let working = detect("codex", "• Working (12s • esc to interrupt)").unwrap();
        assert_eq!(working.status, AgentStatus::Working);
        let idle = detect("claude", "────────────────\n❯ ").unwrap();
        assert_eq!(idle.status, AgentStatus::Idle);
    }

    #[test]
    fn review_regression_restored_codex_identity_requires_live_prompt_and_footer() {
        let live = "› Ask Codex to do anything\n\n  gpt-6-astra max · /mnt/d/temp_build/project · Saved conversation";
        assert_eq!(identify(live), Some(AgentKind::Codex));
        for text in [
            "› generic shell prompt",
            "The CLI says Ask Codex to do anything.",
            "› Ask Codex to do anything\nuser@host:~$ ",
            "› Ask Codex to do anything\ngpt-6-astra max · /project\nuser@host:~$ ",
        ] {
            assert_eq!(identify(text), None, "{text}");
        }
    }

    #[test]
    fn branded_screen_chrome_identifies_codex_without_a_visible_host_process() {
        let screen = "OpenAI Codex (v0.42.0)\n\n› Ask Codex to do anything";
        assert_eq!(identify(screen), Some(AgentKind::Codex));
        assert_eq!(identify("› generic shell prompt"), None);
    }

    /// 下面四段都是 2026-08-22 用 runtime `pane.read` 从真实窗口抓的原文,
    /// 也就是判错的现场。规则改动必须对着它们回归,不能对着记忆里的格式写。
    #[test]
    fn real_claude_working_chrome_reads_working() {
        // 正在干活的 pane。spinner 行用 ✶(U+2736)而不是盲文点阵,且与底部
        // 中断提示相隔 6 行——旧规则要求「盲文行 AND 中断词」且 region 只有
        // 6 行,两个条件同时落空,回合进行中因此被判成 idle,再连续两拍降级
        // 成「完成」蓝点。
        let screen = "  ⎿  Running…\n\
                      ✶ Moseying… (8m 42s · ↓ 18.3k tokens)\n\
                      \x20 ⎿  Tip: You haven't used the ui-ux-pro-max plugin in a while.\n\
                      \x20    with /plugin\n\
                      ────────────────────────────────────────\n\
                      ❯ \n\
                      ────────────────────────────────────────\n\
                      \x20 ⏵⏵ bypass permissions on (shift+tab to cycle) · esc to interrupt · ← for agents";
        let detection = detect("claude", screen).unwrap();
        assert_eq!(
            detection.status,
            AgentStatus::Working,
            "rule={}",
            detection.rule_id
        );
    }

    #[test]
    fn real_claude_idle_chrome_reads_idle() {
        // 回合已结束:没有中断提示,输入框空着。输入框可见本身不是判据——
        // 上面那段工作中的屏幕里 ❯ 同样在。
        let screen = "✻ Baked for 15m 24s\n\
                      ────────────────────────────────────────\n\
                      ❯ \n\
                      ────────────────────────────────────────\n\
                      \x20 ⏸ manual mode on · ? for shortcuts · ← for agents";
        let detection = detect("claude", screen).unwrap();
        assert_eq!(
            detection.status,
            AgentStatus::Idle,
            "rule={}",
            detection.rule_id
        );
    }

    #[test]
    fn real_claude_permission_form_reads_blocked() {
        // 真的停在权限框上。这段同时含「Esc to cancel」,共享 working 规则
        // 必须被它的 not 段拦住,否则「等你点头」会被当成「正在干活」。
        let screen = " List dist artifacts and query GitHub release assets\n\
                      \x20This command requires approval\n\
                      \x20Do you want to proceed?\n\
                      \x20❯ 1. Yes\n\
                      \x20  2. Yes, and don't ask again for: gh release *\n\
                      \x20  3. No\n\
                      \x20Esc to cancel · Tab to amend · ctrl+e to explain";
        let detection = detect("claude", screen).unwrap();
        assert_eq!(
            detection.status,
            AgentStatus::Blocked,
            "rule={}",
            detection.rule_id
        );
    }

    #[test]
    fn codex_queue_and_compaction_are_live_work_only_in_the_chrome_tail() {
        for chrome in [
            "• Working (12s · esc to interrupt)\n• Messages to be submitted after next tool call (press esc to interrupt and send immediately)\n  ↳ Continue the task",
            "• Compacting context (1m 41s · esc to interrupt)\n  └ Making room to continue.",
        ] {
            let frame = format!("{chrome}\n› Ask Codex to do anything\ngpt-6 max · /project");
            assert_eq!(
                detect("codex", &frame).unwrap().status,
                AgentStatus::Working
            );
            let stale = format!(
                "{chrome}\n{}› Ask Codex to do anything\ngpt-6 max · /project",
                "completed output\n".repeat(20)
            );
            assert_eq!(detect("codex", &stale).unwrap().status, AgentStatus::Idle);
        }
        let quoted = "The log said Compacting context; Messages to be submitted after next tool call\n› Ask Codex to do anything\ngpt-6 max · /project";
        assert_eq!(detect("codex", quoted).unwrap().status, AgentStatus::Idle);
    }

    #[test]
    fn real_codex_idle_chrome_reads_idle() {
        // codex 早已答完,屏幕上就是空闲输入框。这块屏幕当初根本没人去匹:
        // running_program 是 None,1 Hz 看门狗在入口就早退了,于是转圈长挂。
        let screen = "  代码级修复和带 gpui-shell 的编译、针对性测试已经通过。\n\
                      › Improve documentation in @filename\n\
                      \x20 gpt-5.6-sol xhigh · D:\\temp_build\\slTerminal";
        let detection = detect("codex", screen).unwrap();
        assert_eq!(
            detection.status,
            AgentStatus::Idle,
            "rule={}",
            detection.rule_id
        );
    }

    /// 下面六段是 2026-09-17 用 runtime `pane.read` 从 kimi 0.43.1 真实窗口
    /// 抓的原文(tmp/pane11*.json)的精简转写,也就是判错的现场。规则改动
    /// 必须对着它们回归,不能对着记忆里的格式写。
    #[test]
    fn real_kimi_working_chrome_reads_working() {
        // 回合进行中:输入框正上方挂月相 spinner 行(U+1F311–U+1F318 逐帧
        // 轮转,pane11-w2–w8),回合结束该行被重绘消失。输入框与状态行此时
        // 同样在场,idle 规则必须输给 working。
        let screen = "✨ 写一首关于秋天的四行短诗,直接输出诗,不要解释\n\
                      \x20 🌕 · Tip: /tasks to check progress and status for background tasks\n\
                      \x20╭─────────────────────────────────────╮\n\
                      \x20│ >                                    │\n\
                      \x20╰─────────────────────────────────────╯\n\
                      \x20K3 thinking: high  D:\\slTerminal\\tmp  feat/kimi-hooks-20260917\n\
                      \x20                                       context: 4% (33.4k/1M)";
        let detection = detect("kimi", screen).unwrap();
        assert_eq!(
            detection.status,
            AgentStatus::Working,
            "rule={}",
            detection.rule_id
        );
        assert_eq!(detection.rule_id, "moon_spinner_working");
    }

    #[test]
    fn real_kimi_idle_chrome_reads_idle() {
        // 回合已结束(pane11-idle.json):月相行消失,只剩历史回答、空输入框
        // 和状态行。状态行的「K3 thinking: high」是模型名 + 思考档位,
        // 「context: N%」是用量条,都不是 working 证据。
        let screen = "● 叶落知秋深,\n\
                      \x20  风凉染层林。\n\
                      \x20  雁过长天远,\n\
                      \x20  霜轻月色沉。\n\
                      \x20╭─────────────────────────────────────╮\n\
                      \x20│ >                                    │\n\
                      \x20╰─────────────────────────────────────╯\n\
                      \x20K3 thinking: high  D:\\slTerminal\\tmp  feat/kimi-hooks-20260917\n\
                      \x20                                       context: 4% (33.6k/1M)";
        let detection = detect("kimi", screen).unwrap();
        assert_eq!(
            detection.status,
            AgentStatus::Idle,
            "rule={}",
            detection.rule_id
        );
        assert_eq!(detection.rule_id, "prompt_idle");
    }

    #[test]
    fn real_kimi_approval_panel_reads_blocked() {
        // 真实审批面板(pane11-blocked.json):面板取代输入框固定在底部,上下
        // 各一条 ─ 横线;问句是大写 R 的「▶ Run this command?」,底行是
        // 「↑/↓ select · 1/2/3/4 choose · ↵ confirm」——旧规则写的小写
        // 问句、无斜杠 ↑↓ 和 esc cancel 都对不上这块面板。
        let screen = "● Running a command · $ echo hello-kimi-probe\n\
                      \x20  Press Ctrl+B to run in background\n\
                      \x20──────────────────────────────────────\n\
                      \x20  ▶ Run this command?\n\
                      \n\
                      \x20  cwd: D:/Documents/HTA/My Projects/slTerminal/tmp\n\
                      \x20  $ echo hello-kimi-probe\n\
                      \n\
                      \x20  ▶ 1. Approve once\n\
                      \x20    2. Approve for this session\n\
                      \x20    3. Reject\n\
                      \x20    4. Reject with feedback\n\
                      \n\
                      \x20  ↑/↓ select · 1/2/3/4 choose · ↵ confirm\n\
                      \x20──────────────────────────────────────\n\
                      \x20K3 thinking: high  D:\\slTerminal\\tmp  feat/kimi-hooks-20260917  ctrl+o expand\n\
                      \x20                                       context: 4% (33.7k/1M)";
        let detection = detect("kimi", screen).unwrap();
        assert_eq!(
            detection.status,
            AgentStatus::Blocked,
            "rule={}",
            detection.rule_id
        );
        assert_eq!(detection.rule_id, "approval_panel_blocked");
    }

    #[test]
    fn real_kimi_thinking_preview_reads_working() {
        // 回合内的另一种 working 画面(pane11-x5.json):没有月相行,盲文
        // spinner + 流式思考预览悬在输入框正上方。盲文行锚定底部后才是有效
        // 证据,这条把它钉住。
        let screen = "    /exit\n\
                      \n\
                      \x20 ⠙ thinking…\n\
                      \x20  User input weird: \"D:/Git/exit\n\
                      \x20  /exit\". Likely trying to exit\n\
                      \x20╭─────────────────────────────────────╮\n\
                      \x20│ >                                    │\n\
                      \x20╰─────────────────────────────────────╯\n\
                      \x20K3 thinking: high  D:\\slTerminal\\tmp  feat/kimi-hooks-20260917\n\
                      \x20                                       context: 4% (33.8k/1M)";
        let detection = detect("kimi", screen).unwrap();
        assert_eq!(
            detection.status,
            AgentStatus::Working,
            "rule={}",
            detection.rule_id
        );
        assert_eq!(detection.rule_id, "braille_thinking_working");
    }

    #[test]
    fn kimi_stale_braille_thinking_line_does_not_read_working() {
        // 回合结束后,「⠙ thinking…」预览被最终回答替换,只可能留在滚屏高处
        // (pane11.json 里旧思考内容距底部 10+ 非空行)。陈旧盲文行没有月相行
        // 陪同、又不在底部窗口内时,不得把屏幕拖回 Working。
        let screen = " ⠙ thinking…\n\
                      \x20  User input weird: \"D:/Git/exit\n\
                      \x20  /exit\". Likely trying to exit\n\
                      \n\
                      \x20● Bash 工具没有运行成功:用户拒绝了执行批准请求,没有产生输出。\n\
                      \n\
                      \x20╭─────────────────────────────────────╮\n\
                      \x20│ >                                    │\n\
                      \x20╰─────────────────────────────────────╯\n\
                      \x20K3 thinking: high  D:\\slTerminal\\tmp  feat/kimi-hooks-20260917\n\
                      \x20                                       context: 4% (33.8k/1M)";
        let detection = detect("kimi", screen).unwrap();
        assert_eq!(
            detection.status,
            AgentStatus::Idle,
            "rule={}",
            detection.rule_id
        );
        assert_eq!(detection.rule_id, "prompt_idle");
    }

    #[test]
    fn kimi_welcome_box_identifies_the_cli() {
        // 「Welcome to Kimi Code!」与「Send /help for help information.」两条
        // 顶行文字在 kimi.exe 0.43.1 二进制内核验;30 行 capture 窗口
        // (pane11.json)只留下框尾和下面的 Web UI 推广行,后者同样品牌独有。
        let welcome = "╭─────────────────────────────────────╮\n\
                       \x20│   Welcome to Kimi Code!                │\n\
                       \x20│   Send /help for help information.     │\n\
                       \x20╰─────────────────────────────────────╯";
        assert_eq!(identify(welcome), Some(AgentKind::Kimi));
        let tail = " │  Version:   0.43.1\n\
                    │  MCP:       1 connected\n\
                    ╰──────────────────────────────────────────────╯\n\
                    ✦ Try Kimi Code Web UI - clearer task progress and settings management";
        assert_eq!(identify(tail), Some(AgentKind::Kimi));
        assert_eq!(identify("the docs say welcome to kimi code, warmly"), None);
        assert_eq!(identify("user@host:~$ \n> generic prompt"), None);
    }

    #[test]
    fn shared_rules_light_up_agents_without_their_own_working_rule() {
        // 归一化的意义:中断提示对每个注册的 CLI 都点亮 working,新接的 CLI
        // 不必从零再写一遍 spinner 正则(cline 至今就没有 working 规则)。
        let screen = "some output\n────────\n> \n  esc to interrupt · ? for shortcuts";
        for agent in ["grok", "pi", "opencode", "gemini", "cline", "kilo", "kimi"] {
            let found =
                detect(agent, screen).unwrap_or_else(|| panic!("{agent} produced no detection"));
            assert_eq!(
                found.status,
                AgentStatus::Working,
                "{agent} rule={}",
                found.rule_id
            );
        }
    }

    #[test]
    fn shared_blocked_rules_outrank_shared_working_on_confirmation_forms() {
        // 确认框里几乎总有中断词;共享层必须自己把这一对张力解开,不能指望
        // 每个 CLI 都记得写 blocked 规则。
        let screen = "Run rm -rf /tmp/x ?\n  Do you want to proceed?\n  1. Yes\n  2. No\n\
                      esc to cancel · enter to confirm";
        for agent in ["grok", "pi", "cline", "kilo", "kimi"] {
            let found =
                detect(agent, screen).unwrap_or_else(|| panic!("{agent} produced no detection"));
            assert_eq!(
                found.status,
                AgentStatus::Blocked,
                "{agent} rule={}",
                found.rule_id
            );
        }
    }

    #[test]
    fn real_antigravity_working_chrome_reads_working() {
        let screen = "  ⠋ Thinking... (8s)\n\
                      ────────────────────────────────────────\n\
                      › \n\
                      ────────────────────────────────────────\n\
                      \x20 Plan mode: research & plan only (shift+tab to cycle) · Press esc to interrupt generation";
        let detection = detect("antigravity", screen).unwrap();
        assert_eq!(
            detection.status,
            AgentStatus::Working,
            "rule={}",
            detection.rule_id
        );
    }

    #[test]
    fn real_antigravity_idle_chrome_reads_idle() {
        let screen = "  I have completed the task and updated the configuration.\n\
                      ────────────────────────────────────────\n\
                      › \n\
                      ────────────────────────────────────────\n\
                      \x20 Plan mode: research & plan only (shift+tab to cycle) · ? for shortcuts";
        let detection = detect("antigravity", screen).unwrap();
        assert_eq!(
            detection.status,
            AgentStatus::Idle,
            "rule={}",
            detection.rule_id
        );
    }

    #[test]
    fn antigravity_old_scrollback_spinner_does_not_prevent_idle() {
        // 历史滚屏中留有上一轮的思考转圈符号;region 限制在底部后不能把已
        // 结束的回合误判为 Working。
        let screen = "  ⠋ Thinking... (from previous turn 5m ago)\n\
                      \x20 Result: success\n\
                      \x20 Line 1\n\
                      \x20 Line 2\n\
                      \x20 Line 3\n\
                      \x20 Line 4\n\
                      \x20 Line 5\n\
                      \x20 Line 6\n\
                      \x20 Line 7\n\
                      \x20 Line 8\n\
                      \x20 Line 9\n\
                      \x20 Line 10\n\
                      ────────────────────────────────────────\n\
                      › \n\
                      ────────────────────────────────────────\n\
                      \x20 Accept-edits mode: file edits auto-approved (shift+tab to cycle)";
        let detection = detect("antigravity", screen).unwrap();
        assert_eq!(
            detection.status,
            AgentStatus::Idle,
            "rule={}",
            detection.rule_id
        );
    }

    #[test]
    fn real_antigravity_permission_form_reads_blocked() {
        let screen = " requesting permission for:\n\
                      \x20 Run command: cargo test\n\
                      \x20 Do you want to proceed?\n\
                      \x20 › 1. Yes, allow\n\
                      \x20   2. No, deny access\n\
                      \x20 esc to cancel · tab amend";
        let detection = detect("antigravity", screen).unwrap();
        assert_eq!(
            detection.status,
            AgentStatus::Blocked,
            "rule={}",
            detection.rule_id
        );
    }

    #[test]
    fn antigravity_previous_confirmation_does_not_override_current_prompt() {
        for phrase in [
            "The tool requires approval before running (y/n).",
            "Use enter to confirm or tab to amend the command.",
            "Do you want to proceed?",
            "waiting for permission",
            "requesting permission for: cargo test\n[y] Yes\nesc to cancel · enter to confirm",
        ] {
            for (activity, footer, expected) in [
                ("", "? for shortcuts", AgentStatus::Idle),
                ("", "esc to interrupt", AgentStatus::Working),
                (
                    "⠋ Thinking... (8s)\n",
                    "esc to interrupt",
                    AgentStatus::Working,
                ),
            ] {
                let screen = format!("{phrase}\n{activity}────────\n› \n────────\n{footer}");
                let detection = detect("antigravity-cli", &screen).unwrap();
                assert_eq!(
                    detection.status, expected,
                    "{screen}: rule={}",
                    detection.rule_id
                );
            }
        }
    }

    #[test]
    fn antigravity_working_footer_excludes_quoted_form_hints() {
        for footer in [
            "↑/↓ Navigate · enter Select · esc Skip",
            "↑/↓ Navigate · tab Amend · f full diff",
        ] {
            let working = format!(
                "The footer reads:\n{footer}\nand that means a choice is pending.\n⠋ Thinking... (2s)\nesc to interrupt"
            );
            assert_eq!(
                detect("agy", &working).unwrap().status,
                AgentStatus::Working
            );
            let approval = format!("Previous output: esc to interrupt\n{footer}\nesc to cancel");
            assert_eq!(
                detect("agy", &approval).unwrap().status,
                AgentStatus::Blocked
            );
        }
    }

    #[test]
    fn prompt_drafts_need_input_box_context() {
        for row in [
            "> fix the tests please",
            "│ › fix the tests │",
            "› Ask anything…",
        ] {
            let screen =
                format!("Do you want to proceed?\n────────\n{row}\n────────\n? for shortcuts");
            assert_eq!(detect("agy", &screen).unwrap().status, AgentStatus::Idle);
        }
        for row in [
            "> Yes, I trust this folder",
            "> Allow once",
            "> [ ] 中文",
            "> 1) Yes",
            "> 1. Yes",
        ] {
            let screen = format!("Do you want to proceed?\n{row}\nesc to cancel");
            assert_eq!(from_last_empty_prompt(&screen), screen);
        }
    }

    #[test]
    fn malformed_prompt_regions_are_rejected_at_load() {
        for region in [
            "after_last_prompt_row()",
            "after_last_prompt_row(",
            "after_last_prompt_row(unknown)",
            "after_last_prompt_row(bottom_non_empty_lines(0))",
        ] {
            let source = format!(
                "id = 'custom'\n[[rules]]\nid = 'test'\nstate = 'idle'\nregion = '{region}'\ncontains = ['ready']"
            );
            assert!(
                compile_manifest(&source, None)
                    .unwrap_err()
                    .contains("invalid prompt region")
            );
        }
    }

    #[test]
    fn prompt_regions_are_explicit_and_preserve_unknown_input() {
        let screen = "Do you want to proceed?\n│ ›   │\n? for shortcuts";
        assert_eq!(
            region(screen, "after_last_prompt_row(bottom_non_empty_lines(12))"),
            "│ ›   │\n? for shortcuts"
        );
        for row in ["› Ask anything…", "> 1. Yes", "› unfinished draft"] {
            let screen = format!("Do you want to proceed?\n{row}");
            assert_eq!(from_last_empty_prompt(&screen), screen);
        }
        let source = "id = 'custom'\nshared_rule_region = 'after_last_prompt_row'\n\
            [[rules]]\nid = 'shared_custom'\nstate = 'blocked'\n\
            region = 'whole_recent'\ncontains = ['approval']";
        let compiled = compile_manifest(source, None).unwrap();
        assert_eq!(compiled.manifest.rules[0].region, "whole_recent");
        assert_eq!(
            compiled.manifest.rules[1].region,
            "after_last_prompt_row(bottom_non_empty_lines(1))"
        );
        assert!(
            compile_manifest(&source.replace("after_last_prompt_row", "unknown"), None).is_err()
        );
    }

    #[test]
    fn antigravity_long_forms_and_extended_footers_still_block() {
        for (footer, rule) in [
            (
                "↑/↓ Navigate · enter Select · esc Skip",
                "question_selection",
            ),
            ("↑/↓ Navigate · tab Amend · f full diff", "file_review"),
        ] {
            let screen = format!(
                "Question 1/1: Choose\n{}\n{footer} · ctrl+g expand\nesc to cancel",
                "long wrapped option\n".repeat(20)
            );
            let found = detect("agy", &screen).unwrap();
            assert_eq!(found.status, AgentStatus::Blocked);
            assert_eq!(found.rule_id, rule);
            // 回答里被完整引用的 footer 也只是历史:当前输入 prompt 可见即空闲。
            for prompt in [">", "│ ›   │"] {
                let answered = format!("{screen}\n{prompt}\n? for shortcuts");
                assert_eq!(detect("agy", &answered).unwrap().status, AgentStatus::Idle);
            }
        }
    }

    #[test]
    fn antigravity_real_command_permission_requires_attention() {
        let screen = "Command\n────────\nRequesting permission for:\n\
            cmd /c echo agy-command-probe\nDo you want to proceed?\n\
            > 1. Yes\n\
            2. Yes, and always allow in this conversation for commands that start with 'cmd'\n\
            3. Yes, and always allow for commands that start with 'cmd' (Persist to settings.json)\n\
            4. No\n↑/↓ Navigate · tab Amend · ctrl+g edit/expand command\n\
            esc to cancel                         Gemini 3.8 Flash · high";
        assert_eq!(detect("agy", screen).unwrap().status, AgentStatus::Blocked);
    }

    #[test]
    fn antigravity_real_file_creation_review_requires_attention() {
        let review = "Create file\n────────\nagy-permission-check.txt +1\n\
            1 + permission probe\nAllow creation of this file?\n\
            > 1. Yes, allow creation\n  2. No, deny creation\n\
            ↑/↓ Navigate · tab Amend · f full diff\n\
            esc to cancel                         Gemini 3.8 Flash · high";
        let detection = detect("agy", review).unwrap();
        assert_eq!(detection.status, AgentStatus::Blocked);
        assert_eq!(detection.rule_id, "file_review");
        let edit = review
            .replace("Allow creation of this file?", "Accept this file edit?")
            .replace("Yes, allow creation", "Yes, accept this change")
            .replace("No, deny creation", "No, reject this change");
        assert_eq!(detect("agy", &edit).unwrap().status, AgentStatus::Blocked);
        let accepted = format!("{review}\nCreated file.\n────────\n>\n────────\n? for shortcuts");
        assert_eq!(detect("agy", &accepted).unwrap().status, AgentStatus::Idle);
        let automatic = "Created agy-permission-check.txt\n────────\n>\n────────\n? for shortcuts";
        assert_eq!(detect("agy", automatic).unwrap().status, AgentStatus::Idle);
    }

    #[test]
    fn antigravity_real_ask_question_overrides_cancel_working_hint() {
        // 从 agy 1.1.27 的 Windows ConPTY 抓取(选择中文之前)。
        let question = "Question\n────────\n\
            Question 1/1: 请选择接下来交流使用的语言 / Please select the language to use:\n\
            > 1. 中文\n  2. English\n  3. Write-in...\n\
            ↑/↓ Navigate · enter Select · esc Skip\n\
            esc to cancel                         Gemini 3.8 Flash · high";
        let detection = detect("agy", question).unwrap();
        assert_eq!(detection.status, AgentStatus::Blocked);
        assert_eq!(detection.rule_id, "question_selection");

        let answered = format!("{question}\n已选择中文。\n────────\n>\n────────\n? for shortcuts");
        assert_eq!(detect("agy", &answered).unwrap().status, AgentStatus::Idle);
        let prose = "The shortcuts are ↑/↓ Navigate · enter Select · esc Skip\n>\n? for shortcuts";
        assert_eq!(detect("agy", prose).unwrap().status, AgentStatus::Idle);
    }

    #[test]
    fn antigravity_active_confirmation_after_prompt_still_blocks() {
        let screen = "› \nrequesting permission for: cargo test\n› 1. Yes, allow\n\
                      2. No\nesc to cancel · enter to confirm";
        let detection = detect("antigravity-cli", screen).unwrap();
        assert_eq!(
            detection.status,
            AgentStatus::Blocked,
            "rule={}",
            detection.rule_id
        );
    }

    #[test]
    fn antigravity_prose_mentioning_approval_while_idle_reads_idle() {
        // 回答正文中提及 "needs approval for ...",但此时已完成并显示空闲提示符,
        // 不得误判为 Blocked。
        for phrase in [
            "needs approval for",
            "requesting approval for",
            "requesting permission for:",
        ] {
            let screen = format!(
                "│ The tool {phrase} operations outside the workspace.\n\
                 │ Please confirm your settings if you plan to enable this.\n\
                 ────────────────────────────────────────\n\
                 › \n\
                 ────────────────────────────────────────\n\
                 \x20 Plan mode: research & plan only (shift+tab to cycle) · ? for shortcuts"
            );
            let detection = detect("antigravity", &screen).unwrap();
            assert_eq!(
                detection.status,
                AgentStatus::Idle,
                "{phrase}: rule={}",
                detection.rule_id
            );
        }
    }
}
