// sideBar.test.ts — 侧栏视图 Zustand Store 自动化测试
//
// 测试模式同 fontSize.test.ts / keybindings.test.ts：
// mock ../ipc/settings，getState() 直接操作，fake timers 测 debounce。
// 本测试不 import sideViewDefs —— sideViewRegistry.getAll() 初始为空，
// beforeEach 手动 register stub 视图（id="projects"/"explorer"），afterEach _reset() 隔离。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// ─── Hoisted mocks ───
const { mockSaveSettings, mockLoadSettings, mockToastShow, mockGetErrorMessage } = vi.hoisted(() => ({
  mockSaveSettings: vi.fn().mockResolvedValue(undefined),
  // FE-11/D11：wrapper 返回 { data, corrupted }——无文件 = data:null, corrupted:false
  mockLoadSettings: vi.fn().mockResolvedValue({ data: null, corrupted: false }),
  mockToastShow: vi.fn(),
  // FE-09：错误消息统一经 getErrorMessage（契约），默认兜底 String(err)
  mockGetErrorMessage: vi.fn((err: unknown) => String(err)),
}));

vi.mock("../ipc/settings", () => ({
  saveSettings: mockSaveSettings,
  loadSettings: mockLoadSettings,
}));

// FE-09：store 保存失败经 src/lib 的 toast/getErrorMessage——mock 隔离断言
vi.mock("../lib", () => ({
  toast: { show: mockToastShow, _reset: vi.fn() },
  getErrorMessage: mockGetErrorMessage,
}));

// 阻断 side-effect 注册：真实 nav/explorer/commit 视图不得混入本文件的
// sideViewRegistry 隔离（_reset 后仅注册 stub；TQ-B-02）
vi.mock("../features/sideViews/sideViewDefs", () => ({}));

import { useSideBar, cancelPendingSave } from "../stores/sideBar";
import { sideViewRegistry } from "../features/sideViews/sideViewRegistry";
import {
  DEFAULT_ZONES,
  DEFAULT_OPEN,
  WIDTH_DEFAULT,
  WIDTH_MIN,
  WIDTH_MAX,
  SPLIT_DEFAULT,
  SPLIT_MIN,
  SPLIT_MAX,
} from "../features/sideViews/sideBarState";

// 测试用 stub 视图定义（需与 DEFAULT_ZONES 中的 id 对齐，否则 reconcileZones 会过滤未注册 id）
// NAV-05 三槽：nav / explorer / commit（agent-status 已退役）
// icon 字段为组件形态（IC-06）——stub 不渲染内容
function registerTestViews(): void {
  sideViewRegistry.register({
    id: "nav",
    title: "导航树",
    icon: () => null,
    component: () => null,
  });
  sideViewRegistry.register({
    id: "explorer",
    title: "文件浏览器",
    icon: () => null,
    component: () => null,
  });
  sideViewRegistry.register({
    id: "commit",
    title: "Commit",
    icon: () => null,
    component: () => null,
  });
}

