//! AI assistant 触发链(08 篇 M9.2):命令失败后的判定纯函数族、独立配置
//! 文件与建议条数据形态。
//!
//! 这条路径跑在每次命令失败上,不能吵:先过便宜门链(冷却 → 开关 →
//! 规则表),任何一门不过安静返回。隐私闸门三件套的第一件就是默认关闭。
//! 请求体出境前,输出尾部必须经 [`redact_secrets`] 打码;字符上限由抓取侧
//! 执行(24 行 / 2000 字符)。
//!
//! 本步只落可单测的纯逻辑;建议条渲染、请求发送与网格抓取归 M9.5。
//! M9.3 合流:config_guard 自愈失败经 [`evaluate_hook_failure`] 进入同一
//! 闸门,D08-2 裁定的默认关(`enabled=false`)同样适用于失败边。

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use slterm_split::PaneId;
use zeroize::Zeroizing;

/// 配置文件名(数据目录内)。独立于设置主文件:设置页整存整取那份文件,
/// 手工加进去的陌生键会在下次保存时被抹掉;助手配置独立成文件(D08-1 已裁)。
const CONFIG_FILE: &str = "slterm_assistant.txt";

/// 模型给出的一条修复建议。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AiFix {
    pub command: String,
    pub explain: String,
    pub danger: bool,
}

/// 每 pane 一条建议的生命周期。`seq` 防过期响应落位。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AiFixState {
    /// 请求在飞;条显 analyzing。
    Pending { seq: u64 },
    /// 建议已就绪,等 Ctrl+. / Esc / typing。
    Ready { seq: u64, fix: AiFix },
}

impl AiFixState {
    pub(crate) fn seq(&self) -> u64 {
        match self {
            Self::Pending { seq } | Self::Ready { seq, .. } => *seq,
        }
    }
}

/// 独立配置文件的解析结果;文件缺失 = 全默认 = 关闭。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssistantConfig {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub ignored_exit_codes: Vec<i32>,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-5.4-mini".into(),
            ignored_exit_codes: Vec::new(),
        }
    }
}

/// 读侧三态结果(与设置主文件同纪律):`corrupted = true` 表示数据来自
/// `.bak` 或默认回退,损坏原件已隔离。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssistantConfigLoad {
    pub config: AssistantConfig,
    pub corrupted: bool,
}

impl AssistantConfig {
    /// 每次失败命令读一次(亚毫秒,不缓存)。读错误不打断终端工作流,
    /// 但必须留日志证据。
    pub(crate) fn load() -> Self {
        match Self::load_at(&config_path()) {
            Ok(loaded) => {
                if loaded.corrupted {
                    log::warn!("assistant: 配置文件损坏,已按隔离/备份策略回退");
                }
                loaded.config
            }
            Err(error) => {
                log::warn!("assistant: 配置读取失败: {error}");
                Self::default()
            }
        }
    }

