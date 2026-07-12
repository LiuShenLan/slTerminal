// e2e-clipboard-helper.test.ts — E2E clipboard helper 自动化测试
//
// 验证 installAllE2eHelpers() 挂载的 E2E helper：
// - __slterm_e2e_writeClipboard 可用且为函数
// - 调用 writeClipboard → writeText（来自 ipc/clipboard）
// - __slterm_e2e_createProject 不受影响

import { describe, it, expect, vi, beforeEach } from "vitest";

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
      addProject: vi.fn(),
      updatePageLayout: vi.fn(),
    })),
    setState: vi.fn(),
  },
}));

vi.mock("../features/sidebar/SidebarTree", () => ({
  default: () => null,
  makeEmptyLayout: vi.fn(() => ({})),
}));

vi.mock("dockview-react/dist/styles/dockview.css", () => ({}));

// 导入 E2E helpers 并挂载
import { installAllE2eHelpers } from "../../e2e-tests/helpers";

// ─── Tests ───
describe("E2E clipboard helper", () => {
  beforeEach(() => {
    installAllE2eHelpers();
  });

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