describe("sideBar store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    // 注册 stub 视图 → reconcileZones 不会过滤掉已注册 id
    sideViewRegistry._reset();
    // 防御断言：_reset 后注册表必须为空（sideViewDefs 被 mock 阻断，TQ-B-02）
    expect(sideViewRegistry.getAll().length).toBe(0);
    registerTestViews();
    // 重置 store 到初始状态（loaded: false 防 debounce 触发）
    useSideBar.setState({
      zones: { ...DEFAULT_ZONES },
      open: { ...DEFAULT_OPEN },
      width: WIDTH_DEFAULT,
      splitRatio: SPLIT_DEFAULT,
      loaded: false,
    });
    vi.clearAllMocks();
    mockSaveSettings.mockResolvedValue(undefined);
    mockLoadSettings.mockResolvedValue({ data: null, corrupted: false });
  });

  afterEach(() => {
    vi.useRealTimers();
    sideViewRegistry._reset();
  });

  // ── 默认值 ──

  it("1. 默认值：zones/open/width/splitRatio 为契约常量，loaded=false", () => {
    const s = useSideBar.getState();
    expect(s.zones).toEqual(DEFAULT_ZONES);
    expect(s.open).toEqual(DEFAULT_OPEN);
    expect(s.width).toBe(WIDTH_DEFAULT);
    expect(s.splitRatio).toBe(SPLIT_DEFAULT);
    expect(s.loaded).toBe(false);
  });

  // ── toggleView（经 store 委托 toggleViewPure） ──

  it("2. toggleView 关闭已打开视图（R2）", () => {
    useSideBar.setState({ open: { top: "nav", bottom: null } });
    useSideBar.getState().toggleView("nav");
    expect(useSideBar.getState().open).toEqual({ top: null, bottom: null });
  });

  it("3. toggleView 打开未打开视图——隐式关闭同区旧视图（R1）", () => {
    useSideBar.setState({ open: { top: "nav", bottom: null } });
    useSideBar.getState().toggleView("explorer");
    expect(useSideBar.getState().open).toEqual({ top: "explorer", bottom: null });
  });

  // ── moveButton（经 store 委托 moveButtonPure） ──

  it("4. moveButton 跨区移动——打开中视图跟随到目标区（R6）", () => {
    useSideBar.setState({
      zones: { top: ["nav", "explorer"], bottom: [] },
      open: { top: "nav", bottom: null },
    });
    useSideBar.getState().moveButton("nav", "bottom", 0);
    expect(useSideBar.getState().zones).toEqual({ top: ["explorer"], bottom: ["nav"] });
    expect(useSideBar.getState().open).toEqual({ top: null, bottom: "nav" });
  });

  it("5. moveButton 同区移动——只调顺序不动 open（R8）", () => {
    useSideBar.setState({
      zones: { top: ["nav", "explorer"], bottom: [] },
      open: { top: "explorer", bottom: null },
    });
    useSideBar.getState().moveButton("explorer", "top", 0);
    expect(useSideBar.getState().zones).toEqual({ top: ["explorer", "nav"], bottom: [] });
    expect(useSideBar.getState().open).toEqual({ top: "explorer", bottom: null });
  });

  // ── setWidth / setSplitRatio clamp ──

  it("6. setWidth 内部 clamp：低于 MIN → MIN，高于 MAX → MAX，正常值原样", () => {
    const s = useSideBar.getState();
    s.setWidth(WIDTH_MIN - 50);
    expect(useSideBar.getState().width).toBe(WIDTH_MIN);
    s.setWidth(WIDTH_MAX + 50);
    expect(useSideBar.getState().width).toBe(WIDTH_MAX);
    s.setWidth(300);
    expect(useSideBar.getState().width).toBe(300);
  });

  it("7. setSplitRatio 内部 clamp：低于 MIN → MIN，高于 MAX → MAX，正常值原样", () => {
    const s = useSideBar.getState();
    s.setSplitRatio(SPLIT_MIN - 0.1);
    expect(useSideBar.getState().splitRatio).toBe(SPLIT_MIN);
    s.setSplitRatio(SPLIT_MAX + 0.1);
    expect(useSideBar.getState().splitRatio).toBe(SPLIT_MAX);
    s.setSplitRatio(0.6);
    expect(useSideBar.getState().splitRatio).toBe(0.6);
  });

  it("7b. setWidth/setSplitRatio 非有限数 NaN/±Infinity → clamp 回退 MIN", () => {
    const s = useSideBar.getState();
    s.setWidth(NaN);
    expect(useSideBar.getState().width).toBe(WIDTH_MIN);
    s.setWidth(Infinity);
    expect(useSideBar.getState().width).toBe(WIDTH_MIN);
    s.setWidth(-Infinity);
    expect(useSideBar.getState().width).toBe(WIDTH_MIN);
    s.setSplitRatio(NaN);
    expect(useSideBar.getState().splitRatio).toBe(SPLIT_MIN);
    s.setSplitRatio(Infinity);
    expect(useSideBar.getState().splitRatio).toBe(SPLIT_MIN);
    s.setSplitRatio(-Infinity);
    expect(useSideBar.getState().splitRatio).toBe(SPLIT_MIN);
  });

  // ── loadFromDisk ──

  it("8. loadFromDisk 首次启动（data:null）→ 保持默认，loaded=true", async () => {
    mockLoadSettings.mockResolvedValue({ data: null, corrupted: false });
    await useSideBar.getState().loadFromDisk();
    const s = useSideBar.getState();
    expect(s.zones).toEqual(DEFAULT_ZONES);
    expect(s.open).toEqual(DEFAULT_OPEN);
    expect(s.width).toBe(WIDTH_DEFAULT);
    expect(s.splitRatio).toBe(SPLIT_DEFAULT);
    expect(s.loaded).toBe(true);
  });

  it("9. loadFromDisk 读取合法 sideBar 段 → 正确恢复", async () => {
    mockLoadSettings.mockResolvedValue({
      data: {
        sideBar: {
          zones: { top: ["explorer", "nav"], bottom: [] },
          open: { top: "explorer", bottom: null },
          width: 320,
          splitRatio: 0.7,
        },
      },
      corrupted: false,
    });
    await useSideBar.getState().loadFromDisk();
    const s = useSideBar.getState();
    // reconcileZones 补全注册表中缺失的 commit
    expect(s.zones).toEqual({ top: ["explorer", "nav", "commit"], bottom: [] });
    expect(s.open).toEqual({ top: "explorer", bottom: null });
    expect(s.width).toBe(320);
    expect(s.splitRatio).toBe(0.7);
    expect(s.loaded).toBe(true);
  });

  it("10. loadFromDisk 脏数据 sanitize——非对象 sideBar → 全回退默认", async () => {
    mockLoadSettings.mockResolvedValue({
      data: { sideBar: "not-an-object" },
      corrupted: false,
    });
    await useSideBar.getState().loadFromDisk();
    const s = useSideBar.getState();
    // sanitizeSideBar 入参非对象 → 返回全默认值
    expect(s.zones).toEqual(DEFAULT_ZONES);
    expect(s.open).toEqual(DEFAULT_OPEN);
    expect(s.width).toBe(WIDTH_DEFAULT);
    expect(s.splitRatio).toBe(SPLIT_DEFAULT);
    expect(s.loaded).toBe(true);
  });

  it("11. loadFromDisk 脏数据 sanitize——width/splitRatio 超限 clamp", async () => {
    mockLoadSettings.mockResolvedValue({
      data: {
        sideBar: {
          zones: { top: ["projects"], bottom: [] },
          open: { top: "projects", bottom: null },
          width: 9999,
          splitRatio: 99,
        },
      },
      corrupted: false,
    });
    await useSideBar.getState().loadFromDisk();
    const s = useSideBar.getState();
    expect(s.width).toBe(WIDTH_MAX);
    expect(s.splitRatio).toBe(SPLIT_MAX);
  });

  it("12. loadFromDisk 脏数据 sanitize——zones/open 结构非法 → 回退默认", async () => {
    mockLoadSettings.mockResolvedValue({
      data: {
        sideBar: {
          zones: { top: "not-an-array", bottom: null },
          open: 123,
          width: 250,
          splitRatio: 0.5,
        },
      },
      corrupted: false,
    });
    await useSideBar.getState().loadFromDisk();
    const s = useSideBar.getState();
    expect(s.zones).toEqual(DEFAULT_ZONES);
    expect(s.open).toEqual(DEFAULT_OPEN);
    // width/splitRatio 仍按合法值保留
    expect(s.width).toBe(WIDTH_DEFAULT);
    expect(s.splitRatio).toBe(SPLIT_DEFAULT);
  });

  it("13. loadFromDisk sideBar 键缺失 → 保持默认，loaded=true", async () => {
    mockLoadSettings.mockResolvedValue({
      data: { terminalFontSize: 14 },
      corrupted: false,
    });
    await useSideBar.getState().loadFromDisk();
    const s = useSideBar.getState();
    expect(s.zones).toEqual(DEFAULT_ZONES);
    expect(s.open).toEqual(DEFAULT_OPEN);
    expect(s.loaded).toBe(true);
  });

  it("14. loadFromDisk 异常 → 保持默认，loaded=true", async () => {
    mockLoadSettings.mockRejectedValue(new Error("disk error"));
    await useSideBar.getState().loadFromDisk();
    const s = useSideBar.getState();
    expect(s.zones).toEqual(DEFAULT_ZONES);
    expect(s.open).toEqual(DEFAULT_OPEN);
    expect(s.width).toBe(WIDTH_DEFAULT);
    expect(s.splitRatio).toBe(SPLIT_DEFAULT);
    expect(s.loaded).toBe(true);
  });

  it("14b. loadFromDisk corrupted=true → 默认值 + toast 告警（FE-11）", async () => {
    mockLoadSettings.mockResolvedValue({ data: null, corrupted: true });
    await useSideBar.getState().loadFromDisk();
    const s = useSideBar.getState();
    expect(s.zones).toEqual(DEFAULT_ZONES);
    expect(s.open).toEqual(DEFAULT_OPEN);
    expect(s.loaded).toBe(true);
    // FE-11：配置损坏统一 toast 告警（warning + 固定文案）
    expect(mockToastShow).toHaveBeenCalledWith("warning", "配置已损坏，已回退默认值");
  });

  it("14c. loadFromDisk corrupted=true 且带数据 → 消费数据 + toast 告警（FE-11）", async () => {
    mockLoadSettings.mockResolvedValue({
      data: {
        sideBar: {
          zones: { top: ["explorer"], bottom: [] },
          open: { top: "explorer", bottom: null },
          width: 300,
          splitRatio: 0.6,
        },
      },
      corrupted: true,
    });
    await useSideBar.getState().loadFromDisk();
    // 损坏时后端返回默认/可用数据，仍正常消费
    expect(useSideBar.getState().width).toBe(300);
    expect(useSideBar.getState().splitRatio).toBe(0.6);
    expect(mockToastShow).toHaveBeenCalledWith("warning", "配置已损坏，已回退默认值");
  });

  it("15. loadFromDisk reconcileZones 过滤未注册 id + 补全缺失", async () => {
    // saved 引用 id "ghost"（未注册），应被过滤掉
    mockLoadSettings.mockResolvedValue({
      data: {
        sideBar: {
          zones: { top: ["ghost", "nav"], bottom: ["ghost"] },
          open: { top: "ghost", bottom: "ghost" },
          width: 250,
          splitRatio: 0.5,
        },
      },
      corrupted: false,
    });
    await useSideBar.getState().loadFromDisk();
    const s = useSideBar.getState();
    // "ghost" 被过滤掉，explorer、commit 补全到上区末尾
    expect(s.zones.top).toEqual(["nav", "explorer", "commit"]);
    expect(s.zones.bottom).toEqual([]);
    // open 指向未注册 id → 清 null
    expect(s.open).toEqual({ top: null, bottom: null });
  });

  it("15b. loadFromDisk 旧 settings（projects/agent-status 未注册 id）→ 丢弃回退三槽（NAV-07 迁移）", async () => {
    // Stage 05 前持久化的旧数据：projects/agent-status 视图已退役
    mockLoadSettings.mockResolvedValue({
      data: {
        sideBar: {
          zones: { top: ["projects", "agent-status", "explorer"], bottom: ["commit"] },
          open: { top: "agent-status", bottom: "commit" },
          width: 250,
          splitRatio: 0.5,
        },
      },
      corrupted: false,
    });
    await useSideBar.getState().loadFromDisk();
    const s = useSideBar.getState();
    // 未注册 id（projects/agent-status）全部丢弃；缺失的注册 id（nav）
    // 按 registeredIds 顺序补全上区末尾；已注册的 commit 留在 bottom 原位
    expect(s.zones).toEqual({ top: ["explorer", "nav"], bottom: ["commit"] });
    // open 指向被丢弃的 agent-status → 置 null；bottom 的 commit 打开态保留
    expect(s.open).toEqual({ top: null, bottom: "commit" });
    expect(s.loaded).toBe(true);
  });

  // ── 持久化 ──

  it("16. loaded=false 时不触发 saveSettings", () => {
    useSideBar.getState().setWidth(300);
    vi.advanceTimersByTime(2000);
    expect(mockSaveSettings).not.toHaveBeenCalled();
  });

  it("17. loaded=true 变更 → 2s debounce → saveSettings({sideBar}) payload 键集合精确匹配", () => {
    useSideBar.setState({ loaded: true });
    useSideBar.getState().setWidth(300);

    // 不到 2s 不触发
    vi.advanceTimersByTime(1500);
    expect(mockSaveSettings).not.toHaveBeenCalled();

    // 到达 2s 触发
    vi.advanceTimersByTime(600);
    expect(mockSaveSettings).toHaveBeenCalledTimes(1);

    const payload = mockSaveSettings.mock.calls[0][0];
    // 顶层仅 sideBar 一键（契约 [C]）
    expect(Object.keys(payload)).toEqual(["sideBar"]);
    // sideBar 内部精确四键
    const sb = payload["sideBar"] as Record<string, unknown>;
    expect(Object.keys(sb).sort()).toEqual(["open", "splitRatio", "width", "zones"].sort());
    // 值正确
    expect(sb["width"]).toBe(300);
    expect(sb["splitRatio"]).toBe(SPLIT_DEFAULT);
    expect(sb["zones"]).toEqual(DEFAULT_ZONES);
    expect(sb["open"]).toEqual(DEFAULT_OPEN);
  });

  it("18. 多次变更 → debounce 只写最后一次", () => {
    useSideBar.setState({ loaded: true });
    useSideBar.getState().setWidth(200);
    vi.advanceTimersByTime(500);
    useSideBar.getState().setWidth(300);
    vi.advanceTimersByTime(500);
    useSideBar.getState().setWidth(400);

    vi.advanceTimersByTime(2000);
    expect(mockSaveSettings).toHaveBeenCalledTimes(1);
    expect(mockSaveSettings.mock.calls[0][0]["sideBar"]["width"]).toBe(400);
  });

  it("19. saveSettings 失败 → toast 告警 + console.warn（FE-09），store 状态不受影响", async () => {
    mockSaveSettings.mockRejectedValue(new Error("write error"));
    mockGetErrorMessage.mockReturnValue("write error");
    useSideBar.setState({ loaded: true });

    const consoleWarnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    expect(() => {
      useSideBar.getState().setWidth(300);
    }).not.toThrow();
    // 异步推进 timer 并排空微任务：saveSettings reject 的 .catch 在微任务队列执行
    await vi.advanceTimersByTimeAsync(2000);

    // FE-09：保存失败统一 toast 告警（warning + 固定文案）
    expect(mockToastShow).toHaveBeenCalledWith("warning", "设置保存失败，重启后将丢失");
    // 错误详情统一经 getErrorMessage 提取后 console.warn 记录
    expect(mockGetErrorMessage).toHaveBeenCalledWith(expect.any(Error));
    expect(consoleWarnSpy).toHaveBeenCalledWith(
      expect.stringContaining("[stores/sideBar]"),
      "write error",
    );

    // store 状态仍正常
    expect(useSideBar.getState().width).toBe(300);

    consoleWarnSpy.mockRestore();
  });

  it("20. cancelPendingSave 取消活跃 timer——推进 2s 不再写盘", () => {
    useSideBar.setState({ loaded: true });
    useSideBar.getState().setWidth(300); // 产生 debounce timer
    vi.advanceTimersByTime(1500);        // 未到 2s，timer 仍活跃
    cancelPendingSave();                 // 关窗冲刷：取消待执行保存
    vi.advanceTimersByTime(2000);        // 越过原定触发点
    expect(mockSaveSettings).not.toHaveBeenCalled();
  });
});