    /// 显式路径读入口(测试注入):缺失 → 默认;损坏 → 隔离原件后尝试
    /// `.bak` 重建;其它 IO 错误向上传播。
    pub(crate) fn load_at(path: &Path) -> io::Result<AssistantConfigLoad> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(AssistantConfigLoad {
                    config: Self::default(),
                    corrupted: false,
                });
            }
            Err(error) => {
                return Err(io::Error::new(
                    error.kind(),
                    format!("读取助手配置失败: {error}"),
                ));
            }
        };
        let Some(text) = String::from_utf8(bytes).ok() else {
            return Ok(recover_corrupted(path));
        };
        match Self::parse(&text) {
            Some(config) => Ok(AssistantConfigLoad {
                config,
                corrupted: false,
            }),
            None => Ok(recover_corrupted(path)),
        }
    }

    /// 保存到生产路径(原子写 + `.bak`)。
    pub(crate) fn save(&self) -> io::Result<()> {
        self.save_at(&config_path())
    }

    /// 显式路径写入口:先备份旧文件,再临时文件 + 原子替换。
    pub(crate) fn save_at(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if path.is_file()
            && let Err(error) = std::fs::copy(path, backup_path(path))
        {
            // 备份不阻断主写入(与设置主文件同纪律)。
            log::warn!("assistant 配置 .bak 备份失败: {error}");
        }
        crate::atomic_file::write(path, self.render().as_bytes())
    }

    /// 行式 `key=value` 解析。空文件视为默认;非空但没有任何赋值行、
    /// 或存在无法切分的非注释行 → 损坏。
    fn parse(text: &str) -> Option<Self> {
        if text.trim().is_empty() {
            return Some(Self::default());
        }
        let mut config = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('=')?;
            let value = value.trim();
            match key.trim() {
                "enabled" => config.enabled = matches!(value, "on" | "true" | "1"),
                "base_url" if !value.is_empty() => config.base_url = value.to_owned(),
                "model" if !value.is_empty() => config.model = value.to_owned(),
                "ignored_exit_codes" => {
                    config.ignored_exit_codes = value
                        .split(',')
                        .filter_map(|code| code.trim().parse().ok())
                        .collect();
                }
                _ => {}
            }
        }
        // 只有注释/空白行也是合法文件(回默认),不算损坏。
        Some(config)
    }

    /// 单行值清洗:去控制字符与首尾空白,防一个值把配置文件写出多行。
    fn render(&self) -> String {
        let codes = self
            .ignored_exit_codes
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "enabled={}\nbase_url={}\nmodel={}\nignored_exit_codes={codes}\n",
            if self.enabled { "on" } else { "off" },
            single_line(&self.base_url),
            single_line(&self.model),
        )
    }
}

fn single_line(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>()
        .trim()
        .to_owned()
}

pub(crate) fn config_path() -> PathBuf {
    crate::platform::dirs::data_dir().join(CONFIG_FILE)
}

/// 同名 `.bak` 路径(与设置主文件同形态)。
fn backup_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(".bak");
    path.with_file_name(name)
}

/// 隔离损坏文件:`slterm_assistant.txt` → `slterm_assistant.corrupted.txt`
/// (copy + remove)。返回隔离文件路径。
fn quarantine(path: &Path) -> Option<PathBuf> {
    let stem = path.file_stem()?.to_str()?;
    let target = path.with_file_name(format!("{stem}.corrupted.txt"));
    std::fs::copy(path, &target).ok()?;
    std::fs::remove_file(path).ok()?;
    Some(target)
}

/// 损坏恢复:隔离原件 → 尝试 `.bak` 重建 → 失败回默认。隔离失败时保留
/// 原文件并拒绝重建(损坏禁覆盖)。
fn recover_corrupted(path: &Path) -> AssistantConfigLoad {
    if quarantine(path).is_none() {
        log::warn!("assistant 配置损坏且隔离失败;保留原文件, 不做重建");
        return AssistantConfigLoad {
            config: AssistantConfig::default(),
            corrupted: true,
        };
    }
    let config = std::fs::read(backup_path(path))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|text| AssistantConfig::parse(&text));
    let Some(config) = config else {
        return AssistantConfigLoad {
            config: AssistantConfig::default(),
            corrupted: true,
        };
    };
    if let Err(error) = crate::atomic_file::write(path, config.render().as_bytes()) {
        log::warn!("assistant 配置从 .bak 重建失败: {error}");
    }
    AssistantConfigLoad {
        config,
        corrupted: true,
    }
}

/// 同 pane 两次触发之间的最短间隔(连续失败的循环里别刷屏)。
pub(crate) const COOLDOWN: Duration = Duration::from_secs(5);

/// 触发序号:全局单调,响应落位时先比 pane 再比 seq。
static NEXT_SEQ: AtomicU64 = AtomicU64::new(1);

/// 每 pane 的触发闸门。只持有冷却时间点;规则判定全是纯函数。
#[derive(Debug, Default)]
pub(crate) struct TriggerGate {
    last_fired: Option<Instant>,
}

