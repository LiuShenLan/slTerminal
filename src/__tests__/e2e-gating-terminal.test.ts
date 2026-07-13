// e2e-gating-terminal.test.ts — 终端 hooks 的 E2E_ENABLED 门控双路径（T2）
//
// 覆盖 useTerminalInstance（initTerminalE2e）与 useXterm（setTerminalSessionReady /
// installTerminalWriteToPty / setTerminalSessionError）共 4 处门控点：
//   - E2E_ENABLED=false（生产语义）→ 4 个 helper 均不调用
//   - E2E_ENABLED=true → 对应 helper 按时机调用
// getter 切换开关；mock 骨架对齐 use-xterm-lifecycle.test.ts。

/* eslint-disable @typescript-eslint/no-explicit-any */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// ─── E2E 门控开关（getter 逐用例切换）───
const gate = vi.hoisted(() => ({ enabled: false }));
vi.mock("../lib/e2eEnabled", () => ({
  get E2E_ENABLED() {
    return gate.enabled;
  },
  computeE2eEnabled: (dev: boolean, viteE2e: string | undefined) =>
    dev || viteE2e === "1",
}));

// ─── E2E helper 桩（spy 4 个门控函数，其余保留真实实现）───
const helperSpies = vi.hoisted(() => ({
  initTerminalE2e: vi.fn(),
  installTerminalWriteToPty: vi.fn(),
  setTerminalSessionReady: vi.fn(),
  setTerminalSessionError: vi.fn(),
}));
vi.mock("../../e2e-tests/helpers", async (importOriginal) => {
  const actual = await importOriginal<Record<string, unknown>>();
  return { ...actual, ...helperSpies };
});

// ─── 低层依赖 mock（对齐 use-xterm-lifecycle.test.ts）───
const {
  mockRegister,
  mockUnregisterFn,
  mockResolve,
  mockFit,
  mockProposeDimensions,
} = vi.hoisted(() => ({
  mockRegister: vi.fn(() => vi.fn()),
  mockUnregisterFn: vi.fn(),
  mockResolve: vi.fn<(e: KeyboardEvent, ctx?: string) => boolean>(() => false),
  mockFit: vi.fn(),
  mockProposeDimensions: vi.fn(() => ({ cols: 80, rows: 24 })),
}));

vi.mock("@xterm/xterm", () => {
  class MockTerminal {
    element: any;
    open = vi.fn();
    loadAddon = vi.fn();
    dispose = vi.fn();
    onData = vi.fn(() => ({ dispose: vi.fn() }));
    write = vi.fn();
    writeln = vi.fn();
    focus = vi.fn();
    getSelection = vi.fn(() => "");
    paste = vi.fn();
    attachCustomKeyEventHandler = vi.fn();
    options: Record<string, unknown> = {};
    parser = {
      registerOscHandler: vi.fn(() => ({ dispose: vi.fn() })),
    };
    constructor() {
      this.element = document.createElement("div");
    }
  }
  return { Terminal: MockTerminal };
});

const { mockUsePanelFocus } = vi.hoisted(() => ({
  mockUsePanelFocus: vi.fn(),
}));
vi.mock("../features/shortcuts", () => ({
  usePanelFocus: mockUsePanelFocus,
  getShortcutRegistry: () => ({
    register: mockRegister,
    unregister: mockUnregisterFn,
    pushContext: vi.fn(),
    popContext: vi.fn(),
    resolve: mockResolve,
    _reset: vi.fn(),
  }),
}));

vi.mock("../ipc", () => ({
  pty: {
    spawn: vi.fn().mockResolvedValue("test-session-id"),
    write: vi.fn().mockResolvedValue(undefined),
    resize: vi.fn().mockResolvedValue(undefined),
    kill: vi.fn().mockResolvedValue(undefined),
    getWindowsBuildNumber: vi.fn().mockResolvedValue(22621),
  },
}));

vi.mock("../ipc/clipboard", () => ({
  writeText: vi.fn(),
  readText: vi.fn(() => Promise.resolve("")),
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit = mockFit;
    proposeDimensions = mockProposeDimensions;
    dispose = vi.fn();
  },
}));

vi.mock("@xterm/addon-webgl", () => ({
  WebglAddon: class {
    dispose = vi.fn();
    onContextLoss = vi.fn();
  },
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(() => Promise.resolve()),
}));

vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    register: vi.fn(),
    get: vi.fn(),
    remove: vi.fn(),
  },
}));

vi.mock("../panels/terminal/TabTitleRegistry", () => ({
  tabTitleRegistry: { match: vi.fn(), register: vi.fn(), _reset: vi.fn() },
}));

vi.mock("../panels/terminal/tabRules", () => ({}));

// ─── 被测模块（mocks 就绪后）───
import { useXterm } from "../panels/terminal/useXterm";
import { pty } from "../ipc";
import { createContainer, mockRaf, type RafMock } from "./helpers/xterm-test-utils";

describe("终端 hooks — E2E_ENABLED 门控 helper 注入", () => {
  let container: HTMLDivElement;
  let raf: RafMock;

  beforeEach(() => {
    vi.clearAllMocks();
    container = createContainer();
    raf = mockRaf();
    (pty.spawn as any).mockResolvedValue("test-session-id");
  });

  afterEach(() => {
    raf.cleanup();
  });

  const renderTerminal = () =>
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "gate-test" }),
    );

  it("禁用（false，spawn 成功）→ 4 个 helper 均不调用", async () => {
    gate.enabled = false;
    renderTerminal();
    raf.flush();
    await waitFor(() => expect(pty.spawn).toHaveBeenCalled());

    expect(helperSpies.initTerminalE2e).not.toHaveBeenCalled();
    expect(helperSpies.installTerminalWriteToPty).not.toHaveBeenCalled();
    expect(helperSpies.setTerminalSessionReady).not.toHaveBeenCalled();
    expect(helperSpies.setTerminalSessionError).not.toHaveBeenCalled();
  });

  it("禁用（false，spawn 失败）→ setTerminalSessionError 仍不调用", async () => {
    gate.enabled = false;
    (pty.spawn as any).mockRejectedValueOnce(new Error("boom"));
    renderTerminal();
    raf.flush();
    await waitFor(() => expect(pty.spawn).toHaveBeenCalled());

    expect(helperSpies.setTerminalSessionError).not.toHaveBeenCalled();
  });

  it("启用（true，spawn 前）→ initTerminalE2e / setTerminalSessionReady / installTerminalWriteToPty 调用", () => {
    gate.enabled = true;
    renderTerminal(); // 不 flush rAF：仅验证 spawn 前的同步注入

    expect(helperSpies.initTerminalE2e).toHaveBeenCalled();
    expect(helperSpies.installTerminalWriteToPty).toHaveBeenCalled();
    expect(helperSpies.setTerminalSessionReady).toHaveBeenCalledWith(
      container,
      false,
    );
  });

  it("启用（true，spawn 成功）→ setTerminalSessionReady(container, true)", async () => {
    gate.enabled = true;
    renderTerminal();
    raf.flush();
    await waitFor(() =>
      expect(helperSpies.setTerminalSessionReady).toHaveBeenCalledWith(
        container,
        true,
      ),
    );
  });

  it("启用（true，spawn 失败）→ setTerminalSessionError 调用", async () => {
    gate.enabled = true;
    (pty.spawn as any).mockRejectedValueOnce(new Error("boom"));
    renderTerminal();
    raf.flush();
    await waitFor(() =>
      expect(helperSpies.setTerminalSessionError).toHaveBeenCalled(),
    );
  });
});
