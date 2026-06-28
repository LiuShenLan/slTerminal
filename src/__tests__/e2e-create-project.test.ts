// e2e-create-project.test.ts — E2E pending 标记 + localStorage 恢复交互逻辑测试
//
// 模拟 App.tsx init 序列核心逻辑，验证：
//   - E2E createProject 后 localStorage 残留不覆盖 activePageId
//   - 正常模式（无 E2E createProject）localStorage 正常恢复
//   - 无 localStorage 残留时两种模式都不影响

import { describe, it, expect, beforeEach, vi } from "vitest";

// ─── 导入 stores（不依赖 App.tsx，直接操作 Zustand）───
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";

// ─── Mock localStorage（jsdom 全局 localStorage 可能不可用）───
const storage = new Map<string, string>();
const mockLocalStorage = {
  getItem: vi.fn((key: string) => storage.get(key) ?? null),
  setItem: vi.fn((key: string, value: string) => { storage.set(key, value); }),
  removeItem: vi.fn((key: string) => { storage.delete(key); }),
  clear: vi.fn(() => { storage.clear(); }),
};

// 注入为全局（simulateInitRestore 中使用）
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(globalThis as any).localStorage = mockLocalStorage;

// ─── 辅助：重置 stores ───
function resetStores() {
  useProjects.setState({
    projects: {},
    expandedNodes: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
  });
  useLayout.setState({ activePageId: null });
}

// ─── 辅助：模拟 App.tsx init 序列的 localStorage 恢复逻辑 ───
// 与 App.tsx:98-101 等价——当 e2eProjectPending 为 true 时跳过恢复
function simulateInitRestore(e2eProjectPending: boolean): void {
  const lastPage = mockLocalStorage.getItem("slterm-last-active-page");
  if (lastPage && !e2eProjectPending) {
    useLayout.getState().setActivePage(lastPage);
  }
}

// ─── Tests ───
describe("E2E pending 标记 — localStorage 恢复交互", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStores();
    storage.clear();
  });

  it("10. E2E pending=true 时 localStorage lastPage 不覆盖 activePageId", () => {
    // 模拟：localStorage 有上次会话残留
    mockLocalStorage.setItem("slterm-last-active-page", "page-old-session");

    // 模拟：E2E 先调 createProject 设置 activePageId
    const e2ePageId = "page-e2e-001";
    useLayout.getState().setActivePage(e2ePageId);

    // 模拟：App init 序列运行（e2eProjectPending=true → 跳过恢复）
    simulateInitRestore(true);

    // 验证：activePageId 仍为 E2E 值，未被覆盖
    expect(useLayout.getState().activePageId).toBe(e2ePageId);
  });

  it("11. E2E pending=false 时 localStorage lastPage 正常覆盖 activePageId", () => {
    // 模拟：localStorage 有上次会话残留
    const savedPageId = "page-saved-session";
    mockLocalStorage.setItem("slterm-last-active-page", savedPageId);

    // 模拟：初始 activePageId 为 null（启动默认状态）
    useLayout.getState().setActivePage(null);
    expect(useLayout.getState().activePageId).toBeNull();

    // 模拟：App init 序列运行（e2eProjectPending=false → 正常恢复）
    simulateInitRestore(false);

    // 验证：activePageId 恢复为 localStorage 中的值
    expect(useLayout.getState().activePageId).toBe(savedPageId);
  });

  it("12. 无 localStorage 残留时无论 pending 状态都不覆盖", () => {
    // 无 localStorage 残留
    expect(mockLocalStorage.getItem("slterm-last-active-page")).toBeNull();

    const initialPageId = "page-initial";
    useLayout.getState().setActivePage(initialPageId);

    // E2E pending=true 时
    simulateInitRestore(true);
    expect(useLayout.getState().activePageId).toBe(initialPageId);

    // E2E pending=false 时
    simulateInitRestore(false);
    expect(useLayout.getState().activePageId).toBe(initialPageId);
  });
});