impl TriggerGate {
    /// 门链顺序固定:冷却(最便宜)→ 开关(默认关)→ 规则表。
    /// 全过才写冷却并取新 seq;任一门不过返回 `None` 且不消耗冷却。
    pub(crate) fn evaluate(
        &mut self,
        enabled: bool,
        exit_code: i32,
        command: &str,
        program: Option<&str>,
        ignored: &[i32],
        now: Instant,
    ) -> Option<u64> {
        if self
            .last_fired
            .is_some_and(|last| now.saturating_duration_since(last) < COOLDOWN)
        {
            return None;
        }
        if !enabled {
            return None;
        }
        if !should_suggest(exit_code, command, program, ignored) {
            return None;
        }
        self.last_fired = Some(now);
        Some(NEXT_SEQ.fetch_add(1, Ordering::Relaxed))
    }
}

/// hook 失败边的一次失败事实(08 篇改造节 9 / D08-2)。config_guard 自愈
/// 失败不是 shell 命令失败,没有真实退出码与最后命令;这里把失败操作投影
/// 成触发链能消费的参数:`command` 是操作描述,只参与规则表判定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HookFailure {
    pub command: String,
    /// 失败操作的程序身份;自愈失败统一为 `None`——agent 名(claude/codex
    /// 等)在 INTERACTIVE 表里,误当成 program 会把该报的失败静音。
    pub program: Option<String>,
}

/// 失败边的合成退出码:非 0、非用户中止码;用户把它写进
/// `ignored_exit_codes` 即可整体静音失败边。
pub(crate) const HOOK_FAILURE_EXIT_CODE: i32 = 1;

/// 失败边通道(D08-2 默认关):config_guard 自愈失败进入与命令失败同一
/// 门链(冷却 → 开关 → 规则表)。未显式开启(`AssistantConfig::enabled`
/// 默认 `false`)时连闸门都不碰——不评估、不写冷却;开启后语义与命令失败
/// 一致,冷却按进程级失败边闸门计。
pub(crate) fn evaluate_hook_failure(
    gate: &mut TriggerGate,
    config: &AssistantConfig,
    failure: &HookFailure,
    now: Instant,
) -> Option<u64> {
    if !config.enabled {
        return None;
    }
    gate.evaluate(
        true,
        HOOK_FAILURE_EXIT_CODE,
        &failure.command,
        failure.program.as_deref(),
        &config.ignored_exit_codes,
        now,
    )
}

/// 表示「用户停了它」而不是「它失败了」的退出码:
/// 130/143/137 = SIGINT/SIGTERM/SIGKILL + 128;负常数是 Windows
/// STATUS_CONTROL_C_EXIT 的 i32 形态。
const USER_ABORT_CODES: &[i32] = &[130, 137, 143, -1073741510];

/// 单词调用天然失败的帮助屏命令:给裸 `git` 提「修复」只会烦人。
const BARE_TOOLS: &[&str] = &[
    "npm", "pnpm", "yarn", "pip", "pip3", "cargo", "git", "dotnet", "go", "docker", "kubectl",
];

/// 交互式程序:非零退出是常态(`:cq`、负载下退出、ssh 掉线),不该「修」。
const INTERACTIVE: &[&str] = &[
    "vim", "nvim", "vi", "nano", "less", "more", "top", "htop", "btop", "ssh", "claude", "codex",
    "gemini", "aider", "lazygit", "yazi", "ranger", "fzf", "gh",
];

/// 防误触判定(规则表)。`program` 与侧栏图标/命令位置源共用同一份身份
/// (`extract_program(last_committed)`)。
pub(crate) fn should_suggest(
    exit_code: i32,
    command: &str,
    program: Option<&str>,
    ignored: &[i32],
) -> bool {
    if exit_code == 0 || USER_ABORT_CODES.contains(&exit_code) || ignored.contains(&exit_code) {
        return false;
    }
    let command = command.trim();
    if command.is_empty() {
        return false;
    }
    let mut words = command.split_whitespace();
    let first = words.next().unwrap_or_default().to_ascii_lowercase();
    let rest: Vec<&str> = words.collect();
    // 多个工具的帮助屏以非零码退出;那是用户主动要的。
    if rest
        .iter()
        .any(|word| matches!(*word, "--help" | "-h" | "-?" | "/?" | "help"))
    {
        return false;
    }
    if rest.is_empty() && BARE_TOOLS.contains(&first.as_str()) {
        return false;
    }
    if let Some(program) = program
        && INTERACTIVE.contains(&program.to_ascii_lowercase().as_str())
    {
        return false;
    }
    true
}

