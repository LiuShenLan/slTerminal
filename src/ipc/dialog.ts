// tauri-plugin-dialog 薄封装——save/open() 自身就是 invoke() 包装器，走相同 IPC 通道
// 聚合到此以遵守架构约束 #1：invoke 只出现在 src/ipc/
//
// ask E2E 钩子（claude-history Stage 06 执行期决策）：
// embedded WDIO 无法操作原生对话框，且 Tauri 2 将 window.__TAURI_INTERNALS__ 双层锁死
// （writable/configurable 全 false，JS patch 静默失败）、@wdio/tauri-service 的 tauri.mock
// 在 embedded 模式无 core.invoke 通道——三条路径均实证不可行。故在 IPC 层提供
// E2E_ENABLED 门控钩子：测试设置 window.__slterm_e2e_dialogAsk（boolean）后，ask 直接返回
// 该值（等效模拟用户点按钮）。生产构建 E2E_ENABLED 编译期折叠为 false，整块 tree-shake，
// 零影响；E2E 构建下未设置钩子同样走真实弹窗。
export { save, open } from "@tauri-apps/plugin-dialog";
import {
  ask as realAsk,
  type ConfirmDialogOptions,
} from "@tauri-apps/plugin-dialog";
import { E2E_ENABLED } from "../lib/e2eEnabled";

/**
 * 确认弹窗。E2E 构建且 window.__slterm_e2e_dialogAsk 已设置时直接返回该值，
 * 否则走真实原生对话框（@tauri-apps/plugin-dialog 的 ask——内部经
 * plugin:dialog|message 后比较返回值 === okLabel）。
 */
export async function ask(
  message: string,
  options?: ConfirmDialogOptions | string,
): Promise<boolean> {
  if (E2E_ENABLED) {
    const override = (window as unknown as Record<string, unknown>)
      .__slterm_e2e_dialogAsk;
    if (override !== undefined) return override as boolean;
  }
  return realAsk(message, options);
}
