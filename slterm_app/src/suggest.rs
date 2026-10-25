//! 补全/建议引擎:ghost 余量与弹窗候选的计算核心。
//!
//! 从上游 `display/suggest_engine.rs` + `display/command_completion.rs` +
//! `display/completion.rs` 收敛为自由函数:不持有窗口、渲染器或执行器状态。
//! 两个呈现形态(GPUI ghost/popup 接线)共用同一套数据源与排序规则,
//! 避免第二套平行实现。
//!
//! 环境抽象收敛为本地单态:补全面对的**永远是本机**文件系统与 PATH。
//! 上游基线中 WSL/SSH 的「走错机器不是补不出来,是补出
//! 另一台机器上的路径」教训保留在此形态下:跨机器分支已无对象,不重新引入。

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use slterm_completions::command_context::ShellSyntax;
use slterm_completions::{SemanticSuggestion, SuggestionKind};

use crate::directory_history::DirectoryHistory;
use crate::history::{HistoryScope, SltermHistory};

/// 补全样式本体归键域枚举,此处只再导出(与上游同形态)。
pub(crate) use slterm_settings::CompletionStyleName as CompletionStyle;

/// 弹窗候选来源;驱动右侧 tag 与图标四态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SltermCompletionKind {
    History,
    Command,
    Dir,
    File,
}

/// 弹窗列表的一行(呈现条目唯一定义处)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SltermCompletionItem {
    /// 展示文本(可能左省略)。
    pub label: String,
    /// 接受时写入 PTY 的字符(用户已打部分的余量)。
    pub insert: String,
    /// 光标前替换字符数。
    pub replace_chars: usize,
    /// 光标后替换字符数(原生行编辑器证明的尾部)。
    pub replace_after_chars: usize,
    pub kind: SltermCompletionKind,
}

/// 幽灵余量上限:防长路径/长命令溢进 chrome。
pub(crate) const SLTERM_GHOST_MAX: usize = 96;

/// 补全执行环境。远程/嵌套 shell 分池随定位约束整砍,只剩本机单态。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SuggestEnv {
    #[default]
    Local,
}

impl SuggestEnv {
    pub(crate) fn is_this_machine(self) -> bool {
        true
    }

    pub(crate) fn history_scope(self) -> HistoryScope {
        HistoryScope::Local
    }
}

/// 一次重算的输入快照;不含窗口或终端状态。
pub(crate) struct Input<'a> {
    pub cwd: &'a str,
    pub env: &'a SuggestEnv,
    pub line: &'a str,
}

/// 借用的共享数据源与运行时开关。生命周期只覆盖一次重算调用。
pub(crate) struct SuggestSources<'a> {
    /// 历史单持有(Mutex);只在取少量匹配项时加锁,文件扫描期间必须释放。
    pub history: &'a Mutex<SltermHistory>,
    pub directories: &'a DirectoryHistory,
    /// PATH 探针结果(后台线程填充)。
    pub commands: &'a Arc<Mutex<Vec<String>>>,
    pub enabled: bool,
    pub style: CompletionStyle,
}

/// 每 pane 的补全缓存与关闭语义(原 pane 状态补全面收敛)。
///
/// 视图只读这些字段渲染;按键路径只更新输入快照与缓存键,候选在后台算。
#[derive(Default)]
pub(crate) struct SuggestState {
    /// cwd + env + line + style + 命令代际的缓存键。
    pub suggestion_key: String,
    /// Esc 关闭语义:行不变不重开。
    pub suppressed_line: Option<String>,
    /// Hybrid 模式的弹窗请求位。
    pub popup_requested: bool,
    /// Inline ghost 余量。
    pub suggestion: String,
    /// Inline 接受时的编辑合同。
    pub suggestion_edit: Option<SltermCompletionItem>,
    /// Popup 列表。
    pub completion_items: Vec<SltermCompletionItem>,
    /// 用户尚未主动导航时不高亮任何候选;这保证 Enter 仍提交原始输入。
    pub completion_selected: Option<usize>,
    /// 当前缓存键的后台结果已回填。
    pub completion_result_ready: bool,
}

impl SuggestState {
    /// Hybrid 的 Tab 请求只改变呈现,不向 PTY 写入。
    pub(crate) fn request_completion_popup(&mut self) -> bool {
        if self.popup_requested {
            return true;
        }
        if self.suggestion.is_empty() && self.completion_items.is_empty() {
            return false;
        }
        self.popup_requested = true;
        self.suppressed_line = None;
        self.clear_completion_hints();
        true
    }

