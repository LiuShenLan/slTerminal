//! 设置页注册表:稳定路由 id / 视觉分组 / 顺序 / 双语搜索别名的单源。
//!
//! 注册表家族契约(硬约束 #13)在本模块的 Rust 形态:模块级单例 +
//! `register` / `get_all` / `_reset`(仅测试)。Rust 无副作用导入,注册
//! 由 [`register_builtin_pages`] 在设置页构造时显式触发——幂等,重复构造
//! 不会产生重复页。
//!
//! 路由 id 只追加不重用(旧入口指向的页面不因重排漂移);视觉顺序由
//! `order` 单独裁定,与 id 解耦。页集合重排后全局组在上:
//! application/appearance/profiles/interaction/advanced/localization/reset;
//! Agent 组页面经 inspect 联调后由同一 `register` 入口追加。

use std::sync::{Mutex, OnceLock};

use crate::i18n::Message;

/// 设置页分组。导航组序全局在上、Agent 在下。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsPageGroup {
    Global,
    /// Agent 组页面(inspect 联调,双轨缝合)。
    Agent,
}

/// 一个设置页的路由元数据。`title` 是 typed 消息 id(值/显示分离:
/// 标题随语言刷新,路由 id 稳定);`search_terms` 是节级中英混合别名串;
/// 控件级别名从 `slterm_settings::keys` 注册表按页面归属拼接,不在此重复。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SettingsPageDef {
    pub id: &'static str,
    pub group: SettingsPageGroup,
    pub order: u32,
    pub title: Message,
    pub search_terms: &'static str,
}

/// 全局组页面定义(本阶段全部可见页)。
pub(crate) const BUILTIN_PAGES: [SettingsPageDef; 7] = [
    SettingsPageDef {
        id: "application",
        group: SettingsPageGroup::Global,
        order: 0,
        title: Message::SettingsSidebarApplication,
        search_terms: "application app 应用 about 关于 version 版本 update 更新 sponsor 赞助",
    },
    SettingsPageDef {
        id: "appearance",
        group: SettingsPageGroup::Global,
        order: 1,
        title: Message::SettingsSidebarAppearance,
        search_terms: "appearance 外观 theme 主题 font 字体 opacity 透明度 background 背景 wallpaper 壁纸 scrollback 回滚 density 密度 import 导入",
    },
    SettingsPageDef {
        id: "profiles",
        group: SettingsPageGroup::Global,
        order: 2,
        title: Message::SettingsSidebarProfiles,
        search_terms: "profiles profile 配置文件 shell terminal 终端 completion 补全 startup 启动 environment 环境变量 notification 通知 bell 铃声",
    },
    SettingsPageDef {
        id: "interaction",
        group: SettingsPageGroup::Global,
        order: 3,
        title: Message::SettingsSidebarInteraction,
        search_terms: "interaction 交互 cursor 光标 copy 复制 paste 粘贴 focus 焦点 mouse 鼠标 tab 标签 keybind 键位 shortcut 快捷键",
    },
    SettingsPageDef {
        id: "advanced",
        group: SettingsPageGroup::Global,
        order: 4,
        title: Message::SettingsSidebarAdvanced,
        search_terms: "advanced 高级 session 会话 restore 恢复 tray 托盘 startup 自启动 login 开机 proxy 代理 update 更新 export 导出 workspace 工作区",
    },
    SettingsPageDef {
        id: "localization",
        group: SettingsPageGroup::Global,
        order: 5,
        title: Message::SettingsSidebarLocalization,
        search_terms: "localization language 语言 i18n locale 区域",
    },
    SettingsPageDef {
        id: "reset",
        group: SettingsPageGroup::Global,
        order: 6,
        title: Message::SettingsSidebarReset,
        search_terms: "reset 重置 restore defaults 恢复默认 backup 备份 settings 设置",
    },
];

