//! 本地 pane 的 Agent 身份契约。
//!
//! 让 pane 里的任何进程——尤其是 Codex / Claude Code 这类 AI CLI——在不读源码、
//! 不扫进程、不猜端口的前提下回答三个问题:
//!
//! | 问题 | 变量 |
//! | --- | --- |
//! | 我在什么终端里? | `TERM_PROGRAM` / `TERM_PROGRAM_VERSION` |
//! | 我是哪个 pane? | `SLTERM_PANE_ID` |
//! | 控制面在哪、怎么调? | `SLTERM_CLI` / `SLTERM_BIN_DIR` / `PATH` |
//!
//! 前两行是**识别**,第三行是**可达性**,两者都不解决「什么时候该调」——那是
//! Skill 的职责。三者不能互相替代:没有身份契约,Skill 里写的命令是空头承诺;
//! 没有 Skill,Agent 至少还能靠 `slterm env` 自己发现完整能力清单。
//!
//! # 只进本地 PTY
//!
//! 这些变量只注入本机 PTY。SSH/WSL 臂随平台裁剪整支不迁,`WSLENV` 合并
//! 与 `SLTERM_PANE_REMOTE` 常量不立:远程 pane 语义无对象。
//!
//! # 幂等
//!
//! 在 slTerminal 的 pane 里再开一个 slTerminal 是常规操作(嵌套 shell、
//! 测试用的隔离实例)。因此 `PATH` 前置按值判重:套多少层,环境块都不增长。

use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt::Display;
use std::path::{Path, PathBuf};

pub(crate) use crate::ai_hook::PANE_ENV;

/// `TERM_PROGRAM` 的取值。这是程序判断「我跑在哪个终端里」的事实标准入口,
/// 终端生态普遍使用同一个变量,所以第三方工具的既有识别逻辑不需要为
/// slTerminal 改代码。
pub const TERM_PROGRAM: &str = "slterm";

/// slTerminal 可执行文件的绝对路径。
///
/// 便携版(解压即用)不一定在 `PATH` 上,而 Agent 需要的是「现在就能执行」,
/// 不是「也许装过」。给精确路径就不必让模型去猜安装位置。
pub const CLI_ENV: &str = "SLTERM_CLI";

/// [`CLI_ENV`] 所在目录,同时被前置到 `PATH`,这样 `slterm` 裸命令也可用。
pub const BIN_DIR_ENV: &str = "SLTERM_BIN_DIR";

/// 当前 slTerminal 进程的 PID。和 pane id 一起构成进程归属,避免两个同时
/// 运行的实例各自的 pane 1 被身份探测混在一起。
pub const PROCESS_ENV: &str = "SLTERM_PROCESS_ID";

const TERM_PROGRAM_ENV: &str = "TERM_PROGRAM";
const TERM_PROGRAM_VERSION_ENV: &str = "TERM_PROGRAM_VERSION";
const PATH_ENV: &str = "PATH";

/// 把身份契约写进一个本地 PTY 的环境表。
///
/// 调用点必须在所有其它 `env` 写入之后:本函数以环境表里的 `PATH` 现值为基准
/// 前置 bin 目录,与调用顺序无关地保住已有条目。
pub fn apply(env: &mut HashMap<String, String>, pane_id: impl Display) {
    insert_env(env, PANE_ENV, pane_id.to_string());
    insert_env(env, PROCESS_ENV, std::process::id().to_string());
    insert_env(env, TERM_PROGRAM_ENV, TERM_PROGRAM.to_owned());
    insert_env(
        env,
        TERM_PROGRAM_VERSION_ENV,
        env!("CARGO_PKG_VERSION").to_owned(),
    );

    if let Some(executable) = executable() {
        insert_env(env, CLI_ENV, executable.display().to_string());
        if let Some(directory) = executable.parent() {
            insert_env(env, BIN_DIR_ENV, directory.display().to_string());
            if let Some(path) = prepended_path(directory, env_value(env, PATH_ENV)) {
                insert_env(env, PATH_ENV, path);
            }
        }
    }

    // 端点注入必须排在末位(04 篇改造节 2):继承来的旧端点一律作废,
    // 子进程只指向「正在为它提供控制面的那个实例」。
    crate::runtime_api::apply_child_endpoint(env);
}