    /// 弹窗高亮行循环移动。初次导航从首项进入(Up 不至列表末尾)。
    pub(crate) fn completion_popup_move(&mut self, delta: isize) {
        let len = self.completion_items.len();
        if len == 0 {
            return;
        }
        self.completion_selected = Some(match self.completion_selected {
            Some(current) => (current as isize + delta).rem_euclid(len as isize) as usize,
            None => 0,
        });
    }

    /// 取走选中候选并关闭列表。
    pub(crate) fn completion_popup_take(&mut self) -> Option<SltermCompletionItem> {
        let item = self
            .completion_items
            .get(self.completion_selected?)?
            .clone();
        self.completion_items.clear();
        self.completion_selected = None;
        self.popup_requested = false;
        Some(item)
    }

    /// Esc 关闭列表:候选清空但重算键保留,列表在行变化前不会复开。
    /// 返回是否真的关了。
    pub(crate) fn completion_popup_dismiss(&mut self, line: &str) -> bool {
        if self.popup_requested {
            self.popup_requested = false;
            self.clear_completion_hints();
            return true;
        }
        if self.completion_items.is_empty() {
            return false;
        }
        if !line.is_empty() {
            self.suppressed_line = Some(line.to_owned());
        }
        self.completion_items.clear();
        self.completion_selected = None;
        true
    }

    pub(crate) fn completion_query_matches(&self, key: &str) -> bool {
        self.suggestion_key == key
    }

    pub(crate) fn completion_query_ready(&self, key: &str) -> bool {
        self.completion_query_matches(key) && self.completion_result_ready
    }

    pub(crate) fn finish_completion_query(&mut self) {
        self.completion_result_ready = true;
    }

    pub(crate) fn begin_completion_query(&mut self, key: String) {
        self.clear_completion_hints();
        self.suggestion_key = key;
    }

    /// 丢弃 ghost 与弹窗候选并作废重算缓存。
    pub(crate) fn clear_completion_hints(&mut self) {
        self.suggestion.clear();
        self.suggestion_edit = None;
        self.suggestion_key.clear();
        self.completion_result_ready = false;
        self.completion_items.clear();
        self.completion_selected = None;
    }
}

/// 两种呈现形态的候选结果。
#[derive(Default)]
pub(crate) struct Candidates {
    /// Inline ghost 余量。
    pub suggestion: String,
    /// Inline 接受时的编辑合同(字节 span → 行尾替换)。
    pub suggestion_edit: Option<SltermCompletionItem>,
    /// Popup 列表。
    pub completion_items: Vec<SltermCompletionItem>,
}

/// 计算一次补全。只借用输入快照;取消令牌全链路透传。
pub(crate) fn calculate(
    sources: &SuggestSources<'_>,
    input: &Input<'_>,
    syntax: ShellSyntax,
    cancelled: &dyn Fn() -> bool,
) -> Candidates {
    let mut result = Candidates::default();
    let line = input.line;
    if cancelled() || !sources.enabled || line.is_empty() {
        return result;
    }

    // Popup 计算多候选列表;两种形态按 pane 互斥。
    if sources.style == CompletionStyle::Popup {
        suggest_collect(sources, input, syntax, &mut result, cancelled);
        return result;
    }

    // 历史优先:最新一条整行前缀匹配(索引 range,开销随命中数增长)。
    let hinted = {
        let history = sources
            .history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        history
            .hint_with_cancel(&input.env.history_scope(), line, cancelled)
            .map(clamp_ghost)
    };
    if let Some(remainder) = hinted {
        result.suggestion = remainder;
        return result;
    }
    if cancelled() {
        return result;
    }

    // 首词且无路径分隔符 = 命令位置;复用 shell 继承的进程 PATH。
    if is_command_position(line)
        && let Ok(commands) = sources.commands.lock()
        && let Some(remainder) = command_hint(commands.as_slice(), line)
    {
        result.suggestion = clamp_ghost(remainder);
        return result;
    }

    let paths = crate::completion::paths::complete(
        input,
        syntax,
        sources.directories,
        sources.style,
        None,
        cancelled,
    );
    semantic_candidates(line, sources.style, paths)
}

