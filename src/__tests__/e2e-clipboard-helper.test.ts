// e2e-clipboard-helper.test.ts — E2E clipboard helper 自动化测试
//
// 验证 App.tsx 顶层代码同步挂载的 E2E helper：
// - __slterm_e2e_writeClipboard 同步可用（非动态 import Promise）
// - 调用 writeClipboard → writeText（来自 ipc/clipboard）
// - __slterm_e2e_createProject 不受影响

import { describe, it, expect, vi } from "vitest";

// ─── Hoisted mocks ───
const { mockWriteText } = vi.hoisted(() => ({
  mockWriteText: vi.fn().mockResolvedValue(undefined),
}));

// ─── Module mocks ───
vi.mock("../ipc/clipboard", () => ({
  writeText: mockWriteText,
  readText: vi.fn().mockResolvedValue(""),
}));

vi.mock("../ipc/window", () => ({
  registerCloseHandler: vi.fn(() => () => {}),
}));

vi.mock("../workspace", () => ({
  Workspace: () => null,
}));

vi.mock("../workspace/layoutSerde", () => ({
  saveLayout: vi.fn(() => ({})),
}));

vi.mock("../stores/layout", () => ({
  useLayout: {
    getState: vi.fn(() => ({
      activePageId: null,
      setActivePage: vi.fn(),
    })),
    setState: vi.fn(),
  },
}));

vi.mock("../stores/projects", () => ({
  loadAllProjects: vi.fn().mockResolvedValue(undefined),
  markPersistenceReady: vi.fn(),
  saveAllProjects: vi.fn().mockResolvedValue(undefined),
  cancelPendingSave: vi.fn(),
  createProjectId: vi.fn(() => "test-proj-id"),
  createPageId: vi.fn(() => "test-page-id"),
  useProjects: {
    getState: vi.fn(() => ({
      projects: {},
      updatePageLayout: vi.fn(),
    })),
    setState: vi.fn(),
  },
}));

vi.mock("../features/sidebar/SidebarTree", () => ({
  default: () => null,
  makeDefaultLayout: vi.fn(() => ({})),
}));

vi.mock("dockview-react/dist/styles/dockview.css", () => ({}));

// 导入 App 触发模块顶层代码（挂载 E2E helpers）
import "../App";

// ─── Tests ───
describe("E2E clipboard helper", () => {
  it("25. __slterm_e2e_writeClipboard 同步可用且为函数", () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const helper = (window as any).__slterm_e2e_writeClipboard;
    expect(typeof helper).toBe("function");
  });

  it("26. 调用 writeClipboard('test-text') → 透传 ipc/clipboard.writeText", async () => {
    mockWriteText.mockClear();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const helper = (window as any).__slterm_e2e_writeClipboard;
    await helper("test-text");
    expect(mockWriteText).toHaveBeenCalledWith("test-text");
  });

  it("27. __slterm_e2e_createProject 同步可用且为函数", () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const helper = (window as any).__slterm_e2e_createProject;
    expect(typeof helper).toBe("function");
  });
});
