//! 布局转换对:「布局单点」唯一落点(05 篇 M5.2)。
//!
//! 持久化布局只经 session schema 存取;运行时 `SplitTree<PaneId>` 与持久化
//! `LayoutSession` 之间的全部形状转换只在本模块的一对纯函数里发生。其他
//! 模块禁直接构造/解构持久化树——文件末的门禁用例机械断言这一步界。
//!
//! 消费点 = M5.5 快照(`snapshot_session`)与启动恢复(`restore_tab`)。

use crate::session::{AgentSession, LaunchSession, LayoutSession, SplitAxis};
use slterm_split::{PaneId, SplitDirection, SplitTree};

fn axis_of(direction: SplitDirection) -> SplitAxis {
    match direction {
        SplitDirection::LeftRight => SplitAxis::LeftRight,
        SplitDirection::TopBottom => SplitAxis::TopBottom,
    }
}

fn direction_of(axis: SplitAxis) -> SplitDirection {
    match axis {
        SplitAxis::LeftRight => SplitDirection::LeftRight,
        SplitAxis::TopBottom => SplitDirection::TopBottom,
    }
}

/// 运行时分屏树 → 持久化树。`leaf_data` 按 pane id 供叶数据(cwd、快照
/// 瞬间的 AI 会话、启动身份、重命名);比例先钳 0.05..0.95 再四舍五入成
/// 千分比整数——自动保存的变化检测与文件 diff 不被 f32 序列化噪声绊倒。
pub(crate) fn layout_from_tree(
    tree: &SplitTree<PaneId>,
    leaf_data: &impl Fn(
        PaneId,
    ) -> (
        String,
        Option<AgentSession>,
        Option<LaunchSession>,
        Option<String>,
    ),
) -> LayoutSession {
    match tree {
        SplitTree::Leaf(id) => {
            let (cwd, agent, launch, custom_name) = leaf_data(*id);
            LayoutSession::Pane {
                cwd,
                custom_name,
                agent,
                launch,
            }
        }
        SplitTree::Split {
            direction,
            ratio,
            first,
            second,
            ..
        } => LayoutSession::Split {
            axis: axis_of(*direction),
            ratio_permille: (ratio.clamp(0.05, 0.95) * 1000.0).round() as u16,
            first: Box::new(layout_from_tree(first, leaf_data)),
            second: Box::new(layout_from_tree(second, leaf_data)),
        },
    }
}

/// 持久化树 → 运行时分屏树。`alloc` 按深度优先序逐叶分配 id,返回的 id
/// 列表与 [`LayoutSession::leaves`] 的深度优先序一一对应——恢复注入
/// (cwd/agent → 第 i 个 pane)靠该配对。比例千分比 → f32 同带钳制。
pub(crate) fn tree_from_layout(
    layout: &LayoutSession,
    alloc: &mut impl FnMut() -> PaneId,
) -> (SplitTree<PaneId>, Vec<PaneId>) {
    fn walk(
        node: &LayoutSession,
        alloc: &mut impl FnMut() -> PaneId,
        ids: &mut Vec<PaneId>,
    ) -> SplitTree<PaneId> {
        match node {
            LayoutSession::Pane { .. } => {
                let id = alloc();
                ids.push(id);
                SplitTree::Leaf(id)
            }
            LayoutSession::Split {
                axis,
                ratio_permille,
                first,
                second,
            } => SplitTree::Split {
                direction: direction_of(*axis),
                ratio: (f32::from(*ratio_permille) / 1000.0).clamp(0.05, 0.95),
                preview_ratio: None,
                dragging: false,
                first: Box::new(walk(first, alloc, ids)),
                second: Box::new(walk(second, alloc, ids)),
            },
        }
    }

    let mut ids = Vec::new();
    let tree = walk(layout, alloc, &mut ids);
    (tree, ids)
}

#[cfg(test)]
mod session_restore_tests {
    use super::*;

    fn sample_tree() -> SplitTree<PaneId> {
        let mut tree = SplitTree::leaf(PaneId(1));
        assert!(tree.split_leaf(PaneId(1), PaneId(2), SplitDirection::LeftRight, 0.618));
        assert!(tree.split_leaf(PaneId(2), PaneId(3), SplitDirection::TopBottom, 0.5));
        tree
    }