/// 输入、执行环境与后台数据代际共同标识候选;异步适配器用它拒绝过期结果。
pub(crate) fn suggestion_key(
    input: &Input<'_>,
    style: CompletionStyle,
    command_generation: usize,
) -> String {
    format!(
        "{:?}\0{}\0{}\0{style:?}\0{command_generation}",
        input.env, input.cwd, input.line
    )
}

/// 截断幽灵余量,防长路径溢进 chrome。
fn clamp_ghost(remainder: &str) -> String {
    remainder.chars().take(SLTERM_GHOST_MAX).collect()
}

/// 长标签从左侧省略(路径保留信息量更大的尾部,用户已打的前缀可舍)。
pub(crate) fn elide_left(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_owned();
    }
    let tail: String = text.chars().skip(count + 1 - max_chars).collect();
    format!("…{tail}")
}

/// 保留候选拼写:大小写不敏感/模糊匹配改变了已打文本时,只有字面前缀可安全追加。
fn popup_edit(line: &str, candidate: &str) -> (usize, String) {
    match candidate.strip_prefix(line) {
        Some(suffix) => (0, suffix.to_owned()),
        None => (line.chars().count(), candidate.to_owned()),
    }
}

/// 把语义 byte span 投影为终端行尾编辑合同。
pub(crate) fn semantic_candidates(
    line: &str,
    style: CompletionStyle,
    candidates: Vec<SemanticSuggestion>,
) -> Candidates {
    semantic_candidates_at(line, line.len(), style, candidates)
}

pub(crate) fn semantic_candidates_at(
    line: &str,
    cursor: usize,
    style: CompletionStyle,
    candidates: Vec<SemanticSuggestion>,
) -> Candidates {
    let mut result = Candidates::default();
    let items: Vec<_> = candidates
        .into_iter()
        .filter_map(|semantic| {
            let candidate = semantic.suggestion;
            if candidate.span.start > cursor
                || candidate.span.end < cursor
                || candidate.span.end > line.len()
            {
                return None;
            }
            let token = line.get(candidate.span.start..cursor)?;
            // 只替换分歧后的尾部;已闭合引号也能接受,不删除整条命令或 UTF-8 半字符。
            let common: usize = token
                .chars()
                .zip(candidate.value.chars())
                .take_while(|(a, b)| a == b)
                .map(|(ch, _)| ch.len_utf8())
                .sum();
            Some(SltermCompletionItem {
                label: elide_left(candidate.display_value(), 44),
                insert: candidate.value[common..].to_owned(),
                replace_chars: token[common..].chars().count(),
                replace_after_chars: line.get(cursor..candidate.span.end)?.chars().count(),
                kind: match semantic.kind {
                    Some(SuggestionKind::Directory) => SltermCompletionKind::Dir,
                    Some(SuggestionKind::File) => SltermCompletionKind::File,
                    _ => SltermCompletionKind::Command,
                },
            })
        })
        .filter(|item| {
            style == CompletionStyle::Popup
                || item.replace_chars != 0
                || item.replace_after_chars != 0
                || !item.insert.is_empty()
        })
        .take(256)
        .collect();
    if style == CompletionStyle::Popup {
        result.completion_items = items;
    } else if let Some(item) = items
        .into_iter()
        .find(|item| !item.insert.is_empty() || item.replace_after_chars != 0)
    {
        result.suggestion = clamp_ghost(&item.insert);
        result.suggestion_edit = Some(item);
    }
    result
}