/// Agent 组页面(全局组之后;组内按 `order` 排)。
pub(crate) const AGENT_PAGES: [SettingsPageDef; 1] = [SettingsPageDef {
    id: "agents",
    group: SettingsPageGroup::Agent,
    order: 0,
    title: Message::SettingsSidebarAgents,
    search_terms: "agents agent hooks 智能体 钩子 接入 integration claude codex cursor kimi copilot grok opencode pi",
}];

fn pages() -> &'static Mutex<Vec<SettingsPageDef>> {
    static PAGES: OnceLock<Mutex<Vec<SettingsPageDef>>> = OnceLock::new();
    PAGES.get_or_init(|| Mutex::new(Vec::new()))
}

/// 注册一个页面;同 id 已存在时拒绝(只追加不重用)。返回是否新登记。
pub(crate) fn register(def: SettingsPageDef) -> bool {
    let mut pages = pages()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if pages.iter().any(|existing| existing.id == def.id) {
        return false;
    }
    pages.push(def);
    true
}

/// 按 (分组序, order) 返回全部页面。分组序:Global 在 Agent 前。
pub(crate) fn get_all() -> Vec<SettingsPageDef> {
    let mut pages = pages()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    pages.sort_by_key(|def| {
        let group = match def.group {
            SettingsPageGroup::Global => 0,
            SettingsPageGroup::Agent => 1,
        };
        (group, def.order)
    });
    pages
}

/// 指定分组的页面(导航分组渲染消费)。
#[allow(dead_code)] // 消费点 = Agent 组导航;全局组当前直接遍历 `get_all`。
pub(crate) fn get_all_in(group: SettingsPageGroup) -> Vec<SettingsPageDef> {
    get_all()
        .into_iter()
        .filter(|def| def.group == group)
        .collect()
}

/// 清空注册表(仅测试;用例间隔离)。
#[cfg(test)]
pub(crate) fn _reset() {
    pages()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

/// 注册内置页(幂等)。设置页构造时调用一次;测试可先 `_reset` 再调用。
pub(crate) fn register_builtin_pages() {
    for def in BUILTIN_PAGES.into_iter().chain(AGENT_PAGES) {
        register(def);
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn builtin_pages_keep_stable_routes_and_the_reordered_global_group() {
        _reset();
        register_builtin_pages();
        let ids: Vec<_> = get_all().into_iter().map(|def| def.id).collect();
        assert_eq!(
            ids,
            [
                "application",
                "appearance",
                "profiles",
                "interaction",
                "advanced",
                "localization",
                "reset",
                "agents",
            ],
            "页集合重排:全局组在上,Agent 组(hooks 页)在下"
        );
        assert_eq!(
            get_all_in(SettingsPageGroup::Global).len(),
            BUILTIN_PAGES.len()
        );
        assert_eq!(
            get_all_in(SettingsPageGroup::Agent).len(),
            AGENT_PAGES.len()
        );
        _reset();
    }

    #[test]
    fn duplicate_route_ids_are_rejected_and_reset_isolates_cases() {
        _reset();
        assert!(register(BUILTIN_PAGES[0]));
        assert!(
            !register(BUILTIN_PAGES[0]),
            "路由 id 只追加不重用,重复注册必须拒绝"
        );
        _reset();
        assert!(get_all().is_empty());
    }

    #[test]
    fn agent_group_sorts_after_the_global_group() {
        _reset();
        register(SettingsPageDef {
            id: "agents",
            group: SettingsPageGroup::Agent,
            order: 0,
            // 测试条目只验证排序,复用现有消息 id 即可。
            title: Message::SettingsSidebarApplication,
            search_terms: "agents 智能体",
        });
        register(SettingsPageDef {
            id: "application",
            group: SettingsPageGroup::Global,
            order: 9,
            title: Message::SettingsSidebarApplication,
            search_terms: "application 应用",
        });
        let ids: Vec<_> = get_all().into_iter().map(|def| def.id).collect();
        assert_eq!(
            ids,
            ["application", "agents"],
            "全局组必须排在 Agent 组之前"
        );
        _reset();
    }
}
