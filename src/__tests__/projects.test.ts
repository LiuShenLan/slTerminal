import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";

// ─── Hoisted mocks ───
const { mockToastShow } = vi.hoisted(() => ({
  mockToastShow: vi.fn(),
}));

// FE-11：loadFromDisk 损坏 → toast 告警——mock src/lib 隔离断言（照 fontSize/keybindings 测试模式）
vi.mock("../lib", () => ({
  toast: { show: mockToastShow, _reset: vi.fn() },
}));
import {
  useProjects,
  createProjectId,
  createPageId,
  cancelPendingSave,
  loadAllProjects,
  saveAllProjects,
  markPersistenceReady,
  markLoadSucceeded,
  _resetPersistence,
  MAX_PAGES,
} from "../stores/projects";
import type { Project, OperationPage } from "../stores/projects";

/** 构造测试用 OperationPage */
function makePage(
  name: string,
  cwd?: string,
): OperationPage {
  return {
    pageId: createPageId(),
    name,
    layout: {},
    cwd,
    createdAt: Date.now(),
    lastAccessedAt: Date.now(),
  };
}

/** 构造测试用 Project */
function makeProject(overrides?: Partial<Project>): Project {
  return {
    projectId: createProjectId(),
    name: "test-project",
    rootPath: "/tmp/test-project",
    pages: [makePage("main-page", "/tmp/test-project")],
    activePageId: null,
    version: 1,
    ...overrides,
  };
}