/// 本地危险词表,与模型自报的 `danger` 按位 OR:模型判意图、词表判字面,
/// 单方都可被骗。
pub(crate) fn is_dangerous(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    const PATTERNS: &[&str] = &[
        "rm -rf",
        "rm -fr",
        "git reset --hard",
        "git push --force",
        "git push -f",
        "git clean -fd",
        "remove-item -recurse",
        "rd /s",
        "del /s",
        "del /f",
        "format ",
        "mkfs",
        "dd if=",
        "shutdown",
        "taskkill /f",
        "kill -9",
        "chmod -r 777",
        "> /dev/sda",
        "drop table",
        "truncate table",
    ];
    PATTERNS.iter().any(|pattern| lower.contains(pattern))
}

/// 打码疑似密钥:40+ 连续 base64/hex 字符(token、私钥体、connection
/// string 的主体形态)替换为 `[redacted]`。出境前唯一闸。
pub(crate) fn redact_secrets(text: &str) -> String {
    fn flush(run: &mut String, out: &mut String) {
        if run.chars().count() >= 40 {
            out.push_str("[redacted]");
        } else {
            out.push_str(run);
        }
        run.clear();
    }

    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric()
            || ch == '+'
            || ch == '/'
            || ch == '='
            || ch == '-'
            || ch == '_'
        {
            run.push(ch);
        } else {
            flush(&mut run, &mut out);
            out.push(ch);
        }
    }
    flush(&mut run, &mut out);
    out
}

/// 一次修复请求携带的全部上下文。
#[derive(Debug, Clone)]
pub(crate) struct FixRequest {
    pub pane: PaneId,
    pub seq: u64,
    pub command: String,
    pub exit_code: i32,
    pub cwd: String,
    pub branch: String,
    /// 失败命令的真实输出尾部,ANSI-free,已过 [`redact_secrets`]。
    pub output_tail: String,
}

/// 兜底凭据的环境变量回退序:`SLTERM_AI_KEY` → `OPENAI_API_KEY`。
const FALLBACK_ENV_VARS: [&str; 2] = ["SLTERM_AI_KEY", "OPENAI_API_KEY"];

/// 纯函数形态:注入环境读取器,测试不改进程环境。
fn fallback_api_key_from(lookup: impl Fn(&str) -> Option<String>) -> Option<Zeroizing<String>> {
    for var in FALLBACK_ENV_VARS {
        if let Some(value) = lookup(var) {
            let value = value.trim().to_owned();
            if !value.is_empty() {
                return Some(Zeroizing::new(value));
            }
        }
    }
    None
}

/// 无启用 provider 时的环境兜底;消费点 = M9.5 请求目标组装。
#[allow(dead_code)]
pub(crate) fn fallback_api_key() -> Option<Zeroizing<String>> {
    fallback_api_key_from(|name| std::env::var(name).ok())
}