/// 填充 popup 列表:与 ghost 同源(历史 → PATH 命令 → 文件系统),但每源保留多项。
fn suggest_collect(
    sources: &SuggestSources<'_>,
    input: &Input<'_>,
    syntax: ShellSyntax,
    result: &mut Candidates,
    cancelled: &dyn Fn() -> bool,
) {
    // 8 是视口行数,不是数据上限。旧实现把两者混成一个常量,候选在收集
    // 阶段就被截断,因而既画不出滚动条,Tab 也永远走不到第九项以后。
    const POPUP_LIMIT: usize = 256;
    const LABEL_MAX: usize = 44;

    let mut items: Vec<SltermCompletionItem> = Vec::new();
    let push = |items: &mut Vec<SltermCompletionItem>, item: SltermCompletionItem| {
        if item.insert.is_empty() {
            return;
        }
        if items
            .iter()
            .any(|seen| seen.insert == item.insert && seen.kind == item.kind)
        {
            return;
        }
        if items.len() < POPUP_LIMIT {
            items.push(item);
        }
    };

    // 整行历史匹配,新者在前。
    let history_matches = {
        let history = sources
            .history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        history
            .search_with_cancel(&input.env.history_scope(), input.line, 8, cancelled)
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    for full in history_matches {
        let (replace_chars, insert) = popup_edit(input.line, &full);
        push(
            &mut items,
            SltermCompletionItem {
                replace_chars,
                replace_after_chars: 0,
                label: elide_left(&full, LABEL_MAX),
                insert,
                kind: SltermCompletionKind::History,
            },
        );
    }

    if cancelled() {
        return;
    }

    // 首词输入时补 PATH 可执行文件;精确命令追加空格,Enter 只完成选择不执行。
    if is_command_position(input.line)
        && let Ok(commands) = sources.commands.lock()
    {
        for command in command_hints(commands.as_slice(), input.line, POPUP_LIMIT) {
            let (replace_chars, mut insert) = popup_edit(input.line, command);
            if command.eq_ignore_ascii_case(input.line) {
                insert.push(' ');
            }
            push(
                &mut items,
                SltermCompletionItem {
                    replace_chars,
                    replace_after_chars: 0,
                    label: elide_left(command, LABEL_MAX),
                    insert,
                    kind: SltermCompletionKind::Command,
                },
            );
        }
    }

    let paths = crate::completion::paths::complete(
        input,
        syntax,
        sources.directories,
        sources.style,
        None,
        cancelled,
    );
    for item in semantic_candidates(input.line, CompletionStyle::Popup, paths).completion_items {
        push(&mut items, item);
    }
    result.completion_items = items;
}

// ---------------------------------------------------------------------------
// 命令位置来源(`display/command_completion.rs` 收敛)
// ---------------------------------------------------------------------------

/// 已提交命令行的首词 → 程序身份:小写、剥路径与常见 Windows 启动后缀。
///
/// 终端命令边界与 Runtime 快照共用同一份程序身份。
pub(crate) fn extract_program(line: &str) -> Option<String> {
    let first = line.split_whitespace().next()?;
    let base = first
        .trim_matches('"')
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(first);
    let mut name = base.to_ascii_lowercase();
    for extension in [".exe", ".cmd", ".bat", ".ps1", ".com"] {
        if let Some(stripped) = name.strip_suffix(extension) {
            name = stripped.to_owned();
            break;
        }
    }
    (!name.is_empty()).then_some(name)
}

fn pathexts() -> Vec<String> {
    std::env::var("PATHEXT")
        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD;.PS1".to_owned())
        .split(';')
        .filter_map(|extension| {
            let extension = extension.trim();
            if extension.is_empty() {
                None
            } else if extension.starts_with('.') {
                Some(extension.to_ascii_lowercase())
            } else {
                Some(format!(".{}", extension.to_ascii_lowercase()))
            }
        })
        .collect()
}

fn command_name(path: &Path, pathexts: &[String]) -> Option<String> {
    let extension = path.extension()?.to_str()?;
    let extension = format!(".{extension}").to_ascii_lowercase();
    if !pathexts.iter().any(|known| known == &extension) {
        return None;
    }
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
}

fn path_commands() -> Vec<String> {
    let Some(path_env) = std::env::var_os("PATH") else {
        return Vec::new();
    };

    let mut commands = Vec::new();
    let mut seen = HashSet::new();
    let pathexts = pathexts();

    for directory in std::env::split_paths(&path_env).filter(|dir| !dir.as_os_str().is_empty()) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }
            if let Some(command) = command_name(&entry.path(), &pathexts)
                && seen.insert(command.to_ascii_lowercase())
            {
                commands.push(command);
            }
        }
    }

    commands.sort_by(|a, b| {
        a.to_ascii_lowercase()
            .cmp(&b.to_ascii_lowercase())
            .then(a.cmp(b))
    });
    commands
}

/// PowerShell cmdlet/函数/别名不在 PATH 上;不加载用户 profile 收集一次。
/// 产品默认 shell 是 pwsh 7,优先探测它;启动失败或非零退出才回退 5.1。
/// 两者都失败只意味着 PATH 命令仍是可用来源。
fn powershell_commands() -> Vec<String> {
    powershell_commands_with(|program| {
        let mut command = std::process::Command::new(program);
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-Command -CommandType Cmdlet,Function,Alias -ErrorAction SilentlyContinue \
             | Select-Object -ExpandProperty Name",
        ]);
        let output = crate::platform::process::hidden_command(&mut command)
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    })
    .unwrap_or_default()
}