fn env_value<'a>(env: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    env.iter()
        .find(|(existing, _)| existing.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn insert_env(env: &mut HashMap<String, String>, name: &str, value: String) {
    // Windows 环境变量名大小写不敏感:`Path` 与 `PATH` 是同一个变量,不先
    // 移除等值键就会留下两份、后写者未必是读取方看到的那份。
    env.retain(|existing, _| !existing.eq_ignore_ascii_case(name));
    env.insert(name.to_owned(), value);
}

/// 当前运行的 slTerminal 可执行文件。
///
/// 优先 `current_exe()` 而不是继承来的 [`CLI_ENV`]:嵌套启动时,继承值指向
/// 外层那个二进制,可能已经是旧版本或另一个便携副本;PTY 应当指向**正在为
/// 它提供控制面的那个**进程。
pub fn executable() -> Option<PathBuf> {
    preferred_executable(
        std::env::current_exe().ok(),
        std::env::var_os(CLI_ENV).map(PathBuf::from),
    )
}

fn preferred_executable(current: Option<PathBuf>, inherited: Option<PathBuf>) -> Option<PathBuf> {
    current.or(inherited)
}

/// 把 `directory` 挪到 `PATH` 首位,返回新值;已经在首位时返回 `None`。
///
/// 先移除所有等值项再前置,所以重复调用是稳定的——否则每层嵌套都会给环境块
/// 多加一份同样的目录。
fn prepended_path(directory: &Path, current: Option<&str>) -> Option<String> {
    let current = match current {
        Some(value) => OsString::from(value),
        None => std::env::var_os(PATH_ENV).unwrap_or_default(),
    };
    let mut entries: Vec<PathBuf> = std::env::split_paths(&current).collect();
    if entries
        .first()
        .is_some_and(|first| same_directory(first, directory))
    {
        return None;
    }
    entries.retain(|entry| !same_directory(entry, directory));
    entries.insert(0, directory.to_path_buf());
    // `join_paths` 在某条目自带分隔符时失败。那种 `PATH` 本来就已经坏了,
    // 保留原值比写回一个被截断的列表安全。
    std::env::join_paths(entries)
        .ok()
        .map(|joined| joined.to_string_lossy().into_owned())
}

fn same_directory(left: &Path, right: &Path) -> bool {
    // Windows 路径大小写不敏感:`D:\slTerminal` 与 `d:\slterm` 是同一个
    // 目录,不归一化判重就等于没判。
    left.as_os_str().eq_ignore_ascii_case(right.as_os_str())
}

#[cfg(test)]
mod env_contract_tests {
    use super::*;

    fn applied(pane_id: u64) -> HashMap<String, String> {
        let mut env = HashMap::new();
        apply(&mut env, pane_id);
        env
    }

    #[test]
    fn identity_group_is_exact_and_legacy_names_are_absent() {
        let env = applied(17);
        let executable = executable().expect("测试二进制的 current_exe 一定存在");
        let bin_dir = executable.parent().expect("可执行文件必有父目录");
        assert_eq!(env.get(PANE_ENV).map(String::as_str), Some("17"));
        assert_eq!(
            env.get(PROCESS_ENV).map(String::as_str),
            Some(std::process::id().to_string().as_str())
        );
        assert_eq!(
            env.get(CLI_ENV).map(String::as_str),
            Some(executable.to_string_lossy().as_ref())
        );
        assert_eq!(
            env.get(BIN_DIR_ENV).map(String::as_str),
            Some(bin_dir.to_string_lossy().as_ref())
        );
        assert_eq!(
            env.get(TERM_PROGRAM_ENV).map(String::as_str),
            Some(TERM_PROGRAM)
        );
        assert_eq!(
            env.get(TERM_PROGRAM_VERSION_ENV).map(String::as_str),
            Some(env!("CARGO_PKG_VERSION"))
        );
        // 旧品牌名与远程 pane 常量不立,环境组不许多出任何身份键。
        for legacy in [
            "NEBULA_PANE_ID",
            "NEBULA_CLI",
            "NEBULA_BIN_DIR",
            "PEBREL_CLI",
            "SLTERM_PANE_REMOTE",
        ] {
            assert!(!env.contains_key(legacy), "{legacy} 不应存在");
        }
        let expected: Vec<&str> = vec![
            PANE_ENV,
            PROCESS_ENV,
            CLI_ENV,
            BIN_DIR_ENV,
            TERM_PROGRAM_ENV,
            TERM_PROGRAM_VERSION_ENV,
        ];
        for key in expected {
            assert!(env.contains_key(key), "{key} 缺失");
        }
    }

    #[test]
    fn cli_directory_leads_path() {
        let env = applied(1);
        let bin_dir = PathBuf::from(env.get(BIN_DIR_ENV).expect("bin dir"));
        let path = env.get(PATH_ENV).expect("PATH 缺少 bin 目录时应被重写");
        let first = std::env::split_paths(path)
            .next()
            .expect("first PATH entry");
        assert!(
            same_directory(&first, &bin_dir),
            "{first:?} should lead PATH"
        );
    }

    #[test]
    fn path_prepend_is_idempotent_and_does_not_duplicate_entries() {
        let directory = Path::new(r"D:\slTerminal\bin");
        let base = std::env::join_paths([r"D:\tools", r"D:\Windows"])
            .expect("join")
            .to_string_lossy()
            .into_owned();

        let once = prepended_path(directory, Some(base.as_str())).expect("首次调用改写 PATH");
        // 已在首位 → 不再重写,嵌套启动因此不会让环境块增长。
        assert_eq!(prepended_path(directory, Some(once.as_str())), None);

        // 目录在中段时移到首位,且不留下重复项。
        let middle = std::env::join_paths([r"D:\tools", r"D:\SLTERMINAL\BIN", r"D:\Windows"])
            .expect("join")
            .to_string_lossy()
            .into_owned();
        let moved = prepended_path(directory, Some(middle.as_str())).expect("非首位条目应被前移");
        let entries: Vec<PathBuf> = std::env::split_paths(&moved).collect();
        assert!(same_directory(&entries[0], directory));
        assert_eq!(
            entries
                .iter()
                .filter(|entry| same_directory(entry, directory))
                .count(),
            1
        );
    }

    #[test]
    fn apply_twice_keeps_the_environment_block_stable() {
        let mut env = HashMap::new();
        apply(&mut env, 4);
        let once = env.get(PATH_ENV).cloned();
        let keys = env.len();
        apply(&mut env, 4);
        assert_eq!(env.get(PATH_ENV).cloned(), once, "PATH 不应重复增长");
        assert_eq!(env.len(), keys, "重复 apply 不新增键");
    }

    #[test]
    fn windows_path_spelling_is_normalized_before_deduplication() {
        if !cfg!(windows) {
            return;
        }
        let directory = Path::new(r"C:\Tools\slTerminal");
        let base = std::env::join_paths([r"C:\a", r"C:\TOOLS\SLTERMINAL", r"C:\b"])
            .expect("join")
            .to_string_lossy()
            .into_owned();
        let moved = prepended_path(directory, Some(base.as_str())).expect("应前移");
        let entries: Vec<PathBuf> = std::env::split_paths(&moved).collect();
        assert_eq!(entries.len(), 3, "大小写不同的等值目录只留一份");
        assert!(same_directory(&entries[0], directory));
    }

    #[test]
    fn current_exe_wins_over_an_inherited_cli_path() {
        let current = PathBuf::from(r"D:\slTerminal\slterm.exe");
        let inherited = PathBuf::from(r"D:\old-copy\slterm.exe");
        assert_eq!(
            preferred_executable(Some(current.clone()), Some(inherited)),
            Some(current)
        );
        assert_eq!(
            preferred_executable(None, Some(PathBuf::from(r"D:\old-copy\slterm.exe"))),
            Some(PathBuf::from(r"D:\old-copy\slterm.exe"))
        );
        assert_eq!(preferred_executable(None, None), None);
    }
}
