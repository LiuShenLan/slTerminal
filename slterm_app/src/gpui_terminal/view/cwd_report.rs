//! 目录元数据落地:不改写上报的文件系统路径本身。
//!
//! 标题协议前缀为 `SLTERM|`(core 侧 `osc_cwd`/集成脚本同源;上游同位
//! 协议已随全仓改名一并换锚)。信封形态 `SLTERM|<cwd>|<branch>|<program>`:
//! cwd 由 [`apply`] 摘取,branch/program 由 [`apply_title_metadata`] 摘取。

use slterm_terminal::event::Event;

/// 从事件里摘目录:`CwdReport`(OSC 7/9;9)与 `SLTERM|cwd|branch` 标题协议
/// 两条通道同构。返回是否真的变了。
pub(super) fn apply(current: &mut String, event: &Event) -> bool {
    let reported = match event {
        Event::CwdReport(path) => Some(path.as_str()),
        Event::Title(title) => title
            .strip_prefix("SLTERM|")
            .and_then(|rest| rest.split('|').next()),
        _ => None,
    };
    let Some(path) = reported else { return false };
    if path.chars().any(char::is_control)
        || !(path.starts_with('/') || std::path::Path::new(path).is_absolute())
        || current.as_str() == path
    {
        return false;
    }
    current.clear();
    current.push_str(path);
    true
}

/// 标题协议第二/三段:branch 原样 trim,空 program 视为无程序。返回是否
/// 有任何字段变化;OSC 7 目录上报不触碰这两项(两条通道并存)。
pub(super) fn apply_title_metadata(
    branch: &mut String,
    running_program: &mut Option<String>,
    event: &Event,
) -> bool {
    let Event::Title(title) = event else {
        return false;
    };
    let Some(rest) = title.strip_prefix("SLTERM|") else {
        return false;
    };
    let mut parts = rest.splitn(3, '|');
    parts.next();
    let next_branch = parts.next().unwrap_or("").trim().to_owned();
    let next_program = parts
        .next()
        .map(str::trim)
        .filter(|program| !program.is_empty())
        .map(str::to_owned);
    let branch_changed = branch.as_str() != next_branch;
    let program_changed = running_program.as_deref() != next_program.as_deref();
    if branch_changed {
        branch.clone_from(&next_branch);
    }
    if program_changed {
        *running_program = next_program;
    }
    branch_changed || program_changed
}

#[cfg(test)]
mod cwd_report_tests {
    use super::*;

    #[test]
    fn cwd_events_preserve_absolute_paths_verbatim() {
        for path in ["/", "/srv/Team's \"App\" ", "/home/guest/new project"] {
            let mut current = "/original".to_owned();
            let report = Event::CwdReport(path.to_owned());
            assert!(apply(&mut current, &report));
            assert_eq!(current, path);
            assert!(!apply(&mut current, &report));
        }
    }

    #[test]
    fn shell_title_events_preserve_the_directory_field() {
        let mut current = "/original".to_owned();
        let path = "/srv/Team's \"App\" ";
        assert!(apply(
            &mut current,
            &Event::Title(format!("SLTERM|{path}| main "))
        ));
        assert_eq!(current, path);
        assert!(!apply(
            &mut current,
            &Event::Title("editor title".to_owned())
        ));
        assert_eq!(current, path);
    }

    #[test]
    fn invalid_directory_events_keep_the_last_known_directory() {
        for path in [
            "",
            "relative",
            " /srv/app",
            "/srv/app\r",
            "\n/srv/app",
            "/srv/\x1bapp",
        ] {
            for event in [
                Event::CwdReport(path.to_owned()),
                Event::Title(format!("SLTERM|{path}|")),
            ] {
                let mut current = "/last known".to_owned();
                assert!(!apply(&mut current, &event), "{path:?}");
                assert_eq!(current, "/last known");
            }
        }
    }

    #[test]
    fn title_metadata_updates_branch_and_program() {
        let mut branch = String::new();
        let mut program = None;
        assert!(apply_title_metadata(
            &mut branch,
            &mut program,
            &Event::Title("SLTERM|/srv/app| feature/x | claude ".to_owned())
        ));
        assert_eq!(branch, "feature/x");
        assert_eq!(program.as_deref(), Some("claude"));

        // 同值不报变化;缺 program 段按空处理。
        assert!(!apply_title_metadata(
            &mut branch,
            &mut program,
            &Event::Title("SLTERM|/srv/app|feature/x|claude".to_owned())
        ));
        assert!(apply_title_metadata(
            &mut branch,
            &mut program,
            &Event::Title("SLTERM|/srv/app|feature/x|".to_owned())
        ));
        assert_eq!(program, None);
    }

    #[test]
    fn cwd_reports_leave_branch_and_program_untouched() {
        let mut branch = "main".to_owned();
        let mut program = Some("codex".to_owned());
        assert!(!apply_title_metadata(
            &mut branch,
            &mut program,
            &Event::CwdReport("/srv/other".to_owned())
        ));
        assert_eq!(branch, "main");
        assert_eq!(program.as_deref(), Some("codex"));
    }

    #[cfg(windows)]
    #[test]
    fn local_windows_directory_events_keep_drive_and_unc_paths() {
        for path in [r"C:\Users\Test User", r"\\server\share\project"] {
            let mut current = String::new();
            assert!(apply(&mut current, &Event::CwdReport(path.to_owned())));
            assert_eq!(current, path);
        }
    }
}
