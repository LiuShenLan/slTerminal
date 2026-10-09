//! pane/tab 纯标识 newtype(类型锚点,归属设计 05 篇):分屏树的叶类型即
//! pane id,app 内跨模块(hook 环境/runtime API 等)引用标识统一经本 crate,
//! 禁重定义、禁别名漂移。newtype 零成本、零依赖。

/// 终端 pane 标识:全 workspace 唯一、终生不复用。
/// 分配权威在 Workspace 的单调计数器(newtype 不持全局态,测试可注入序列)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PaneId(pub u64);

/// tab 稳定标识:跨排序/活跃切换不变;不进 session schema(schema 按位置存取)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(pub u64);

#[cfg(test)]
mod ids_tests {
    use super::*;
    use crate::{SplitDirection, SplitTree};
    use std::collections::HashMap;

    #[test]
    fn pane_id_supports_map_keys_and_ordering() {
        let a = PaneId(1);
        let b = PaneId(2);
        assert_ne!(a, b);
        assert!(a < b);
        // Copy 语义:复制后原值仍可用。
        let copied = a;
        assert_eq!(copied, a);
        let mut map = HashMap::new();
        map.insert(a, "first");
        assert_eq!(map.get(&PaneId(1)), Some(&"first"));
    }

    #[test]
    fn tab_id_supports_map_keys_and_ordering() {
        let a = TabId(1);
        let b = TabId(2);
        assert_ne!(a, b);
        assert!(a < b);
        let copied = a;
        assert_eq!(copied, a);
        let mut map = HashMap::new();
        map.insert(a, "first");
        assert_eq!(map.get(&TabId(1)), Some(&"first"));
    }

    #[test]
    fn split_tree_instantiates_with_pane_id() {
        // 运行时实例化形态锚点:SplitTree<PaneId>。
        let mut tree = SplitTree::leaf(PaneId(1));
        assert!(tree.split_leaf(PaneId(1), PaneId(2), SplitDirection::LeftRight, 0.5));
        assert_eq!(tree.leaves(), vec![PaneId(1), PaneId(2)]);
    }
}