/// `powershell_commands` 的可测形态:运行器返回 stdout(启动失败或非零退出为
/// None),按 pwsh → 5.1 顺序取首个成功结果并去空行/trim;两者都失败返回 None。
fn powershell_commands_with(run: impl Fn(&str) -> Option<String>) -> Option<Vec<String>> {
    for program in ["pwsh", "powershell"] {
        if let Some(stdout) = run(program) {
            return Some(
                stdout
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(ToOwned::to_owned)
                    .collect(),
            );
        }
    }
    None
}

fn collect_commands() -> Vec<String> {
    let mut commands = path_commands();
    let mut seen: HashSet<String> = commands.iter().map(|c| c.to_ascii_lowercase()).collect();
    for command in powershell_commands() {
        if seen.insert(command.to_ascii_lowercase()) {
            commands.push(command);
        }
    }
    commands.sort_by(|a, b| {
        a.to_ascii_lowercase()
            .cmp(&b.to_ascii_lowercase())
            .then(a.cmp(b))
    });
    commands
}

/// 进程级命令表,后台线程填充——发现过程不阻塞窗口启动/按键路径。
pub(crate) fn commands_handle() -> Arc<Mutex<Vec<String>>> {
    static COMMANDS: OnceLock<Arc<Mutex<Vec<String>>>> = OnceLock::new();
    COMMANDS
        .get_or_init(|| {
            let shared = Arc::new(Mutex::new(Vec::new()));
            let background = shared.clone();
            std::thread::spawn(move || {
                let commands = collect_commands();
                if let Ok(mut guard) = background.lock() {
                    *guard = commands;
                }
            });
            shared
        })
        .clone()
}

/// Inline 命令余量;精确命中不往更长的邻居补。
pub(crate) fn command_hint<'a>(commands: &'a [String], prefix: &str) -> Option<&'a str> {
    if prefix.is_empty() {
        return None;
    }
    if commands
        .iter()
        .any(|command| command.eq_ignore_ascii_case(prefix))
    {
        return None;
    }
    commands.iter().find_map(|command| {
        if command.len() <= prefix.len() || !command.is_char_boundary(prefix.len()) {
            return None;
        }
        let (head, remainder) = command.split_at(prefix.len());
        head.eq_ignore_ascii_case(prefix).then_some(remainder)
    })
}

/// 返回最多 `limit` 个扩展 `prefix` 的完整命令名;精确命中排首位但不压制邻居。
pub(crate) fn command_hints<'a>(
    commands: &'a [String],
    prefix: &str,
    limit: usize,
) -> Vec<&'a str> {
    if prefix.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut query = slterm_completions::command_search::CommandQuery::new(prefix);
    let mut matches: Vec<_> = commands
        .iter()
        .filter_map(|command| query.score(command).map(|score| (score, command.as_str())))
        .collect();
    matches.sort_by(|(a_score, a), (b_score, b)| {
        b_score
            .cmp(a_score)
            .then(a.len().cmp(&b.len()))
            .then(a.cmp(b))
    });
    matches.dedup_by(|a, b| a.1 == b.1);
    matches
        .into_iter()
        .take(limit)
        .map(|(_, command)| command)
        .collect()
}

/// 首词且无分隔符 = 命令位置。
pub(crate) fn is_command_position(line: &str) -> bool {
    !line.contains([' ', '\t'])
        && !line.contains(['/', '\\'])
        && line.as_bytes().get(1) != Some(&b':')
}

/// cd 类命令的路径补全只要目录。
pub(crate) fn path_wants_directory(line: &str) -> bool {
    let command = line.split([' ', '\t']).next().unwrap_or("");
    matches!(
        command.to_ascii_lowercase().as_str(),
        "cd" | "chdir" | "pushd" | "sl" | "set-location"
    )
}