    #[test]
    fn layout_round_trips_structure_ratio_and_leaf_order() {
        let tree = sample_tree();
        let layout = layout_from_tree(&tree, &|id| (format!("D:/pane-{}", id.0), None, None, None));
        assert_eq!(layout.pane_count(), 3);
        assert_eq!(layout.first_cwd(), "D:/pane-1");

        let mut next = 10u64;
        let (rebuilt, ids) = tree_from_layout(&layout, &mut || {
            next += 1;
            PaneId(next)
        });
        // 深度优先叶序保持:源树 leaves()、返回 ids 与重建树 leaves() 一致配对。
        assert_eq!(ids, vec![PaneId(11), PaneId(12), PaneId(13)]);
        assert_eq!(rebuilt.leaves(), ids);
        let cwds: Vec<_> = layout
            .leaves()
            .iter()
            .map(|leaf| match leaf {
                LayoutSession::Pane { cwd, .. } => cwd.as_str(),
                LayoutSession::Split { .. } => unreachable!("leaves 只返回叶"),
            })
            .collect();
        assert_eq!(cwds, ["D:/pane-1", "D:/pane-2", "D:/pane-3"]);

        // 比例经千分比往返,误差 ≤ 0.001。
        let LayoutSession::Split { ratio_permille, .. } = &layout else {
            panic!("根应是 Split");
        };
        assert_eq!(*ratio_permille, 618);
        let SplitTree::Split { ratio, .. } = &rebuilt else {
            panic!("根应是 Split");
        };
        assert!((ratio - 0.618).abs() < 0.001);
    }

    /// 叶数据闭包按 pane id 注入,agent 等字段落在所属叶上。
    #[test]
    fn leaf_data_rides_on_the_owning_leaf() {
        let tree = SplitTree::leaf(PaneId(7));
        let layout = layout_from_tree(&tree, &|id| {
            assert_eq!(id, PaneId(7));
            (
                "D:/work".to_owned(),
                Some(AgentSession {
                    source: "claude".into(),
                    session_id: Some("abc-1".into()),
                    session_file: None,
                }),
                Some(LaunchSession::Default),
                Some("前端".into()),
            )
        });
        let LayoutSession::Pane {
            cwd,
            custom_name,
            agent,
            launch,
        } = layout
        else {
            panic!("单叶必须是 Pane");
        };
        assert_eq!(cwd, "D:/work");
        assert_eq!(custom_name.as_deref(), Some("前端"));
        assert_eq!(launch, Some(LaunchSession::Default));
        assert!(
            agent.as_ref().is_some_and(|agent| agent.source == "claude"),
            "agent 快照必须落在所属叶"
        );
    }

    /// 两侧带钳同值:0.05..0.95 是持久化可恢复范围的边界。
    #[test]
    fn ratios_clamp_to_the_restorable_band() {
        for (ratio, permille) in [(0.0, 50), (0.0499, 50), (0.999, 950), (1.0, 950)] {
            let tree = SplitTree::Split {
                direction: SplitDirection::LeftRight,
                ratio,
                preview_ratio: None,
                dragging: false,
                first: Box::new(SplitTree::leaf(PaneId(1))),
                second: Box::new(SplitTree::leaf(PaneId(2))),
            };
            let layout = layout_from_tree(&tree, &|_| (String::new(), None, None, None));
            let LayoutSession::Split { ratio_permille, .. } = layout else {
                panic!("根应是 Split");
            };
            assert_eq!(ratio_permille, permille, "输入比例 {ratio}");
        }

        for (permille, ratio) in [(0, 0.05), (65535, 0.95)] {
            let layout = LayoutSession::Split {
                axis: SplitAxis::TopBottom,
                ratio_permille: permille,
                first: Box::new(LayoutSession::Pane {
                    cwd: String::new(),
                    custom_name: None,
                    agent: None,
                    launch: None,
                }),
                second: Box::new(LayoutSession::Pane {
                    cwd: String::new(),
                    custom_name: None,
                    agent: None,
                    launch: None,
                }),
            };
            let (tree, _) = tree_from_layout(&layout, &mut || PaneId(1));
            let SplitTree::Split { ratio: rebuilt, .. } = tree else {
                panic!("根应是 Split");
            };
            assert!((rebuilt - ratio).abs() < f32::EPSILON, "千分比 {permille}");
        }
    }

    /// 「全仓唯一摸持久化树形状」grep 门禁(05 篇 M5.2 出口):持久化布局树
    /// 的构造/解构只允许出现在 schema 模块与转换对文件里。这是「布局单点」
    /// 的机械保障;新消费方若需要叶数据,应经 schema 的访问方法,而不是
    /// 手写变体匹配。
    #[test]
    fn persisted_layout_shape_is_confined_to_the_schema_and_conversion_pair() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = manifest.parent().expect("workspace root");
        let allowed = [
            manifest.join("src").join("session.rs"),
            manifest
                .join("src")
                .join("app_shell")
                .join("session_restore.rs"),
        ];
        let patterns = ["LayoutSession::Pane", "LayoutSession::Split"];

        let mut offenders = Vec::new();
        for path in rust_sources(root) {
            if allowed.contains(&path) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for (index, line) in text.lines().enumerate() {
                if patterns.iter().any(|pattern| line.contains(pattern)) {
                    offenders.push(format!("{}:{}: {}", path.display(), index + 1, line.trim()));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "持久化布局树形状只允许出现在 session.rs 与 session_restore.rs:\n{}",
            offenders.join("\n")
        );
    }

    fn rust_sources(root: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut sources = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if matches!(
                        entry.file_name().to_str(),
                        Some(".git" | "target" | "node_modules")
                    ) {
                        continue;
                    }
                    pending.push(path);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    sources.push(path);
                }
            }
        }
        sources
    }
}
