use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};

use crate::tty::Options;

const MARK: &str = "\x1b]1337;SetUserVar=slterm_cmd_prompt=MQ==\x07";
const START: &str = "\x1b]133;A\x07";
const END: &str = "\x1b]133;B\x07\x1b]1337;SetUserVar=slterm_cmd_prompt=MQ==\x07";

pub(super) fn prepare(config: &Options) -> Options {
    let mut prepared = config.clone();
    let Some(shell) = &config.shell else {
        return prepared;
    };
    let name = shell
        .program()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default();
    if !name.eq_ignore_ascii_case("cmd") && !name.eq_ignore_ascii_case("cmd.exe") {
        return prepared;
    }

    let prompt = config
        .env
        .get("PROMPT")
        .map(OsStr::to_os_string)
        .or_else(|| {
            (!config.env_is_complete)
                .then(|| std::env::var_os("PROMPT"))
                .flatten()
        })
        .unwrap_or_else(|| OsString::from("$P$G"));
    prepared.env.remove("PROMPT");
    // 只改子进程的环境副本；嵌套启动或重复准备不能不断叠加标记。
    let prompt = if has_wide_prefix(&prompt, START) && has_wide_suffix(&prompt, END) {
        prompt
    } else {
        let prompt = strip_wide_prefix(&prompt, MARK);
        let mut wrapped = OsString::from(START);
        wrapped.push(prompt);
        wrapped.push(END);
        wrapped
    };
    prepared.env.insert("PROMPT", prompt);
    prepared
}

fn has_wide_prefix(value: &OsStr, prefix: &str) -> bool {
    let prefix: Vec<u16> = prefix.encode_utf16().collect();
    value.encode_wide().collect::<Vec<_>>().starts_with(&prefix)
}

fn has_wide_suffix(value: &OsStr, suffix: &str) -> bool {
    let suffix: Vec<u16> = suffix.encode_utf16().collect();
    value.encode_wide().collect::<Vec<_>>().ends_with(&suffix)
}

fn strip_wide_prefix(value: &OsStr, prefix: &str) -> OsString {
    let value: Vec<u16> = value.encode_wide().collect();
    let prefix: Vec<u16> = prefix.encode_utf16().collect();
    if value.starts_with(&prefix) {
        OsString::from_wide(&value[prefix.len()..])
    } else {
        OsString::from_wide(&value)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tty::Shell;

    fn cmd() -> Options {
        Options {
            shell: Some(Shell::new(
                "C:\\Windows\\System32\\CMD.EXE".into(),
                vec!["/d".into()],
            )),
            env_is_complete: true,
            ..Options::default()
        }
    }

    #[test]
    fn native_prompt_preserves_custom_text_without_mutating_caller() {
        let mut config = cmd();
        config.env.insert("Prompt", "[$P]$_$G");
        let prepared = prepare(&config);
        assert_eq!(
            prepared.env.get("PROMPT"),
            Some(OsStr::new(&format!("{START}[$P]$_$G{END}")))
        );
        let prompt_names: Vec<_> = prepared
            .env
            .keys()
            .filter(|name| name.eq_ignore_ascii_case("PROMPT"))
            .collect();
        assert_eq!(prompt_names, [OsStr::new("PROMPT")]);
        assert_eq!(config.env.get("Prompt").unwrap(), "[$P]$_$G");
    }

    #[test]
    fn native_prompt_defaults_and_is_idempotent() {
        let prepared = prepare(&cmd());
        assert_eq!(
            prepared.env.get("PROMPT"),
            Some(OsStr::new(&format!("{START}$P$G{END}")))
        );
        assert_eq!(prepare(&prepared), prepared);
    }

    #[test]
    fn native_prompt_does_not_instrument_other_shells() {
        for name in ["powershell.exe", "pwsh", "wsl.exe", "mycmd.exe"] {
            let mut config = cmd();
            config.shell = Some(Shell::new(name.into(), vec![]));
            assert_eq!(prepare(&config), config);
        }
        let mut config = cmd();
        config.shell = None;
        assert_eq!(prepare(&config), config);
    }

    #[test]
    fn native_prompt_preserves_explicit_empty_prompt() {
        let mut config = cmd();
        config.env.insert("PROMPT", String::new());
        assert_eq!(
            prepare(&config).env.get("PROMPT"),
            Some(OsStr::new(&format!("{START}{END}")))
        );
    }

    #[test]
    fn native_prompt_upgrades_a_prefix_only_marker_without_duplicating_it() {
        let mut config = cmd();
        config.env.insert("PROMPT", format!("{MARK}[$P]$S"));
        let prepared = prepare(&config);
        assert_eq!(
            prepared.env.get("PROMPT"),
            Some(OsStr::new(&format!("{START}[$P]$S{END}")))
        );
        assert_eq!(prepare(&prepared), prepared);
    }
}
