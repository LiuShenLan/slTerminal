//! 安装渠道门禁(09 篇 M10.5 采纳点 33):外部托管渠道不参与自更新。
//!
//! 渠道只收窄自动更新能力,不改变用户设置、插件或终端热路径。探测结果
//! 进程内缓存一次:安装形态在进程生命周期内不会变化。
//!
//! slTerminal 当前分发形态 = 手工 zip(`.claude/package.ps1`)与将来 Inno
//! setup.exe;两者都无标记文件 → `Direct`。标记文件 `slterm-distribution`
//! 是与可执行文件同目录的旁置文件:内容是 `scoop` 时归 Scoop,其余任何
//! 内容(含损坏/超长)归 `Unrecognized`——未知标记不得悄悄切回普通安装器。

use std::path::Path;
use std::sync::OnceLock;

/// 分发渠道。`Unrecognized` 表示「有标记但无法识别」,比缺省更严格。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Distribution {
    /// 自管安装(便携 zip / 官方 setup.exe):允许自动更新。
    Direct,
    /// Scoop 外部包管理器:更新归 `scoop update`。
    Scoop,
    /// 标记存在但无法识别:视为外部托管,拒绝自更新。
    Unrecognized,
}

impl Distribution {
    /// 非 `Direct` 一律不参与应用自更新(下载与 handoff 闸门共用)。
    pub(crate) fn externally_managed(self) -> bool {
        self != Self::Direct
    }
}

/// 进程内单次探测;设置页与更新链读同一个不可变值。
pub(crate) fn current() -> Distribution {
    static CHANNEL: OnceLock<Distribution> = OnceLock::new();
    *CHANNEL.get_or_init(detect)
}

/// 更新链的统一闸门:外部托管渠道返回可展示的拒绝原因。
pub(crate) fn require_direct_update() -> Result<(), String> {
    require_direct(current())
}

/// 纯函数形态:测试不依赖进程级 `OnceLock`。
fn require_direct(distribution: Distribution) -> Result<(), String> {
    match distribution {
        Distribution::Direct => Ok(()),
        Distribution::Scoop => {
            Err("该安装由 Scoop 管理,请使用 scoop update slterminal 更新".into())
        }
        Distribution::Unrecognized => Err("无法确认安装归属;请通过原始分发渠道更新".into()),
    }
}

fn detect() -> Distribution {
    #[cfg(windows)]
    {
        std::env::current_exe()
            .and_then(std::fs::canonicalize)
            .map(|executable| from_marker(&executable))
            .unwrap_or(Distribution::Unrecognized)
    }
    #[cfg(not(windows))]
    Distribution::Direct
}

#[cfg(any(windows, test))]
fn from_marker(executable: &Path) -> Distribution {
    use std::io::Read as _;

    let Some(directory) = executable.parent() else {
        return Distribution::Unrecognized;
    };
    let marker = directory.join("slterm-distribution");
    let file = match std::fs::File::open(marker) {
        Ok(file) => file,
        // 无标记 = 自管安装(便携 zip / setup.exe 均无标记)。
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Distribution::Direct;
        }
        Err(_) => return Distribution::Unrecognized,
    };
    // 有界读取:标记内容只可能是几字节;超长/读失败都按无法识别处理。
    let mut bytes = Vec::with_capacity(33);
    if file.take(33).read_to_end(&mut bytes).is_err() {
        return Distribution::Unrecognized;
    }
    parse_marker(&bytes)
}

#[cfg(any(windows, test))]
fn parse_marker(bytes: &[u8]) -> Distribution {
    if bytes.len() <= 32 && bytes.trim_ascii() == b"scoop" {
        Distribution::Scoop
    } else {
        // 未知或损坏标记不得回退成 Direct:标记只收窄更新能力。
        Distribution::Unrecognized
    }
}

#[cfg(test)]
mod distribution_tests {
    use super::{Distribution, from_marker, parse_marker, require_direct};

    #[test]
    fn only_explicit_scoop_marker_is_recognized() {
        for value in [b"scoop".as_slice(), b"scoop\r\n", b" scoop "] {
            assert_eq!(parse_marker(value), Distribution::Scoop);
        }
        for value in [b"".as_slice(), b"installer", b"Scoop", &[b' '; 33]] {
            assert_eq!(parse_marker(value), Distribution::Unrecognized);
        }
    }

    #[test]
    fn ownership_is_local_to_installation_not_directory_name() {
        let root = tempfile::tempdir().unwrap();
        let executable = root.path().join("slterm.exe");
        assert_eq!(from_marker(&executable), Distribution::Direct);
        std::fs::write(root.path().join("slterm-distribution"), b"scoop\n").unwrap();
        assert_eq!(from_marker(&executable), Distribution::Scoop);
        std::fs::write(root.path().join("slterm-distribution"), b"other").unwrap();
        assert_eq!(from_marker(&executable), Distribution::Unrecognized);
    }

    #[test]
    fn external_channels_never_own_setup_updates() {
        assert!(!Distribution::Direct.externally_managed());
        assert!(require_direct(Distribution::Direct).is_ok());
        for channel in [Distribution::Scoop, Distribution::Unrecognized] {
            assert!(channel.externally_managed());
            assert!(require_direct(channel).is_err());
        }
    }
}
