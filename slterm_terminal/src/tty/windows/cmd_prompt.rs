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
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("PROMPT"))
        .map(|(_, value)| value.clone())
        .or_else(|| {
            (!config.env_is_complete)
                .then(|| std::env::var("PROMPT").ok())
                .flatten()
        })
        .unwrap_or_else(|| "$P$G".into());
    prepared
        .env
        .retain(|key, _| !key.eq_ignore_ascii_case("PROMPT"));
    // 只改子进程的环境副本；嵌套启动或重复准备不能不断叠加标记。
    let prompt = if prompt.starts_with(START) && prompt.ends_with(END) {
        prompt
    } else {
        let prompt = prompt.strip_prefix(MARK).unwrap_or(&prompt);
        format!("{START}{prompt}{END}")
    };
    prepared.env.insert("PROMPT".into(), prompt);
    prepared
}