#[cfg(test)]
mod suggest_tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct Fixture {
        history: Mutex<SltermHistory>,
        directories: DirectoryHistory,
        commands: Arc<Mutex<Vec<String>>>,
    }

    impl Fixture {
        /// 历史与 frecency 留空,断言只反映被测分支;commands 放只有 Windows
        /// 才有的名字,用来验证 PATH 来源。
        fn new() -> Self {
            Self {
                history: Mutex::new(SltermHistory::default()),
                directories: DirectoryHistory::empty(),
                commands: Arc::new(Mutex::new(vec![
                    "notepad.exe".to_owned(),
                    "grepwin.exe".to_owned(),
                ])),
            }
        }

        fn sources(&self, style: CompletionStyle) -> SuggestSources<'_> {
            SuggestSources {
                history: &self.history,
                directories: &self.directories,
                commands: &self.commands,
                enabled: true,
                style,
            }
        }

        fn ghost(&self, cwd: &str, line: &str) -> String {
            calculate(
                &self.sources(CompletionStyle::Inline),
                &Input {
                    cwd,
                    env: &SuggestEnv::Local,
                    line,
                },
                ShellSyntax::PowerShell,
                &|| false,
            )
            .suggestion
        }

        fn popup(&self, cwd: &str, line: &str) -> Vec<String> {
            calculate(
                &self.sources(CompletionStyle::Popup),
                &Input {
                    cwd,
                    env: &SuggestEnv::Local,
                    line,
                },
                ShellSyntax::PowerShell,
                &|| false,
            )
            .completion_items
            .into_iter()
            .map(|item| item.insert)
            .collect()
        }
    }

    #[test]
    fn current_directory_candidates_work_in_both_styles() {
        let temp = tempfile::tempdir().unwrap();
        for name in ["apple", "banana", "orange"] {
            std::fs::create_dir(temp.path().join(name)).unwrap();
        }
        for name in ["x.exe", "y.txt", "z.docx", "a.txt"] {
            std::fs::write(temp.path().join(name), b"").unwrap();
        }
        let cwd = temp.path().to_str().unwrap();
        let fixture = Fixture::new();
        for (input, expected) in [
            ("cd a", format!("pple{}", std::path::MAIN_SEPARATOR)),
            ("cd b", format!("anana{}", std::path::MAIN_SEPARATOR)),
            ("x", ".exe".into()),
            ("y", ".txt".into()),
        ] {
            assert_eq!(fixture.ghost(cwd, input), expected, "{input}");
            assert!(fixture.popup(cwd, input).contains(&expected), "{input}");
        }
        assert!(!fixture.popup(cwd, "cd a").contains(&".txt".into()));
    }

    #[test]
    fn history_hint_wins_before_command_and_paths() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("cargo-build.txt"), b"").unwrap();
        let fixture = Fixture::new();
        fixture
            .history
            .lock()
            .unwrap()
            .insert_for_test(&HistoryScope::Local, "cargo build --release");
        let cwd = temp.path().to_str().unwrap();
        assert_eq!(fixture.ghost(cwd, "cargo b"), "uild --release");
        assert!(
            fixture
                .popup(cwd, "cargo b")
                .contains(&"uild --release".to_owned())
        );
    }

    #[test]
    fn exact_command_suppresses_only_the_inline_remainder() {
        let commands = vec!["claude".to_owned(), "claude-agent-acp".to_owned()];
        assert_eq!(command_hint(&commands, "clau"), Some("de"));
        assert_eq!(command_hint(&commands, "claude"), None);
        assert_eq!(
            command_hints(&commands, "claude", 8),
            vec!["claude", "claude-agent-acp"]
        );
        assert_eq!(
            command_hints(&commands, "c", 2),
            vec!["claude", "claude-agent-acp"]
        );
    }

    #[test]
    fn program_identity_strips_paths_and_launcher_extensions() {
        assert_eq!(
            extract_program(r#"D:\tools\Claude.EXE --resume"#).as_deref(),
            Some("claude")
        );
        assert_eq!(
            extract_program("/usr/local/bin/cargo test").as_deref(),
            Some("cargo")
        );
        assert_eq!(extract_program(""), None);
    }

    #[test]
    fn command_and_directory_positions_are_explicit() {
        assert!(is_command_position("cargo"));
        assert!(!is_command_position("cargo test"));
        assert!(!is_command_position("./cargo"));
        assert!(path_wants_directory("cd src"));
        assert!(path_wants_directory("Set-Location src"));
        assert!(!path_wants_directory("cat src"));
    }

    #[test]
    fn filesystem_completion_does_not_hold_the_shared_history_lock() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("candidate.txt"), b"").unwrap();
        let fixture = Fixture::new();
        let checks = Cell::new(0);
        for style in [CompletionStyle::Inline, CompletionStyle::Popup] {
            let result = calculate(
                &fixture.sources(style),
                &Input {
                    cwd: temp.path().to_str().unwrap(),
                    env: &SuggestEnv::Local,
                    line: "cat ca",
                },
                ShellSyntax::PowerShell,
                &|| {
                    checks.set(checks.get() + 1);
                    assert!(
                        fixture.history.try_lock().is_ok(),
                        "directory work cannot block history submission"
                    );
                    false
                },
            );
            assert!(checks.get() > 3, "the real filesystem calculation must run");
            assert!(!result.suggestion.is_empty() || !result.completion_items.is_empty());
        }
    }

    #[test]
    fn cancellation_discards_every_source() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("candidate.txt"), b"").unwrap();
        let fixture = Fixture::new();
        fixture
            .history
            .lock()
            .unwrap()
            .insert_for_test(&HistoryScope::Local, "cargo build");
        for style in [CompletionStyle::Inline, CompletionStyle::Popup] {
            let result = calculate(
                &fixture.sources(style),
                &Input {
                    cwd: temp.path().to_str().unwrap(),
                    env: &SuggestEnv::Local,
                    line: "cargo b",
                },
                ShellSyntax::PowerShell,
                &|| true,
            );
            assert!(result.suggestion.is_empty());
            assert!(result.completion_items.is_empty());
            assert!(result.suggestion_edit.is_none());
        }
    }

    #[test]
    fn suggestion_key_tracks_line_style_and_command_generation() {
        let env = SuggestEnv::Local;
        let input = |line| Input {
            cwd: "D:/work",
            env: &env,
            line,
        };
        let base = suggestion_key(&input("cargo"), CompletionStyle::Inline, 0);
        assert_eq!(
            base,
            suggestion_key(&input("cargo"), CompletionStyle::Inline, 0)
        );
        assert_ne!(
            base,
            suggestion_key(&input("cargo t"), CompletionStyle::Inline, 0)
        );
        assert_ne!(
            base,
            suggestion_key(&input("cargo"), CompletionStyle::Popup, 0)
        );
        assert_ne!(
            base,
            suggestion_key(&input("cargo"), CompletionStyle::Inline, 3)
        );
    }

    #[test]
    fn semantic_spans_replace_only_the_divergent_tail_and_keep_utf8_whole() {
        let line = "git switch \"feature/中-old\" --quiet";
        let cursor = "git switch \"feature/中".len();
        let semantic = SemanticSuggestion::with_kind(
            slterm_completions::Suggestion {
                value: "feature/中文".into(),
                span: slterm_completions::Span::new(
                    "git switch \"".len(),
                    "git switch \"feature/中-old".len(),
                ),
                ..Default::default()
            },
            SuggestionKind::Command("git".into()),
        );
        let result = semantic_candidates_at(line, cursor, CompletionStyle::Popup, vec![semantic]);
        let item = &result.completion_items[0];
        let head: String = line[..cursor]
            .chars()
            .take(line[..cursor].chars().count() - item.replace_chars)
            .collect();
        let tail: String = line[cursor..]
            .chars()
            .skip(item.replace_after_chars)
            .collect();
        assert_eq!(
            format!("{head}{}{tail}", item.insert),
            "git switch \"feature/中文\" --quiet"
        );
    }

    #[test]
    fn ghost_length_is_bounded_by_the_chrome_budget() {
        let remainder = "x".repeat(SLTERM_GHOST_MAX + 56);
        assert_eq!(clamp_ghost(&remainder).chars().count(), SLTERM_GHOST_MAX);
    }

    #[test]
    fn left_elision_keeps_the_informative_tail() {
        assert_eq!(elide_left("short", 44), "short");
        let long = format!("{}{}", "a".repeat(50), "tail");
        let elided = elide_left(&long, 10);
        assert_eq!(elided.chars().count(), 10);
        assert!(elided.ends_with("tail"), "{elided}");
    }

    fn popup_state() -> SuggestState {
        let item = |label: &str, insert: &str| SltermCompletionItem {
            label: label.to_owned(),
            insert: insert.to_owned(),
            replace_chars: 0,
            replace_after_chars: 0,
            kind: SltermCompletionKind::Command,
        };
        SuggestState {
            completion_items: vec![
                item("git pull upstream", " upstream"),
                item("git pull --rebase", " --rebase"),
            ],
            ..SuggestState::default()
        }
    }

    #[test]
    fn popup_does_not_accept_before_explicit_navigation() {
        let mut state = popup_state();
        assert_eq!(state.completion_selected, None);
        assert!(state.completion_popup_take().is_none());
        assert!(!state.completion_items.is_empty(), "未导航不消耗列表");
    }

    #[test]
    fn first_popup_navigation_enters_at_the_first_item() {
        for delta in [-1, 1] {
            let mut state = popup_state();
            state.completion_popup_move(delta);
            assert_eq!(state.completion_selected, Some(0));
            assert_eq!(
                state.completion_popup_take().map(|item| item.insert),
                Some(" upstream".to_owned())
            );
            assert!(state.completion_items.is_empty());
        }
    }

    #[test]
    fn popup_navigation_wraps_in_both_directions() {
        let mut state = popup_state();
        state.completion_popup_move(1);
        state.completion_popup_move(1);
        assert_eq!(state.completion_selected, Some(1));
        state.completion_popup_move(1);
        assert_eq!(state.completion_selected, Some(0));
        state.completion_popup_move(-1);
        assert_eq!(state.completion_selected, Some(1));
    }

    #[test]
    fn escape_closes_the_list_and_keeps_the_line_suppressed() {
        let mut state = popup_state();
        assert!(state.completion_popup_dismiss("git pull"));
        assert_eq!(state.suppressed_line.as_deref(), Some("git pull"));
        assert!(state.completion_items.is_empty());
        // 行不变时不重开由刷新路径消费 suppressed_line 保证。
        assert!(!state.completion_popup_dismiss("git pull"));
    }

    #[test]
    fn hybrid_popup_request_clears_the_suppression() {
        let mut state = popup_state();
        state.suppressed_line = Some("git pull".into());
        state.suggestion = " upstream".into();
        assert!(state.request_completion_popup());
        assert!(state.popup_requested);
        assert!(state.suppressed_line.is_none());
        assert!(state.suggestion.is_empty(), "请求位清空旧候选");
    }

    #[test]
    fn query_keys_gate_stale_background_results() {
        let mut state = SuggestState::default();
        state.begin_completion_query("key-a".into());
        assert!(state.completion_query_matches("key-a"));
        assert!(!state.completion_query_ready("key-a"));
        state.finish_completion_query();
        assert!(state.completion_query_ready("key-a"));
        state.begin_completion_query("key-b".into());
        assert!(!state.completion_query_ready("key-a"));
    }

    /// pwsh 成功即采用其输出,不再探测 5.1;输出 trim 且丢弃空行。
    #[test]
    fn powershell_probe_prefers_pwsh_when_it_succeeds() {
        let calls = RefCell::new(Vec::new());
        let commands = powershell_commands_with(|program| {
            calls.borrow_mut().push(program.to_owned());
            (program == "pwsh").then(|| "Get-Item\n  Write-Host  \n\n".to_owned())
        });
        assert_eq!(calls.borrow().as_slice(), &["pwsh".to_owned()][..]);
        assert_eq!(
            commands,
            Some(vec!["Get-Item".to_owned(), "Write-Host".to_owned()])
        );
    }

    /// pwsh 启动失败/非零退出时按顺序回退 5.1 并返回其结果。
    #[test]
    fn powershell_probe_falls_back_to_windows_powershell() {
        let calls = RefCell::new(Vec::new());
        let commands = powershell_commands_with(|program| {
            calls.borrow_mut().push(program.to_owned());
            (program == "powershell").then(|| "Get-Date\nSet-Location\n".to_owned())
        });
        assert_eq!(
            calls.borrow().as_slice(),
            &["pwsh".to_owned(), "powershell".to_owned()][..]
        );
        assert_eq!(
            commands,
            Some(vec!["Get-Date".to_owned(), "Set-Location".to_owned()])
        );
    }

    /// 两者都失败时返回空,且两条探测路径都被尝试。
    #[test]
    fn powershell_probe_returns_empty_when_both_fail() {
        let calls = RefCell::new(Vec::new());
        let commands = powershell_commands_with(|program| {
            calls.borrow_mut().push(program.to_owned());
            None
        });
        assert!(commands.is_none());
        assert_eq!(
            calls.borrow().as_slice(),
            &["pwsh".to_owned(), "powershell".to_owned()][..]
        );
    }
}
