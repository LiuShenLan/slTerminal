//! 本机路径拼写辅助:补全匹配的大小写语义与回写路径分隔符形态。
//!
//! 匹配与引用分离纪律:本模块只决定「候选值如何拼写」;shell 的引用与
//! 转义合同唯一归 `slterm_completions::command_context::CommandContext::candidate`
//! (08 篇边界 3),此处不复制第二套。

use slterm_completions::command_context::ShellSyntax;

/// 文件补全匹配是否区分大小写。Windows 文件系统不区分,匹配层按不敏感折叠,
/// 才能补出与磁盘大小写不同的既有条目。
pub(crate) fn completion_case_sensitive() -> bool {
    !cfg!(windows)
}

/// 候选路径回写拼写:Windows 原生程序重建 argv 时,CMD 与 PowerShell 对
/// 反斜杠/引号的处理不同,统一用 `/` 表示目录分隔符可绕开两侧歧义。
///
/// PowerShell 仅在有空白时替换:无空白的 `C:\dir\file` 保持原样更贴近用户
/// 已经键入的形态。
pub(crate) fn completion_spelling(path: &str, syntax: ShellSyntax) -> String {
    if cfg!(windows)
        && (matches!(syntax, ShellSyntax::Posix | ShellSyntax::Cmd)
            || syntax == ShellSyntax::PowerShell && path.chars().any(char::is_whitespace))
    {
        path.replace('\\', "/")
    } else {
        path.to_owned()
    }
}

#[cfg(test)]
mod local_paths_tests {
    use super::*;

    #[test]
    fn posix_and_cmd_spelling_uses_forward_slashes_on_windows() {
        if !cfg!(windows) {
            return;
        }
        let path = r"repo name\child";
        assert_eq!(
            completion_spelling(path, ShellSyntax::Posix),
            "repo name/child"
        );
        assert_eq!(
            completion_spelling(path, ShellSyntax::Cmd),
            "repo name/child"
        );
        assert_eq!(
            completion_spelling(path, ShellSyntax::PowerShell),
            "repo name/child"
        );
        assert_eq!(
            completion_spelling(r"plain\child", ShellSyntax::PowerShell),
            r"plain\child"
        );
        assert_eq!(completion_spelling(path, ShellSyntax::Literal), path);
    }

    #[test]
    fn case_sensitivity_follows_the_host_filesystem() {
        assert_eq!(completion_case_sensitive(), !cfg!(windows));
    }
}
