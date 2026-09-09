// tabClose.ts —— 页签关闭守卫统一入口（FE-49，SC-FE-07 语义迁移）
//
// 单一关闭逻辑：settings 面板 dirty → confirmDialog 确认才 api.close()；其余直关。
// × 关闭按钮 / Ctrl+W / 鼠标中键 / 右键菜单「关闭」四路共用本函数，防守卫逻辑
// 多路漂移（原内联于 tabChrome DefaultTab ×，Ctrl+W 曾绕过守卫——F11
// 登记的不对称修复）。判据 = panelId 的 settings- 前缀 + dirtyRegistry 真值源
// （与壳同源读写同一真值源；CP-017 后条目脱离壳生命周期，清除收口到确认丢弃
// 动作点；非 settings 面板 / 非 dirty 直关，行为零回归）。
// 依赖仅 confirmDialog（src/lib 命令式全局契约，与组件树解耦）——故 shortcuts
// 等非 React 模块可安全引用本模块，不拉入组件树。

import { confirmDialog } from "../lib";
import {
  isSettingsDirty,
  clearSettingsDirty,
} from "../features/settingsCenter/dirtyRegistry";
import { isSettingsPanelId } from "./pageGroups";

/**
 * 守卫关闭页签：settings 面板且 dirty → 确认才关闭；取消不关。
 * @param api 目标面板的 api（close 原语；调用方负责传"被关闭目标"自身）
 * @param panelId 面板 params.panelId（settings- 前缀判据；undefined = 无判据直关）
 */
export async function closeTabGuarded(
  api: { close(): void },
  panelId: string | undefined,
): Promise<void> {
  if (isSettingsPanelId(panelId) && isSettingsDirty(panelId)) {
    const ok = await confirmDialog({
      title: "未保存的修改",
      message: "当前配置页有未保存的修改，关闭将丢弃这些修改。",
      kind: "warning",
    });
    if (!ok) return;
    // CP-017：确认丢弃 = 真值源条目唯一清除点之一（壳卸载钩子已移除）
    clearSettingsDirty(panelId);
  }
  api.close();
}

/**
 * 守卫批量关闭（CP-036——关闭其他/关闭全部接入 dirty 守卫）：
 * 收集待关列表中 dirty 的 settings 面板 → 单次 confirmDialog 列明确认 →
 * 确认才全部 close（并清除被丢弃面板的 dirtyRegistry 条目，CP-017 契约）；
 * 无 dirty 面板零交互直关（对非 settings 面板行为零回归）。
 * @param tabs 待关闭页签（close 原语 + params.panelId 判据 + 页签标题——确认列表展示用）
 */
export async function closeTabsGuarded(
  tabs: Array<{
    api: { close(): void };
    panelId: string | undefined;
    title: string;
  }>,
): Promise<void> {
  const dirtyTabs = tabs.filter(
    (t) => isSettingsPanelId(t.panelId) && isSettingsDirty(t.panelId),
  );
  if (dirtyTabs.length > 0) {
    const ok = await confirmDialog({
      title: "未保存的修改",
      message:
        `以下设置面板有未保存的修改，关闭将丢弃这些修改：\n` +
        dirtyTabs.map((t) => `· ${t.title}`).join("\n"),
      kind: "warning",
    });
    if (!ok) return;
    for (const t of dirtyTabs) clearSettingsDirty(t.panelId!);
  }
  for (const t of tabs) t.api.close();
}