describe("projects store", () => {
  beforeEach(() => {
    // 重置持久化状态（清 timer + 复位 initialized 标记），防止上一用例 subscribe 触发写盘
    _resetPersistence();
    // FE-11：清 toast mock 调用史（loadFromDisk corrupted 用例断言独立）
    vi.clearAllMocks();
    // 重置 store 到初始状态
    useProjects.setState({
      projects: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
      expandedNodes: {},
    });
  });

  afterEach(() => {
    clearMocks();
    // STS-06：清活跃 debounce timer，防止上一用例残留 timer 泄漏到下一用例
    cancelPendingSave();
  });

  // ── 已有的 Project CRUD 测试 ──────────────────────────────

  it("addProject 应在 store 中添加新项目", () => {
    const project = makeProject();
    useProjects.getState().addProject(project);

    const { projects } = useProjects.getState();
    expect(Object.keys(projects)).toHaveLength(1);
    expect(projects[project.projectId].name).toBe("test-project");
    expect(projects[project.projectId].pages).toHaveLength(1);
  });

  it("removeProject 应从 store 中移除项目", () => {
    const project = makeProject();
    useProjects.getState().addProject(project);
    useProjects.getState().removeProject(project.projectId);

    const { projects } = useProjects.getState();
    expect(Object.keys(projects)).toHaveLength(0);
  });

  it("addPage 应在项目中追加操作页面", () => {
    const project = makeProject();
    useProjects.getState().addProject(project);

    const newPage = makePage("extra-page");
    useProjects.getState().addPage(project.projectId, newPage);

    const { projects } = useProjects.getState();
    expect(projects[project.projectId].pages).toHaveLength(2);
  });

  it("removePage 应移除操作页面并更新 activePageId", () => {
    const page1 = makePage("page-1");
    const page2 = makePage("page-2");
    const project = makeProject({ pages: [page1, page2], activePageId: page1.pageId });
    useProjects.getState().addProject(project);

    useProjects.getState().removePage(project.projectId, page1.pageId);

    const { projects } = useProjects.getState();
    expect(projects[project.projectId].pages).toHaveLength(1);
    expect(projects[project.projectId].activePageId).toBe(page2.pageId);
  });

  it("switchToPage 应更新 activePageId 和 lastAccessedAt", () => {
    const page1 = makePage("page-1");
    const project = makeProject({ pages: [page1], activePageId: null });
    useProjects.getState().addProject(project);

    useProjects.getState().switchToPage(project.projectId, page1.pageId);

    const { projects } = useProjects.getState();
    expect(projects[project.projectId].activePageId).toBe(page1.pageId);
  });

  // ── 已有的 renamePage 测试 ────────────────────────────────

  it("renamePage 应更新操作页面名称", () => {
    const page = makePage("原名");
    const project = makeProject({ pages: [page] });
    useProjects.getState().addProject(project);

    useProjects.getState().renamePage(project.projectId, page.pageId, "新名称");

    const updated = useProjects.getState().projects[project.projectId].pages[0];
    expect(updated.name).toBe("新名称");
  });

  it("renamePage 空字符串 → 允许空名称", () => {
    const page = makePage("原名");
    const project = makeProject({ pages: [page] });
    useProjects.getState().addProject(project);

    useProjects.getState().renamePage(project.projectId, page.pageId, "");

    const updated = useProjects.getState().projects[project.projectId].pages[0];
    expect(updated.name).toBe("");
  });

  it("renamePage 不存在的 pageId → 状态不变", () => {
    const page = makePage("保持");
    const project = makeProject({ pages: [page] });
    useProjects.getState().addProject(project);

    useProjects.getState().renamePage(project.projectId, "nonexistent", "新");

    const updated = useProjects.getState().projects[project.projectId].pages[0];
    expect(updated.name).toBe("保持");
  });

  // ═══════════════════════════════════════════════════════════
  // 以下为本次追加的测试用例
  // ═══════════════════════════════════════════════════════════

  // ── updatePageLayout ─────────────────────────────────────

  it("updatePageLayout 应更新已存在页面的 layout 字段", () => {
    const page = makePage("test-page");
    const project = makeProject({ pages: [page] });
    useProjects.getState().addProject(project);

    const newLayout = { panels: { panel1: { size: 100 } } };
    useProjects.getState().updatePageLayout(project.projectId, page.pageId, newLayout);

    const updated = useProjects.getState().projects[project.projectId].pages[0];
    expect(updated.layout).toEqual(newLayout);
  });

  it("updatePageLayout 不存在的 projectId → 状态不变", () => {
    const page = makePage("test-page");
    const project = makeProject({ pages: [page] });
    useProjects.getState().addProject(project);

    // STS-11②：structuredClone 深拷贝快照——同引用 toEqual 无法验证不可变性（原地 mutate 会同时改快照）
    const stateBefore = structuredClone(useProjects.getState().projects);
    useProjects.getState().updatePageLayout("nonexistent-proj", page.pageId, { x: 1 });

    expect(useProjects.getState().projects).toEqual(stateBefore);
  });

  it("updatePageLayout 不存在的 pageId → 页面不变但 version 仍递增（守卫仅检查 projectId）", () => {
    // 已知当前行为（无影响操作仍 bump version），非强契约（D3 锁定现状，勿据此阻塞未来守卫优化）
    const page = makePage("test-page");
    const project = makeProject({ pages: [page], version: 5 });
    useProjects.getState().addProject(project);

    useProjects.getState().updatePageLayout(project.projectId, "nonexistent-page", { x: 1 });

    const updated = useProjects.getState().projects[project.projectId];
    expect(updated.pages[0].layout).toEqual(page.layout);
    expect(updated.version).toBe(6);
  });

  it("updatePageLayout 应递增 version", () => {
    const page = makePage("test-page");
    const project = makeProject({ pages: [page], version: 3 });
    useProjects.getState().addProject(project);

    useProjects.getState().updatePageLayout(project.projectId, page.pageId, { y: 2 });

    expect(useProjects.getState().projects[project.projectId].version).toBe(4);
  });

  // ── toggleExpand ─────────────────────────────────────────

  it("toggleExpand 应展开已折叠的节点", () => {
    useProjects.setState({ expandedNodes: { "node-1": false } });

    useProjects.getState().toggleExpand("node-1");

    expect(useProjects.getState().expandedNodes["node-1"]).toBe(true);
  });

  it("toggleExpand 应折叠已展开的节点", () => {
    useProjects.setState({ expandedNodes: { "node-1": true } });

    useProjects.getState().toggleExpand("node-1");

    expect(useProjects.getState().expandedNodes["node-1"]).toBe(false);
  });

  it("toggleExpand 不存在的 nodeId → 新键被设为 true（!undefined === true）", () => {
    expect(useProjects.getState().expandedNodes["new-node"]).toBeUndefined();

    useProjects.getState().toggleExpand("new-node");

    expect(useProjects.getState().expandedNodes["new-node"]).toBe(true);
  });

  // ── cancelPendingSave ────────────────────────────────────

  it("cancelPendingSave 应作为可调用函数导出", () => {
    expect(typeof cancelPendingSave).toBe("function");
    // 无参数调用不抛错（空定时器场景）
    expect(() => cancelPendingSave()).not.toThrow();
  });

  // ── removePage 边界 ──────────────────────────────────────

  it("removePage 移除最后一个页面 → activePageId 变为 null", () => {
    const page = makePage("only-page");
    const project = makeProject({ pages: [page], activePageId: page.pageId });
    useProjects.getState().addProject(project);

    useProjects.getState().removePage(project.projectId, page.pageId);

    const updated = useProjects.getState().projects[project.projectId];
    expect(updated.pages).toHaveLength(0);
    expect(updated.activePageId).toBeNull();
  });

  it("removePage 移除非当前活跃页面 → activePageId 保持不变", () => {
    const page1 = makePage("page-1");
    const page2 = makePage("page-2");
    const page3 = makePage("page-3");
    const project = makeProject({ pages: [page1, page2, page3], activePageId: page1.pageId });
    useProjects.getState().addProject(project);

    useProjects.getState().removePage(project.projectId, page2.pageId);

    const updated = useProjects.getState().projects[project.projectId];
    expect(updated.pages).toHaveLength(2);
    expect(updated.activePageId).toBe(page1.pageId);
  });

  // ── switchToPage 增强 ────────────────────────────────────

  it("switchToPage 应更新 lastAccessedAt 为非零值", () => {
    const page = makePage("test-page");
    page.lastAccessedAt = 0; // 设已知初始值
    const project = makeProject({ pages: [page], activePageId: null });
    useProjects.getState().addProject(project);

    useProjects.getState().switchToPage(project.projectId, page.pageId);

    const updated = useProjects.getState().projects[project.projectId].pages[0];
    expect(updated.lastAccessedAt).toBeGreaterThan(0);
  });

  it("switchToPage 应递增 version", () => {
    const page = makePage("test-page");
    const project = makeProject({ pages: [page], version: 5 });
    useProjects.getState().addProject(project);

    useProjects.getState().switchToPage(project.projectId, page.pageId);

    expect(useProjects.getState().projects[project.projectId].version).toBe(6);
  });

  // ── 不存在 projectId 的守卫 ──────────────────────────────

  it("addPage 不存在的 projectId → 状态不变", () => {
    // STS-11②：structuredClone 深拷贝快照——同引用 toEqual 无法验证不可变性（原地 mutate 会同时改快照）
    const stateBefore = structuredClone(useProjects.getState().projects);
    useProjects.getState().addPage("nonexistent", makePage("p"));

    expect(useProjects.getState().projects).toEqual(stateBefore);
  });

  it("removePage 不存在的 projectId → 状态不变", () => {
    // STS-11②：structuredClone 深拷贝快照——同引用 toEqual 无法验证不可变性（原地 mutate 会同时改快照）
    const stateBefore = structuredClone(useProjects.getState().projects);
    useProjects.getState().removePage("nonexistent", "any-page");

    expect(useProjects.getState().projects).toEqual(stateBefore);
  });

  it("switchToPage 不存在的 projectId → 状态不变", () => {
    // STS-11②：structuredClone 深拷贝快照——同引用 toEqual 无法验证不可变性（原地 mutate 会同时改快照）
    const stateBefore = structuredClone(useProjects.getState().projects);
    useProjects.getState().switchToPage("nonexistent", "any-page");

    expect(useProjects.getState().projects).toEqual(stateBefore);
  });

  it("renamePage 不存在的 projectId → 状态不变", () => {
    // STS-10：renamePage 守卫缺失补测——不存在的 projectId 时 projects/version 均不变
    const page = makePage("保持");
    const project = makeProject({ pages: [page] });
    useProjects.getState().addProject(project);

    const stateBefore = structuredClone(useProjects.getState().projects);
    useProjects.getState().renamePage("nonexistent-proj", page.pageId, "新");

    expect(useProjects.getState().projects).toEqual(stateBefore);
  });

  // ── ID 格式 ──────────────────────────────────────────────

  it("createProjectId 应以 'proj-' 开头", () => {
    const id = createProjectId();
    expect(id.startsWith("proj-")).toBe(true);
  });

  it("createPageId 应以 'page-' 开头", () => {
    const id = createPageId();
    expect(id.startsWith("page-")).toBe(true);
  });

  // ── addPage activePageId 保留 ─────────────────────────────

  // ── FE-01: 页面总数上限 MAX_PAGES ────────────────────────

  it("FE-01: MAX_PAGES 常量为 20（S12 跨边界契约）", () => {
    expect(MAX_PAGES).toBe(20);
  });

  it("FE-01/FE-36: 全局页面数达 MAX_PAGES 上限 → addPage 拒绝 + toast 告警，状态不变", () => {
    // FE-36：上限按跨项目全局计数——A 10 页 + B 10 页合计 20，B 再增被拒
    const pagesA = Array.from({ length: MAX_PAGES / 2 }, (_, i) => makePage(`a-${i}`));
    const projectA = makeProject({ pages: pagesA, activePageId: pagesA[0].pageId });
    useProjects.getState().addProject(projectA);
    const pagesB = Array.from({ length: MAX_PAGES / 2 }, (_, i) => makePage(`b-${i}`));
    const projectB = makeProject({ pages: pagesB });
    useProjects.getState().addProject(projectB);

    // STS-11②：structuredClone 深拷贝快照——同引用 toEqual 无法验证不可变性
    const stateBefore = structuredClone(useProjects.getState().projects);
    const extra = makePage("extra");
    useProjects.getState().addPage(projectB.projectId, extra);

    // 拒绝新增：页面数不变、version 不递增、expandedNodes 不新增
    expect(useProjects.getState().projects[projectB.projectId].pages).toHaveLength(MAX_PAGES / 2);
    expect(useProjects.getState().projects).toEqual(stateBefore);
    expect(useProjects.getState().expandedNodes[extra.pageId]).toBeUndefined();
    // toast 告警（契约文案「页面数已达上限」）
    expect(mockToastShow).toHaveBeenCalledWith("warning", "页面数已达上限");
  });

  it("FE-01/FE-36: 全局页面数恰为 MAX_PAGES-1 → addPage 正常新增（边界内不拒绝不告警）", () => {
    // FE-36：跨项目边界——A 15 页 + B 4 页合计 19，B 再增 1 页成功
    const pagesA = Array.from({ length: 15 }, (_, i) => makePage(`a-${i}`));
    const projectA = makeProject({ pages: pagesA });
    useProjects.getState().addProject(projectA);
    const pagesB = Array.from({ length: MAX_PAGES - 16 }, (_, i) => makePage(`b-${i}`));
    const projectB = makeProject({ pages: pagesB });
    useProjects.getState().addProject(projectB);

    useProjects.getState().addPage(projectB.projectId, makePage("last"));

    expect(useProjects.getState().projects[projectB.projectId].pages).toHaveLength(MAX_PAGES - 15);
    expect(mockToastShow).not.toHaveBeenCalled();
  });

  it("FE-36: 跨项目全局计数——项目 A 15 页 + 项目 B 5 页时，B addPage 拒绝 + toast", () => {
    // 各项目单独未达上限（均 < MAX_PAGES），但全局合计 20 达上限——B 再增被拒
    const pagesA = Array.from({ length: 15 }, (_, i) => makePage(`a-${i}`));
    const projectA = makeProject({ pages: pagesA });
    useProjects.getState().addProject(projectA);
    const pagesB = Array.from({ length: 5 }, (_, i) => makePage(`b-${i}`));
    const projectB = makeProject({ pages: pagesB });
    useProjects.getState().addProject(projectB);

    const stateBefore = structuredClone(useProjects.getState().projects);
    useProjects.getState().addPage(projectB.projectId, makePage("extra"));

    // 拒绝新增：B 页面数不变、全局状态不变
    expect(useProjects.getState().projects[projectB.projectId].pages).toHaveLength(5);
    expect(useProjects.getState().projects).toEqual(stateBefore);
    expect(mockToastShow).toHaveBeenCalledWith("warning", "页面数已达上限");
  });

  it("addPage 已有 activePageId 时不改变活跃页面", () => {
    const page1 = makePage("page-1");
    const project = makeProject({ pages: [page1], activePageId: page1.pageId });
    useProjects.getState().addProject(project);

    const page2 = makePage("page-2");
    useProjects.getState().addPage(project.projectId, page2);

    const updated = useProjects.getState().projects[project.projectId];
    // activePageId 应保持为 page1，不因新增页面改变
    expect(updated.activePageId).toBe(page1.pageId);
  });

  // ── markPersistenceReady + subscribe ──────────────────────

  it("markPersistenceReady 应允许后续 save 操作", () => {
    // STS-10②：原用例仅断言"不抛错"，名实不符——补实际 save 断言：
    // markPersistenceReady 后变更应在 2s debounce 后触发 save_projects
    vi.useFakeTimers();

    const writeSpy = vi.fn();
    mockIPC((cmd, args) => {
      if (cmd === "save_projects") writeSpy(cmd, args);
    });

    // 标记持久化就绪（beforeEach 已 _resetPersistence 重置 initialized=false）
    markPersistenceReady();

    // 触发变更 → 2s debounce 后应触发保存
    const project = makeProject();
    useProjects.getState().addProject(project);
    vi.advanceTimersByTime(2100);

    expect(writeSpy).toHaveBeenCalledTimes(1);

    vi.useRealTimers();
  });

  // ── removeProject 清理 expandedNodes ──────────────────────

  it("removeProject 应同时清理 expandedNodes", () => {
    const project = makeProject();
    useProjects.getState().addProject(project);
    useProjects.setState({ expandedNodes: { [project.projectId]: true } });

    useProjects.getState().removeProject(project.projectId);

    expect(useProjects.getState().expandedNodes[project.projectId]).toBeUndefined();
  });

  // ── removePage 不存在 pageId ────────────────────────────

  it("removePage 不存在的 pageId → version 递增但页面列表不变", () => {
    // 已知当前行为（无影响操作仍 bump version），非强契约（D3 锁定现状，勿据此阻塞未来守卫优化）
    const page = makePage("keep");
    const project = makeProject({ pages: [page], activePageId: page.pageId, version: 3 });
    useProjects.getState().addProject(project);

    useProjects.getState().removePage(project.projectId, "nonexistent-page");

    const updated = useProjects.getState().projects[project.projectId];
    // 页面列表不变
    expect(updated.pages).toHaveLength(1);
    expect(updated.pages[0].pageId).toBe(page.pageId);
    // version 仍递增（removePage 无条件递增）
    expect(updated.version).toBe(4);
  });

  // ═══════════════════════════════════════════════════════════
  // loadFromDisk — 磁盘加载
  // ═══════════════════════════════════════════════════════════

  it("loadFromDisk JSON 有效 → 正确 set 到 store", async () => {
    const mockData = {
      projects: {
        "proj-1": {
          projectId: "proj-1",
          name: "从磁盘加载的项目",
          rootPath: "C:\\loaded",
          pages: [],
          activePageId: null,
          version: 2,
        },
      },
      expandedNodes: { "node-a": true },
      deletionLock: { pendingDelete: "proj-1", acquiredAt: 1000 },
    };

    mockIPC((cmd) => {
      // FE-11/D11：后端返回 { data, corrupted }
      if (cmd === "load_projects") {
        return { data: JSON.stringify(mockData), corrupted: false };
      }
    });

    const { loadFromDisk } = useProjects.getState();
    await loadFromDisk();

    const { projects, expandedNodes, deletionLock } = useProjects.getState();
    expect(projects["proj-1"].name).toBe("从磁盘加载的项目");
    expect(projects["proj-1"].rootPath).toBe("C:\\loaded");
    expect(expandedNodes["node-a"]).toBe(true);
    expect(deletionLock.pendingDelete).toBe("proj-1");
  });

  it("loadFromDisk corrupted=true → 默认状态 + toast 告警（FE-11）", async () => {
    // 后端解析失败回退默认数据（"{}"）且 corrupted:true
    mockIPC((cmd) => {
      if (cmd === "load_projects") return { data: "{}", corrupted: true };
    });

    const stateBefore = structuredClone(useProjects.getState().projects);

    const { loadFromDisk } = useProjects.getState();
    // 不应抛错
    await expect(loadFromDisk()).resolves.toBeUndefined();

    // 状态保持初始空值
    expect(useProjects.getState().projects).toEqual(stateBefore);
    expect(useProjects.getState().projects).toEqual({});
    // FE-11：配置损坏统一 toast 告警（warning + 固定文案）
    expect(mockToastShow).toHaveBeenCalledWith("warning", "配置已损坏，已回退默认值");
  });

  it("loadFromDisk 文件不存在 → 静默降级，状态不变", async () => {
    // Rust 端返回 data:"{}" corrupted:false 表示文件不存在/首次启动
    mockIPC((cmd) => {
      if (cmd === "load_projects") return { data: "{}", corrupted: false };
    });

    const stateBefore = structuredClone(useProjects.getState().projects);

    const { loadFromDisk } = useProjects.getState();
    await expect(loadFromDisk()).resolves.toBeUndefined();

    // "{}" 解析后 projects 为 undefined，走 ?? {} → 空对象
    expect(useProjects.getState().projects).toEqual(stateBefore);
    expect(useProjects.getState().projects).toEqual({});
    // 未损坏 → 无 toast
    expect(mockToastShow).not.toHaveBeenCalled();
  });

  // ═══════════════════════════════════════════════════════════
  // saveToDisk — 磁盘保存
  // ═══════════════════════════════════════════════════════════

  it("saveToDisk 将当前 store 状态序列化为正确 JSON 结构", async () => {
    const project = makeProject();
    useProjects.getState().addProject(project);
    useProjects.setState({
      expandedNodes: { [project.projectId]: true },
      deletionLock: { pendingDelete: null, acquiredAt: null },
    });

    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    const { saveToDisk } = useProjects.getState();
    await saveToDisk();

    // 验证 save_projects 被调用
    const call = spy.mock.calls.find(([cmd]) => cmd === "save_projects");
    expect(call).toBeDefined();

    const [, args] = call as [string, { data: string }];
    expect(args.data).toBeDefined();

    // 验证 data 可解析且包含期望字段
    const parsed = JSON.parse(args.data);
    expect(parsed).toHaveProperty("projects");
    expect(parsed).toHaveProperty("expandedNodes");
    expect(parsed).toHaveProperty("deletionLock");
    expect(parsed.projects[project.projectId].name).toBe("test-project");
    expect(parsed.expandedNodes[project.projectId]).toBe(true);
    expect(parsed.deletionLock).toEqual({ pendingDelete: null, acquiredAt: null });
  });

  it("saveToDisk save_projects 失败 → 异常传播", async () => {
    // FE-01：空写守卫拦截空 store 直调——显式放行，聚焦「写盘失败异常传播」语义
    markLoadSucceeded();

    mockIPC((cmd) => {
      if (cmd === "save_projects") throw new Error("disk full");
    });

    const { saveToDisk } = useProjects.getState();
    await expect(saveToDisk()).rejects.toThrow("disk full");
  });

  // ═══════════════════════════════════════════════════════════
  // loadAllProjects / saveAllProjects — 持久化连线
  // ═══════════════════════════════════════════════════════════

  it("loadAllProjects 调用 loadFromDisk，从磁盘加载项目数据", async () => {
    const mockData = {
      projects: {
        "proj-loaded": {
          projectId: "proj-loaded",
          name: "已加载项目",
          rootPath: "D:\\repo",
          pages: [],
          activePageId: null,
          version: 1,
        },
      },
      expandedNodes: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
    };

    mockIPC((cmd) => {
      if (cmd === "load_projects") {
        return { data: JSON.stringify(mockData), corrupted: false };
      }
    });

    await loadAllProjects();

    const { projects } = useProjects.getState();
    expect(projects["proj-loaded"].name).toBe("已加载项目");
    expect(projects["proj-loaded"].rootPath).toBe("D:\\repo");
  });

  it("saveAllProjects 调用 saveToDisk → save_projects 被调用", async () => {
    const project = makeProject();
    useProjects.getState().addProject(project);
    useProjects.setState({
      expandedNodes: { [project.projectId]: false },
      deletionLock: { pendingDelete: null, acquiredAt: null },
    });

    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await saveAllProjects();

    const call = spy.mock.calls.find(([cmd]) => cmd === "save_projects");
    expect(call).toBeDefined();

    const [, args] = call as [string, { data: string }];
    const parsed = JSON.parse(args.data);
    expect(parsed.projects[project.projectId].name).toBe("test-project");
  });

  it("saveAllProjects 写入失败 → console.error", async () => {
    // FE-01：空写守卫拦截空 store 直调——显式放行，聚焦「写盘失败 console.error」语义
    markLoadSucceeded();

    mockIPC((cmd) => {
      if (cmd === "save_projects") throw new Error("permission denied");
    });

    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    await saveAllProjects();

    expect(consoleSpy).toHaveBeenCalledWith(
      "[slTerminal] 保存项目数据失败:",
      expect.any(Error),
    );

    consoleSpy.mockRestore();
  });

  // ═══════════════════════════════════════════════════════════
  // M2: debounce 持久化测试
  // ═══════════════════════════════════════════════════════════

  it("M2.1 initialized=false 时不触发 saveAllProjects（subscribe 守卫）", () => {
    vi.useFakeTimers();

    // _resetPersistence 已在 beforeEach 中调用，initialized=false
    // 触发 state 变更
    const project = makeProject();
    useProjects.getState().addProject(project);

    // 推进 3s（远超 2s debounce）
    vi.advanceTimersByTime(3000);

    // saveAllProjects 不应被触发——检查 save_projects 未被调用
    const spy = vi.fn();
    mockIPC((cmd, args) => { spy(cmd, args); });

    // 再推进一次确认无 pending timer
    vi.advanceTimersByTime(2000);
    expect(spy).not.toHaveBeenCalled();

    vi.useRealTimers();
  });

  it("M2.2 markPersistenceReady() 后变更 → 2s debounce → saveAllProjects", () => {
    vi.useFakeTimers();

    const writeSpy = vi.fn();
    mockIPC((cmd, args) => {
      if (cmd === "save_projects") writeSpy(cmd, args);
    });

    // 标记持久化就绪
    markPersistenceReady();

    // 触发变更
    const project = makeProject();
    useProjects.getState().addProject(project);

    // 不到 2s 不触发
    vi.advanceTimersByTime(1500);
    expect(writeSpy).not.toHaveBeenCalled();

    // 到达 2s 触发
    vi.advanceTimersByTime(600);
    expect(writeSpy).toHaveBeenCalledTimes(1);

    vi.useRealTimers();
  });

  it("M2.3 多次变更合并为一次写入（debounce 重置 timer）", () => {
    vi.useFakeTimers();

    const writeSpy = vi.fn();
    mockIPC((cmd, args) => {
      if (cmd === "save_projects") writeSpy(cmd, args);
    });

    markPersistenceReady();

    // 连续三次变更，每次间隔 500ms
    const project = makeProject();
    useProjects.getState().addProject(project);
    vi.advanceTimersByTime(500);

    useProjects.getState().addPage(project.projectId, makePage("p2"));
    vi.advanceTimersByTime(500);

    useProjects.getState().toggleExpand("node-1");
    vi.advanceTimersByTime(500);

    // 此时距首次变更 1500ms，距末次变更 500ms，debounce 未到
    expect(writeSpy).not.toHaveBeenCalled();

    // 推进到末次变更后 2s
    vi.advanceTimersByTime(1500);

    // 仅触发一次写入
    expect(writeSpy).toHaveBeenCalledTimes(1);

    vi.useRealTimers();
  });

  it("M2.4 cancelPendingSave() 取消待执行的 timer", () => {
    vi.useFakeTimers();

    const writeSpy = vi.fn();
    mockIPC((cmd, args) => {
      if (cmd === "save_projects") writeSpy(cmd, args);
    });

    markPersistenceReady();

    // 触发变更
    const project = makeProject();
    useProjects.getState().addProject(project);

    // 推进 1s（timer 未到）
    vi.advanceTimersByTime(1000);

    // 取消待执行的保存
    cancelPendingSave();

    // 推进到原本会触发的时间点
    vi.advanceTimersByTime(2000);

    // 不应有写入（timer 已被清除）
    expect(writeSpy).not.toHaveBeenCalled();

    vi.useRealTimers();
  });

  // ═══════════════════════════════════════════════════════════
  // T4: 新增持久化测试（sandbox 绕过 + 便携路径）
  // ═══════════════════════════════════════════════════════════

  it("T4.13 loadFromDisk 空 JSON 对象 → store 保持空", async () => {
    // Rust 端首次启动返回 data:"{}" corrupted:false
    mockIPC((cmd) => {
      if (cmd === "load_projects") return { data: "{}", corrupted: false };
    });

    const { loadFromDisk } = useProjects.getState();
    await loadFromDisk();

    const { projects, expandedNodes, deletionLock } = useProjects.getState();
    expect(projects).toEqual({});
    expect(expandedNodes).toEqual({});
    expect(deletionLock).toEqual({ pendingDelete: null, acquiredAt: null });
  });

  it("T4.14 saveToDisk 写入完整 JSON 结构（含三个顶层键）", async () => {
    const project = makeProject();
    useProjects.getState().addProject(project);
    useProjects.setState({
      expandedNodes: { "n1": true, "n2": false },
      deletionLock: { pendingDelete: "p1", acquiredAt: 999 },
    });

    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    const { saveToDisk } = useProjects.getState();
    await saveToDisk();

    const call = spy.mock.calls.find(([cmd]) => cmd === "save_projects");
    expect(call).toBeDefined();
    const [, args] = call as [string, { data: string }];
    const parsed = JSON.parse(args.data);

    // 验证三个顶层键均存在
    expect(parsed).toHaveProperty("projects");
    expect(parsed).toHaveProperty("expandedNodes");
    expect(parsed).toHaveProperty("deletionLock");
    expect(parsed.projects[project.projectId]).toBeDefined();
    expect(parsed.expandedNodes).toEqual({ n1: true, n2: false });
    expect(parsed.deletionLock).toEqual({ pendingDelete: "p1", acquiredAt: 999 });
  });

  it("T4.15 loadFromDisk → markPersistenceReady → 变更才触发保存", () => {
    vi.useFakeTimers();

    const writeSpy = vi.fn();
    mockIPC((cmd, args) => {
      if (cmd === "load_projects") return { data: "{}", corrupted: false };
      if (cmd === "save_projects") writeSpy(cmd, args);
    });

    // 此时 initialized=false，变更不应触发保存
    const project = makeProject();
    useProjects.getState().addProject(project);
    vi.advanceTimersByTime(3000);
    expect(writeSpy).not.toHaveBeenCalled();

    // markPersistenceReady 后，变更应触发 debounce 保存
    markPersistenceReady();
    useProjects.getState().addPage(project.projectId, makePage("p2"));
    vi.advanceTimersByTime(2100);
    expect(writeSpy).toHaveBeenCalledTimes(1);

    vi.useRealTimers();
  });

  it("T4.16 loadFromDisk JSON 缺少字段 → 走默认值", async () => {
    // 缺少 deletionLock 和 expandedNodes
    mockIPC((cmd) => {
      if (cmd === "load_projects") {
        return {
          data: '{"projects":{"p1":{"name":"only"}}}',
          corrupted: false,
        };
      }
    });

    const { loadFromDisk } = useProjects.getState();
    await loadFromDisk();

    const { projects, expandedNodes, deletionLock } = useProjects.getState();
    expect(projects["p1"].name).toBe("only");
    // 缺失字段走 ?? {} 默认值
    expect(expandedNodes).toEqual({});
    expect(deletionLock).toEqual({ pendingDelete: null, acquiredAt: null });
  });

  // ═══════════════════════════════════════════════════════════
  // FE-01: loadSucceeded 数据防线（空写守卫 / 异常上抛 / 结构校验）
  // ═══════════════════════════════════════════════════════════

  it("FE-01: loadFromDisk IPC 失败 → 异常上抛且 loadSucceeded 未置位（后续空写被拒）", async () => {
    mockIPC((cmd) => {
      if (cmd === "load_projects") throw new Error("ipc error");
    });

    const { loadFromDisk, saveToDisk } = useProjects.getState();
    // 异常上抛（不再被吞）
    await expect(loadFromDisk()).rejects.toThrow("ipc error");

    // loadSucceeded 未置位 + store 空 → 空写被拒，磁盘无写入调用
    const writeSpy = vi.fn();
    mockIPC((cmd, args) => {
      writeSpy(cmd, args);
    });
    await saveToDisk();
    expect(writeSpy).not.toHaveBeenCalled();
  });

  it("FE-01: loadFromDisk 返回 projects 非对象 → throw 格式异常", async () => {
    // 三种非法形态：数字 / null / 数组（数组 typeof 也是 object，须 Array.isArray 识别）
    const illegalPayloads = [
      JSON.stringify({ projects: 1 }),
      JSON.stringify({ projects: null }),
      JSON.stringify({ projects: [] }),
    ];

    const { loadFromDisk } = useProjects.getState();
    for (const payload of illegalPayloads) {
      mockIPC((cmd) => {
        if (cmd === "load_projects") return { data: payload, corrupted: false };
      });
      await expect(loadFromDisk()).rejects.toThrow("项目数据格式异常：projects 字段不是对象");
    }
  });

  it("FE-01: loadFromDisk 成功 → saveAllProjects 空状态正常写盘", async () => {
    // 首次启动 data:"{}" 加载成功 → loadSucceeded 置位 → 空 store 写盘放行
    mockIPC((cmd) => {
      if (cmd === "load_projects") return { data: "{}", corrupted: false };
    });
    const { loadFromDisk } = useProjects.getState();
    await loadFromDisk();

    const writeSpy = vi.fn();
    mockIPC((cmd, args) => {
      writeSpy(cmd, args);
    });
    await saveAllProjects();

    const call = writeSpy.mock.calls.find(([cmd]) => cmd === "save_projects");
    expect(call).toBeDefined();
  });

  it("FE-01: 未加载成功时空写被拒且磁盘无写入调用", async () => {
    // beforeEach 已 _resetPersistence（loadSucceeded=false）且 store 为空
    const writeSpy = vi.fn();
    mockIPC((cmd, args) => {
      writeSpy(cmd, args);
    });

    await saveAllProjects();

    expect(writeSpy).not.toHaveBeenCalled();
  });

  it("FE-01: markLoadSucceeded() 后空状态写盘放行", async () => {
    // E2E 分支/「以空状态继续」显式放行路径
    markLoadSucceeded();

    const writeSpy = vi.fn();
    mockIPC((cmd, args) => {
      writeSpy(cmd, args);
    });
    await saveAllProjects();

    const call = writeSpy.mock.calls.find(([cmd]) => cmd === "save_projects");
    expect(call).toBeDefined();
  });
});
