use std::fmt;
use std::str::FromStr;

/// 侧栏逻辑视图的封闭身份。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SideViewId {
    Nav,
    Explorer,
    Commit,
    AgentFiles,
}

/// 持久文本未命中已登记的侧栏视图。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownSideViewId;

impl SideViewId {
    /// 单一注册点与确定顺序。
    pub const ALL: [Self; 4] = [Self::Nav, Self::Explorer, Self::Commit, Self::AgentFiles];

    /// 返回规范持久文本。
    pub const fn id_str(self) -> &'static str {
        match self {
            Self::Nav => "nav",
            Self::Explorer => "explorer",
            Self::Commit => "commit",
            Self::AgentFiles => "agent_files",
        }
    }
}

impl FromStr for SideViewId {
    type Err = UnknownSideViewId;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "nav" => Ok(Self::Nav),
            "explorer" => Ok(Self::Explorer),
            "commit" => Ok(Self::Commit),
            "agent_files" => Ok(Self::AgentFiles),
            _ => Err(UnknownSideViewId),
        }
    }
}

impl fmt::Display for SideViewId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id_str())
    }
}

impl fmt::Display for UnknownSideViewId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("未知侧栏视图类型")
    }
}

impl std::error::Error for UnknownSideViewId {}

#[cfg(test)]
mod side_view_tests {
    use super::*;

    /// 四种视图共用单一注册顺序与规范小写文本。
    #[test]
    fn side_view_ids_round_trip_canonical_text() {
        let registered = [
            ("nav", SideViewId::Nav),
            ("explorer", SideViewId::Explorer),
            ("commit", SideViewId::Commit),
            ("agent_files", SideViewId::AgentFiles),
        ];
        assert_eq!(
            SideViewId::ALL,
            [
                SideViewId::Nav,
                SideViewId::Explorer,
                SideViewId::Commit,
                SideViewId::AgentFiles,
            ]
        );
        for (text, id) in registered {
            assert_eq!(id.id_str(), text);
            assert_eq!(id.to_string(), text);
            assert_eq!(SideViewId::from_str(text), Ok(id));
        }
    }

    /// 未登记、非规范与旧别名文本均不得成为新身份。
    #[test]
    fn side_view_id_rejects_unknown_and_noncanonical_text() {
        for text in [
            "",
            "NAV",
            "agentFiles",
            "agent-files",
            " nav",
            "agent_files ",
            "unknown",
        ] {
            assert_eq!(SideViewId::from_str(text), Err(UnknownSideViewId), "{text}");
        }
    }

    /// 未知类型错误可经标准错误接口传递。
    #[test]
    fn unknown_side_view_id_error_describes_the_rejection() {
        fn require_error<T: std::error::Error>() {}
        require_error::<UnknownSideViewId>();
        assert_eq!(UnknownSideViewId.to_string(), "未知侧栏视图类型");
    }
}
