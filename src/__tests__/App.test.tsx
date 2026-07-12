// App.spec.tsx — __slterm_e2e_createProject 行为测试
//
// 验证 E2E helper 函数正确操作 Zustand stores，不依赖完整 App 渲染。
// T1 组：projects store 创建、activePage 设置、page 结构、多项目、pending 标记行为。

import { describe, it, expect, beforeEach, vi } from "vitest";

// ─── Mock localStorage（jsdom 全局 localStorage 可能不可用）───
const storage = new Map<string, string>();
const mockLocalStorage = {
  getItem: vi.fn((key: string) => storage.get(key) ?? null),
  setItem: vi.fn((key: string, value: string) => { storage.set(key, value); }),
  removeItem: vi.fn((key: string) => { storage.delete(key); }),
  clear: vi.fn(() => { storage.clear(); }),
};
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(globalThis as any).localStorage = mockLocalStorage;

// ─── Hoisted mocks（在 App.tsx 模块加载前生效）───
const mocks = vi.hoisted(() => ({
  mockWriteText: vi.fn(() => Promise.resolve()),
}));

vi.mock("../ipc/window", () => ({
  registerCloseHandler: vi.fn(() => () => {}),
}));

vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: mocks.mockWriteText,
  readText: vi.fn(() => Promise.resolve("")),
}));

vi.mock("../ipc/notify", () => ({
  onFsEvent: () => () => {},
  startWatch: () => Promise.resolve(),
}));

vi.mock("../features/sidebar/SidebarTree", () => ({
  default: () => null,
  makeEmptyLayout: vi.fn(() => ({})),
}));

// ─── 导入 stores + E2E helpers（不依赖 App 模块）───
import { installAllE2eHelpers } from "../../e2e-tests/helpers";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";

// ─── 辅助函数 ───
function resetStores() {
  useProjects.setState({
    projects: {},
    expandedNodes: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
  });
  useLayout.setState({ activePageId: null });
}

function callCreateProject(dirPath: string): string {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const fn = (window as any).__slterm_e2e_createProject;
  if (typeof fn !== "function") throw new Error("__slterm_e2e_createProject 未挂载");
  return fn(dirPath);
}

// ─── Tests ───
describe("__slterm_e2e_createProject", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStores();
    localStorage.clear();
    // 挂载 E2E helper 到 window（替代旧 App.tsx 模块级代码）
    installAllE2eHelpers();
  });

  it("1. 调用后 projects store 含新项目，rootPath + pages 正确", () => {
    callCreateProject("C:\\e2e-test");

    const { projects } = useProjects.getState();
    const projectIds = Object.keys(projects);
    expect(projectIds).toHaveLength(1);

    const proj = projects[projectIds[0]];
    expect(proj.rootPath).toBe("C:\\e2e-test");
    expect(proj.pages).toHaveLength(1);
    expect(proj.pages[0].cwd).toBe("C:\\e2e-test");
  });

  it("2. 调用后 activePageId 为新建的 pageId（返回值匹配）", () => {
    const returnedPageId = callCreateProject("C:\\e2e-test");
    const { activePageId } = useLayout.getState();

    expect(activePageId).toBe(returnedPageId);
    // activePageId 非空且格式正确
    expect(activePageId).toMatch(/^page-/);
  });

  it("3. 创建的 page 含正确的 layout + cwd", () => {
    callCreateProject("D:\\my-project");

    const { projects } = useProjects.getState();
    const proj = Object.values(projects)[0];
    const page = proj.pages[0];

    // T24: layout 为空对象（新建页面不自动包含 terminal）
    expect(page.layout).toBeDefined();
    expect(typeof page.layout).toBe("object");
    expect(page.layout).toEqual({});
    // T25: layout 无 grid 属性
    expect(page.layout).not.toHaveProperty("grid");

    // cwd 匹配传入路径
    expect(page.cwd).toBe("D:\\my-project");

    // name 为路径最后一段
    expect(page.name).toBe("my-project");
  });

  it("4. E2E pending 标记阻止 localStorage lastPage 覆盖 activePageId", () => {
    // 模拟：localStorage 有上次会话残留
    localStorage.setItem("slterm-last-active-page", "page-old-session");

    // 调 createProject（内部设 e2eProjectPending=true → 跳过 localStorage 恢复）
    const e2ePageId = callCreateProject("C:\\e2e-test");

    // 验证 activePageId 是 E2E 创建的 pageId，而非 localStorage 残留
    const { activePageId } = useLayout.getState();
    expect(activePageId).toBe(e2ePageId);
    expect(activePageId).not.toBe("page-old-session");
  });

  it("5. 重复调用创建多个项目不互相覆盖", () => {
    callCreateProject("C:\\proj-a");
    callCreateProject("D:\\proj-b");

    const { projects } = useProjects.getState();
    const ids = Object.keys(projects);
    expect(ids).toHaveLength(2);

    const paths = Object.values(projects).map((p) => p.rootPath).sort();
    expect(paths).toEqual(["C:\\proj-a", "D:\\proj-b"]);

    // 每个项目各有 1 个 page
    for (const proj of Object.values(projects)) {
      expect(proj.pages).toHaveLength(1);
    }
  });
});
