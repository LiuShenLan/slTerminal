// tab-close.test.ts — closeTabGuarded 页签关闭守卫单测（FE-49）
//
// 守卫判据（SC-FE-07 语义迁移自 DefaultTab × 内联实现）：panelId 为 settings-
// 前缀且 dirtyRegistry 真值（dirty 用真实现 setSettingsDirty 驱动——与生产同源，
// 防测试自 mock 自证）→ confirmDialog 确认才 api.close()；取消不关；非 settings
// / 非 dirty / panelId 缺失一律直关（行为零回归）。
// confirmDialog mock（src/lib 全局命令式契约——真实现依赖 ConfirmDialogHost
// 渲染，jsdom 单测环境不可达）。

import { describe, it, expect, vi, beforeEach } from "vitest";
import { closeTabGuarded } from "../workspace/tabClose";
import {
  isSettingsDirty,
  setSettingsDirty,
  clearSettingsDirty,
} from "../features/settingsCenter/dirtyRegistry";
import { confirmDialog } from "../lib";

vi.mock("../lib", () => ({ confirmDialog: vi.fn() }));

/** settings 面板 id（与生产 params.panelId 同构：settings-{pageId}） */
const PANEL_ID = "settings-page-1";

function makeApi(): { api: { close(): void }; close: ReturnType<typeof vi.fn> } {
  // vi.fn<() => void>()：mock 类型与 closeTabGuarded 的 { close(): void } 结构化
  // 参数兼容（无泛型的 ReturnType<typeof vi.fn> 混入构造签名不可赋给函数类型）
  const close = vi.fn<() => void>();
  return { api: { close }, close };
}

const mockConfirmDialog = vi.mocked(confirmDialog);

beforeEach(() => {
  mockConfirmDialog.mockReset();
  clearSettingsDirty(PANEL_ID);
});

describe("closeTabGuarded", () => {
  it("settings + dirty + 确认 → api.close() 一次，confirmDialog 选项契约正确", async () => {
    setSettingsDirty(PANEL_ID, true);
    mockConfirmDialog.mockResolvedValue(true);
    const { api, close } = makeApi();

    await closeTabGuarded(api, PANEL_ID);

    expect(close).toHaveBeenCalledOnce();
    expect(mockConfirmDialog).toHaveBeenCalledOnce();
    expect(mockConfirmDialog).toHaveBeenCalledWith({
      title: "未保存的修改",
      message: "当前配置页有未保存的修改，关闭将丢弃这些修改。",
      kind: "warning",
    });
  });

  it("settings + dirty + 取消 → 不关闭（确认弹窗拒绝即放弃）", async () => {
    setSettingsDirty(PANEL_ID, true);
    mockConfirmDialog.mockResolvedValue(false);
    const { api, close } = makeApi();

    await closeTabGuarded(api, PANEL_ID);

    expect(close).not.toHaveBeenCalled();
    expect(mockConfirmDialog).toHaveBeenCalledOnce();
  });

  it("settings 面板非 dirty → 直关，不弹确认", async () => {
    // dirtyRegistry 缺键 = 非 dirty（真值源语义）
    const { api, close } = makeApi();
    expect(isSettingsDirty(PANEL_ID)).toBe(false);

    await closeTabGuarded(api, PANEL_ID);

    expect(close).toHaveBeenCalledOnce();
    expect(mockConfirmDialog).not.toHaveBeenCalled();
  });

  it("非 settings 面板（即使有同名 dirty 注册也不可能——判据前缀短路）→ 直关", async () => {
    // 防御：人为往真值源塞非 settings 键（生产不可能发生），守卫仍只认前缀
    const otherId = "terminal-page-1";
    setSettingsDirty(otherId, true);
    const { api, close } = makeApi();

    await closeTabGuarded(api, otherId);

    expect(close).toHaveBeenCalledOnce();
    expect(mockConfirmDialog).not.toHaveBeenCalled();
    clearSettingsDirty(otherId);
  });

  it("panelId 缺失（裸面板/直渲染测试形态）→ 直关，不弹确认", async () => {
    const { api, close } = makeApi();

    await closeTabGuarded(api, undefined);

    expect(close).toHaveBeenCalledOnce();
    expect(mockConfirmDialog).not.toHaveBeenCalled();
  });

  it("重复调用幂等（每次独立守卫，无跨调用状态）", async () => {
    setSettingsDirty(PANEL_ID, true);
    mockConfirmDialog.mockResolvedValue(false);
    const { api, close } = makeApi();

    await closeTabGuarded(api, PANEL_ID);
    await closeTabGuarded(api, PANEL_ID);

    expect(close).not.toHaveBeenCalled();
    expect(mockConfirmDialog).toHaveBeenCalledTimes(2);

    // 确认后关闭
    mockConfirmDialog.mockResolvedValue(true);
    await closeTabGuarded(api, PANEL_ID);
    expect(close).toHaveBeenCalledOnce();
  });
});
