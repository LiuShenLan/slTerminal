// tabClose.ts —— 页签关闭守卫统一入口（FE-49，SC-FE-07 语义迁移）
//
// 单一关闭逻辑：settings 面板 dirty → confirmDialog 确认才 api.close()；其余直关。
// × 关闭按钮 / Ctrl+W / 鼠标中键 / 右键菜单「关闭」四路共用本函数，防守卫逻辑
// 多路漂移（原内联于 PageDockviewHost DefaultTab ×，Ctrl+W 曾绕过守卫——F11
// 登记的不对称修复）。判据 = panelId 的 settings- 前缀 + dirtyRegistry 真值源
// （与 SettingsPanel 壳同源注册，无漂移；非 settings 面板 / 非 dirty 直关，行为零回归）。
// 依赖仅 confirmDialog（src/lib 命令式全局契约，与组件树解耦）——故 shortcuts
// 等非 React 模块可安全引用本模块，不拉入组件树。

import { confirmDialog } from "../lib";
import { isSettingsDirty } from "../features/settingsCenter/dirtyRegistry";

/**
 * 守卫关闭页签：settings 面板且 dirty → 确认才关闭；取消不关。
 * @param api 目标面板的 api（close 原语；调用方负责传"被关闭目标"自身）
 * @param panelId 面板 params.panelId（settings- 前缀判据；undefined = 无判据直关）
 */
export async function closeTabGuarded(
  api: { close(): void },
  panelId: string | undefined,
): Promise<void> {
  if (panelId?.startsWith("settings-") && isSettingsDirty(panelId)) {
    const ok = await confirmDialog({
      title: "未保存的修改",
      message: "当前配置页有未保存的修改，关闭将丢弃这些修改。",
      kind: "warning",
    });
    if (!ok) return;
  }
  api.close();
}
