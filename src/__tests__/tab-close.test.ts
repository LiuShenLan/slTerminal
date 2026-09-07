// tab-close.test.ts — 页签关闭守卫单测（FE-49 单面板 + CP-036 批量）
//
// 守卫判据（SC-FE-07 语义迁移自 DefaultTab × 内联实现）：panelId 为 settings-
// 前缀且 dirtyRegistry 真值（dirty 用真实现 setSettingsDirty 驱动——与生产同源，
// 防测试自 mock 自证）→ confirmDialog 确认才 api.close()；取消不关；非 settings
// / 非 dirty / panelId 缺失一律直关（行为零回归）。
// closeTabsGuarded（CP-036 批量统一入口）：dirty 面板列表 + 单次确认 → 确认后
// 全部 close 并清除被丢弃面板条目；无 dirty 零交互直关。
// confirmDialog mock（src/lib 全局命令式契约——真实现依赖 ConfirmDialogHost
// 渲染，jsdom 单测环境不可达）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { closeTabGuarded, closeTabsGuarded } from "../workspace/tabClose";
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

  it("确认关闭 dirty settings 面板后 isSettingsDirty 为 false（CP-017 确认丢弃清除点）", async () => {
    setSettingsDirty(PANEL_ID, true);
    mockConfirmDialog.mockResolvedValue(true);
    const { api, close } = makeApi();

    await closeTabGuarded(api, PANEL_ID);

    expect(close).toHaveBeenCalledOnce();
    // 确认丢弃 = 真值源条目唯一清除点之一（壳卸载钩子已移除，CP-017）
    expect(isSettingsDirty(PANEL_ID)).toBe(false);
  });

  it("取消关闭后条目仍在（CP-017: 仅确认丢弃才清除，取消保留 dirty）", async () => {
    setSettingsDirty(PANEL_ID, true);
    mockConfirmDialog.mockResolvedValue(false);
    const { api, close } = makeApi();

    await closeTabGuarded(api, PANEL_ID);

    expect(close).not.toHaveBeenCalled();
    expect(isSettingsDirty(PANEL_ID)).toBe(true);
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

describe("closeTabsGuarded（CP-036 批量关闭族入口）", () => {
  const otherDirtyId = (n: number) => `settings-page-batch-${n}`;

  afterEach(() => {
    for (const id of [otherDirtyId(2), otherDirtyId(3)]) {
      clearSettingsDirty(id);
    }
  });

  it("无 dirty 面板 → 零确认全关（confirmDialog 不被调用）", async () => {
    const s = makeApi();
    const t = makeApi();

    await closeTabsGuarded([
      { api: s.api, panelId: PANEL_ID, title: "设置" },
      { api: t.api, panelId: "terminal-page-1", title: "terminal-0" },
    ]);

    expect(mockConfirmDialog).not.toHaveBeenCalled();
    expect(s.close).toHaveBeenCalledOnce();
    expect(t.close).toHaveBeenCalledOnce();
  });

  it("含 1 个 dirty + 确认 → 全部关闭且 dirtyRegistry 条目清除（CP-017 契约）", async () => {
    setSettingsDirty(PANEL_ID, true);
    mockConfirmDialog.mockResolvedValue(true);
    const s = makeApi();
    const t = makeApi();

    await closeTabsGuarded([
      { api: s.api, panelId: PANEL_ID, title: "设置" },
      { api: t.api, panelId: "terminal-page-1", title: "terminal-0" },
    ]);

    expect(mockConfirmDialog).toHaveBeenCalledOnce();
    expect(mockConfirmDialog).toHaveBeenCalledWith(
      expect.objectContaining({ title: "未保存的修改", kind: "warning" }),
    );
    expect(s.close).toHaveBeenCalledOnce();
    expect(t.close).toHaveBeenCalledOnce();
    expect(isSettingsDirty(PANEL_ID)).toBe(false);
  });

  it("含多个 dirty → confirmDialog 消息列全部标题；确认后全部清除", async () => {
    setSettingsDirty(PANEL_ID, true);
    setSettingsDirty(otherDirtyId(2), true);
    setSettingsDirty(otherDirtyId(3), true);
    mockConfirmDialog.mockResolvedValue(true);
    const a = makeApi();
    const b = makeApi();
    const c = makeApi();

    await closeTabsGuarded([
      { api: a.api, panelId: PANEL_ID, title: "设置 A" },
      { api: b.api, panelId: otherDirtyId(2), title: "设置 B" },
      { api: c.api, panelId: otherDirtyId(3), title: "设置 C" },
    ]);

    expect(mockConfirmDialog).toHaveBeenCalledOnce();
    const message = mockConfirmDialog.mock.calls[0][0].message as string;
    expect(message).toContain("设置 A");
    expect(message).toContain("设置 B");
    expect(message).toContain("设置 C");
    expect(isSettingsDirty(PANEL_ID)).toBe(false);
    expect(isSettingsDirty(otherDirtyId(2))).toBe(false);
    expect(isSettingsDirty(otherDirtyId(3))).toBe(false);
  });

  it("取消 → 一个都不关、条目保留（仅确认丢弃才清除）", async () => {
    setSettingsDirty(PANEL_ID, true);
    mockConfirmDialog.mockResolvedValue(false);
    const s = makeApi();
    const t = makeApi();

    await closeTabsGuarded([
      { api: s.api, panelId: PANEL_ID, title: "设置" },
      { api: t.api, panelId: "terminal-page-1", title: "terminal-0" },
    ]);

    expect(mockConfirmDialog).toHaveBeenCalledOnce();
    expect(s.close).not.toHaveBeenCalled();
    expect(t.close).not.toHaveBeenCalled();
    expect(isSettingsDirty(PANEL_ID)).toBe(true);
  });

  it("非 settings 面板（terminal/editor 等）→ 直关零交互", async () => {
    const a = makeApi();
    const b = makeApi();

    await closeTabsGuarded([
      { api: a.api, panelId: "terminal-page-1", title: "terminal-0" },
      { api: b.api, panelId: "editor-1", title: "a.ts" },
    ]);

    expect(mockConfirmDialog).not.toHaveBeenCalled();
    expect(a.close).toHaveBeenCalledOnce();
    expect(b.close).toHaveBeenCalledOnce();
  });
});