/// 从模型回复中抗噪提取 JSON:首个 `{` 至末个 `}` 切片。空 command 视为
/// 刻意沉默;单命令行约束(含换行即拒);`danger` = 模型标志 OR 本地词表。
pub(crate) fn parse_fix(content: &str) -> Option<AiFix> {
    let start = content.find('{')?;
    let end = content.rfind('}')?;
    if end < start {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&content[start..=end]).ok()?;
    let command = value["command"].as_str()?.trim().to_owned();
    if command.is_empty() || command.contains('\n') || command.contains('\r') {
        return None;
    }
    let explain = value["explain"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_owned();
    let danger = value["danger"].as_bool().unwrap_or(false) || is_dangerous(&command);
    Some(AiFix {
        command,
        explain,
        danger,
    })
}

#[cfg(test)]
mod ai_assistant_tests {
    use super::*;
    use std::collections::HashMap;

    // ---- should_suggest 规则表 ----

    #[test]
    fn suggests_only_for_real_failures() {
        assert!(should_suggest(127, "cargoo build", None, &[]));
        assert!(!should_suggest(0, "cargo build", None, &[]));
        // 用户主动中止,Unix 与 Windows 两套约定都覆盖。
        for code in [130, 137, 143, -1073741510] {
            assert!(
                !should_suggest(code, "npm run dev", None, &[]),
                "code {code}"
            );
        }
        // 帮助屏与裸包管理器天然非零退出。
        assert!(!should_suggest(1, "git --help", None, &[]));
        assert!(!should_suggest(1, "cargo", None, &[]));
        assert!(should_suggest(101, "cargo buld", None, &[]));
        // 交互式程序非零退出是常态。
        assert!(!should_suggest(1, "vim src/main.rs", Some("vim"), &[]));
        // 用户自定义忽略码。
        assert!(!should_suggest(2, "make", None, &[2]));
    }

    #[test]
    fn help_forms_and_bare_tools_are_suppressed_case_insensitively() {
        for command in [
            "git --help",
            "cargo -h",
            "foo -?",
            "foo /?",
            "foo help",
            "python --help now",
        ] {
            assert!(!should_suggest(1, command, None, &[]), "{command}");
        }
        for command in ["GIT", "Cargo", "npm", "pip3", "kubectl"] {
            assert!(!should_suggest(1, command, None, &[]), "{command}");
        }
        assert!(should_suggest(1, "git status", None, &[]));
        assert!(should_suggest(1, "cargo build", None, &[]));
    }

    #[test]
    fn interactive_programs_are_suppressed_by_program_identity() {
        assert!(!should_suggest(1, "exit", Some("VIM"), &[]));
        assert!(!should_suggest(1, "q", Some("gh"), &[]));
        assert!(!should_suggest(1, "x", Some("fzf"), &[]));
        assert!(should_suggest(1, "exit", Some("cargo"), &[]));
        assert!(should_suggest(1, "exit", None, &[]));
    }

    #[test]
    fn blank_commands_and_ignored_codes_never_suggest() {
        assert!(!should_suggest(1, "", None, &[]));
        assert!(!should_suggest(1, "   ", None, &[]));
        assert!(!should_suggest(1, "make", None, &[1, 2]));
    }

    // ---- 触发闸门:冷却 → 开关 → 规则表 ----

    #[test]
    fn default_off_keeps_the_chain_silent() {
        let mut gate = TriggerGate::default();
        let now = Instant::now();
        assert!(
            gate.evaluate(false, 127, "cargoo build", None, &[], now)
                .is_none()
        );
        // 关闭门不过不消耗冷却:立即打开后同一时刻即可触发。
        assert!(
            gate.evaluate(true, 127, "cargoo build", None, &[], now)
                .is_some()
        );
    }

    #[test]
    fn cooldown_is_five_seconds_per_pane() {
        let mut gate = TriggerGate::default();
        let start = Instant::now();
        assert!(
            gate.evaluate(true, 127, "cargoo build", None, &[], start)
                .is_some()
        );
        assert!(
            gate.evaluate(
                true,
                127,
                "cargoo build",
                None,
                &[],
                start + Duration::from_secs(4)
            )
            .is_none()
        );
        assert!(
            gate.evaluate(true, 127, "cargoo build", None, &[], start + COOLDOWN)
                .is_some()
        );
    }

    #[test]
    fn failed_rule_gate_does_not_arm_the_cooldown() {
        let mut gate = TriggerGate::default();
        let now = Instant::now();
        assert!(
            gate.evaluate(true, 0, "cargo build", None, &[], now)
                .is_none()
        );
        assert!(
            gate.evaluate(true, 130, "npm run dev", None, &[], now)
                .is_none()
        );
        assert!(gate.evaluate(true, 1, "cargo", None, &[], now).is_none());
        // 三门都过:同一时刻仍然可以触发。
        assert!(
            gate.evaluate(true, 101, "cargo buld", None, &[], now)
                .is_some()
        );
    }

    #[test]
    fn sequence_is_globally_monotonic() {
        let mut first = TriggerGate::default();
        let mut second = TriggerGate::default();
        let now = Instant::now();
        let a = first
            .evaluate(true, 127, "cargoo build", None, &[], now)
            .unwrap();
        let b = second
            .evaluate(true, 127, "cargoo build", None, &[], now)
            .unwrap();
        let c = first
            .evaluate(true, 127, "cargoo build", None, &[], now + COOLDOWN)
            .unwrap();
        assert!(a < b && b < c, "seq 必须全局递增: {a} {b} {c}");
    }

    // ---- 危险词表 ----

    #[test]
    fn danger_catches_both_shells() {
        assert!(is_dangerous("rm -rf /tmp/x"));
        assert!(is_dangerous("git push --force origin main"));
        assert!(is_dangerous("Remove-Item -Recurse -Force C:\\x"));
        assert!(is_dangerous("taskkill /f /im node.exe"));
        assert!(is_dangerous("DROP TABLE users"));
        assert!(!is_dangerous("cargo build --release"));
    }

    #[test]
    fn danger_patterns_cover_the_whole_table() {
        for command in [
            "rm -fr build",
            "git reset --hard HEAD~1",
            "git push -f",
            "git clean -fd",
            "rd /s /q C:\\x",
            "del /s *.tmp",
            "del /f read-only.txt",
            "format C:",
            "mkfs.ext4 /dev/sda1",
            "dd if=/dev/zero of=/dev/sda",
            "shutdown /s",
            "kill -9 1234",
            "chmod -R 777 .",
            "echo x > /dev/sda",
            "truncate table audit",
        ] {
            assert!(is_dangerous(command), "{command}");
        }
    }

    // ---- redact_secrets ----

    #[test]
    fn redaction_hides_long_tokens_only() {
        let text = "token=ghp_0123456789abcdef0123456789abcdef01234567 path=C:/x";
        let out = redact_secrets(text);
        assert!(out.contains("[redacted]"), "got {out}");
        assert!(out.contains("path=C:/x"), "got {out}");
        assert_eq!(redact_secrets("plain words"), "plain words");
    }

    #[test]
    fn redaction_boundary_is_exactly_forty_characters() {
        let short = "a".repeat(39);
        let long = "a".repeat(40);
        assert_eq!(redact_secrets(&short), short);
        assert_eq!(redact_secrets(&long), "[redacted]");
    }

    #[test]
    fn redaction_covers_base64_and_hex_joins() {
        let token = format!("{}{}{}", "A".repeat(20), "/+=", "b".repeat(20));
        assert_eq!(redact_secrets(&token), "[redacted]");
        // `=` 属于长串字符集:赋值符号与值一起打码是预期形态。
        let with_punct = format!("key={} done", "Z".repeat(40));
        assert_eq!(redact_secrets(&with_punct), "[redacted] done");
        // 中文与空白是分隔符,不并入长串。
        let mixed = format!("{}{}", "x".repeat(39), "中文");
        assert_eq!(
            redact_secrets(&mixed),
            format!("{}{}", "x".repeat(39), "中文")
        );
    }

    // ---- parse_fix ----

    #[test]
    fn parses_model_reply_with_fence_noise() {
        let fix = parse_fix(
            "Sure!\n```json\n{\"command\":\"cargo build\",\"explain\":\"拼写\",\"danger\":false}\n```",
        )
        .unwrap();
        assert_eq!(fix.command, "cargo build");
        assert_eq!(fix.explain, "拼写");
        assert!(!fix.danger);
    }

    #[test]
    fn local_danger_table_overrides_a_lying_model() {
        let fix =
            parse_fix("{\"command\":\"rm -rf /\",\"explain\":\"x\",\"danger\":false}").unwrap();
        assert!(fix.danger);
    }

    #[test]
    fn empty_command_means_deliberate_silence() {
        assert!(parse_fix("{\"command\":\"\",\"explain\":\"no fix\",\"danger\":false}").is_none());
    }

    #[test]
    fn multi_line_commands_are_rejected() {
        assert!(parse_fix("{\"command\":\"cargo build\\nrm -rf /\",\"danger\":false}").is_none());
        assert!(parse_fix("{\"command\":\"cargo build\\rrm -rf /\",\"danger\":false}").is_none());
    }

    #[test]
    fn malformed_replies_do_not_panic() {
        for reply in [
            "",
            "no json here",
            "{\"command\":42}",
            "{\"explain\":\"missing command\"}",
            "{ broken",
            "}{",
        ] {
            assert!(parse_fix(reply).is_none(), "{reply}");
        }
        let fix = parse_fix("{\"command\":\"ls\"}").unwrap();
        assert_eq!(fix.explain, "");
        assert!(!fix.danger);
    }

    // ---- fallback_api_key 回退序 ----

    #[test]
    fn fallback_prefers_slterm_key_then_openai_key() {
        let both = HashMap::from([
            ("SLTERM_AI_KEY", "sk-test-slterm"),
            ("OPENAI_API_KEY", "sk-test-openai"),
        ]);
        let key =
            fallback_api_key_from(|name| both.get(name).map(|value| (*value).to_owned())).unwrap();
        assert_eq!(key.as_str(), "sk-test-slterm");

        let only_openai = HashMap::from([("OPENAI_API_KEY", "sk-test-openai")]);
        let key =
            fallback_api_key_from(|name| only_openai.get(name).map(|value| (*value).to_owned()))
                .unwrap();
        assert_eq!(key.as_str(), "sk-test-openai");

        let empty_first = HashMap::from([
            ("SLTERM_AI_KEY", "   "),
            ("OPENAI_API_KEY", "sk-test-openai"),
        ]);
        let key =
            fallback_api_key_from(|name| empty_first.get(name).map(|value| (*value).to_owned()))
                .unwrap();
        assert_eq!(key.as_str(), "sk-test-openai");

        let none: HashMap<&str, &str> = HashMap::new();
        assert!(fallback_api_key_from(|name| none.get(name).map(|v| (*v).to_owned())).is_none());
    }

    #[test]
    fn fallback_trims_whitespace() {
        let key = fallback_api_key_from(|_| Some("  sk-test-trim  ".to_owned())).unwrap();
        assert_eq!(key.as_str(), "sk-test-trim");
    }

    // ---- 独立配置文件:往返 / 原子写 / 损坏三态 ----

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(CONFIG_FILE);
        (directory, path)
    }

    fn directory_names(directory: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn defaults_are_disabled() {
        let config = AssistantConfig::default();
        assert!(!config.enabled);
        assert!(!config.base_url.is_empty());
        assert!(!config.model.is_empty());
        assert!(config.ignored_exit_codes.is_empty());
        assert_eq!(AssistantConfig::parse(""), Some(AssistantConfig::default()));
    }

    #[test]
    fn config_round_trips_through_atomic_file() {
        let (directory, path) = fixture();
        let config = AssistantConfig {
            enabled: true,
            base_url: "https://example.test/v1".into(),
            model: "test-model".into(),
            ignored_exit_codes: vec![2, 3, 130],
        };
        config.save_at(&path).unwrap();
        let loaded = AssistantConfig::load_at(&path).unwrap();
        assert!(!loaded.corrupted);
        assert_eq!(loaded.config, config);
        assert_eq!(
            directory_names(directory.path()),
            [CONFIG_FILE.to_owned()],
            "原子替换不得残留临时文件"
        );

        // 第二次保存前先把旧文档完整备份。
        let first = std::fs::read(&path).unwrap();
        let second = AssistantConfig {
            enabled: false,
            ..config
        };
        second.save_at(&path).unwrap();
        assert_eq!(std::fs::read(backup_path(&path)).unwrap(), first);
        assert_eq!(
            directory_names(directory.path()),
            [CONFIG_FILE.to_owned(), format!("{CONFIG_FILE}.bak"),]
        );
        assert_eq!(AssistantConfig::load_at(&path).unwrap().config, second);
    }

    #[test]
    fn config_parses_handwritten_forms_and_ignores_noise() {
        let config = AssistantConfig::parse(
            "# comment\nenabled=on\nunknown=value\nmodel=m1\nignored_exit_codes=2, 3, bad\n",
        )
        .unwrap();
        assert!(config.enabled);
        assert_eq!(config.model, "m1");
        assert_eq!(config.ignored_exit_codes, vec![2, 3]);
        assert!(!AssistantConfig::parse("enabled=false\n").unwrap().enabled);
        // 纯注释文件不是损坏:回默认值。
        assert_eq!(
            AssistantConfig::parse("# only a comment\n"),
            Some(AssistantConfig::default())
        );
    }

    #[test]
    fn missing_file_reads_defaults_without_creating_it() {
        let (_directory, path) = fixture();
        let loaded = AssistantConfig::load_at(&path).unwrap();
        assert!(!loaded.corrupted);
        assert_eq!(loaded.config, AssistantConfig::default());
        assert!(!path.exists(), "读取不得凭空创建文件");
    }

    #[test]
    fn corrupt_file_is_quarantined_and_rebuilt_from_backup() {
        let (_directory, path) = fixture();
        let corrupt = "this is not a key value file";
        std::fs::write(&path, corrupt).unwrap();
        let backup = AssistantConfig {
            enabled: true,
            model: "from-backup".into(),
            ..AssistantConfig::default()
        };
        std::fs::write(backup_path(&path), backup.render()).unwrap();

        let loaded = AssistantConfig::load_at(&path).unwrap();
        assert!(loaded.corrupted);
        assert_eq!(loaded.config, backup);
        let quarantined = path.with_file_name("slterm_assistant.corrupted.txt");
        assert_eq!(std::fs::read_to_string(&quarantined).unwrap(), corrupt);
        // 主文件已从 .bak 重建(非原地覆盖损坏件)。
        let rebuilt = AssistantConfig::load_at(&path).unwrap();
        assert!(!rebuilt.corrupted);
        assert_eq!(rebuilt.config, backup);
    }

    #[test]
    fn corrupt_file_without_backup_reports_defaults_and_keeps_quarantine() {
        let (_directory, path) = fixture();
        let corrupt = "definitely not a config {{{";
        std::fs::write(&path, corrupt).unwrap();
        let loaded = AssistantConfig::load_at(&path).unwrap();
        assert!(loaded.corrupted);
        assert_eq!(loaded.config, AssistantConfig::default());
        assert_eq!(
            std::fs::read_to_string(path.with_file_name("slterm_assistant.corrupted.txt")).unwrap(),
            corrupt
        );
        assert!(!path.exists(), "损坏件已隔离,主路径待下次写入重建");
    }

    #[test]
    fn non_utf8_file_is_treated_as_corrupt() {
        let (_directory, path) = fixture();
        std::fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();
        let loaded = AssistantConfig::load_at(&path).unwrap();
        assert!(loaded.corrupted);
        assert_eq!(loaded.config, AssistantConfig::default());
        assert!(
            path.with_file_name("slterm_assistant.corrupted.txt")
                .exists()
        );
    }

    #[test]
    fn save_sanitizes_multiline_values() {
        let (_directory, path) = fixture();
        let config = AssistantConfig {
            base_url: " https://example.test/v1\ninjected=on ".into(),
            model: "model\r\nname".into(),
            ..AssistantConfig::default()
        };
        config.save_at(&path).unwrap();
        let loaded = AssistantConfig::load_at(&path).unwrap().config;
        assert_eq!(loaded.base_url, "https://example.test/v1injected=on");
        assert_eq!(loaded.model, "modelname");
        assert!(!loaded.enabled);
    }
}
