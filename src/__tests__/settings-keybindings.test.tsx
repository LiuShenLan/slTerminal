// settings-keybindings.test.tsx — 设置中心「快捷键」配置页 L2 测试（F11，SC-FE-09）
//
// 覆盖：分组渲染 / override 高亮+默认小字 / 未绑定占位 / 录制 Esc 取消 /
// Backspace 解绑 / 纯修饰键忽略 / 保留键红字不写入 / 冲突警告放行 / 合法写入 setBinding /
// ↺ clearBinding / 卸载清 suspended / findConflict 纯函数。
//
// mock 策略：真实注册表单例（注册 COMMAND_CATALOG + 桩 handler）+ 真实 keybindings store，
// 经 wireKeybindings（App.tsx 同款接线）同步 store→registry——测试环境即生产装配。
// store loaded 守卫默认 false，setBinding 不触发 debounce 落盘（零 IPC）。

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent, waitFor, act } from "@testing-library/react";
import { getShortcutRegistry } from "../features/shortcuts/ShortcutRegistry";
import { COMMAND_CATALOG } from "../features/shortcuts/commandCatalog";
import { wireKeybindings } from "../features/shortcuts/wireKeybindings";
import type { Command } from "../features/shortcuts/types";
import { useKeybindings } from "../stores/keybindings";
import KeybindingsPage, { findConflict } from "../panels/settings/pages/KeybindingsPage";
import { makeKeydown } from "./helpers/keyboard";

/** 目录元数据 → 桩 handler 命令（匹配测试不关心 handler 行为） */
function makeCommands(): Command[] {
  return COMMAND_CATALOG.map((meta) => ({ ...meta, handler: () => true }));
}

const registry = getShortcutRegistry();

let unregs: (() => void)[] = [];
let unregWire: (() => void) | null = null;

beforeEach(() => {
  registry._reset();
  useKeybindings.setState({ overrides: {} });
  const unreg = registry.register(makeCommands());
  unregs.push(unreg);
  unregWire = wireKeybindings(registry, useKeybindings);
});

afterEach(() => {
  cleanup();
  unregWire?.();
  unregWire = null;
  for (const h of unregs) {
    try { h(); } catch { /* 幂等 */ }
  }
  unregs = [];
  registry._reset();
  registry.setCaptureSuspended(false);
  useKeybindings.setState({ overrides: {} });
});

/** 行容器查询（data-e2e 属性） */
function row(id: string): HTMLElement {
  const el = document.querySelector(`[data-e2e="kb-row-${id}"]`);
  expect(el).not.toBeNull();
  return el as HTMLElement;
}

/** 行内子元素查询 */
function inRow(id: string, sel: string): Element | null {
  return row(id).querySelector(sel);
}

/** 录制态窗口按键（act 包裹保证 setState 在 act 内） */
function pressWindow(opts: { code?: string; ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean; metaKey?: boolean }) {
  act(() => {
    window.dispatchEvent(makeKeydown(opts));
  });
}

describe("分组渲染", () => {
  it("按 category 分组（目录序 global/terminal/editor/explorer），9 条命令全渲染", () => {
    render(<KeybindingsPage />);
    const groupEls = Array.from(document.querySelectorAll("[data-e2e^='kb-group-']"));
    expect(groupEls.map((el) => el.getAttribute("data-e2e"))).toEqual([
      "kb-group-global",
      "kb-group-terminal",
      "kb-group-editor",
      "kb-group-explorer",
    ]);
    expect(document.querySelectorAll("[data-e2e^='kb-row-']")).toHaveLength(
      COMMAND_CATALOG.length,
    );
    // 各类代表命令标题存在
    expect(row("global.closeTab").textContent).toContain("关闭当前页签");
    expect(row("terminal.copy").textContent).toContain("复制选区");
    expect(row("editor.save").textContent).toContain("保存文件");
    expect(row("explorer.open").textContent).toContain("打开文件");
  });

  it("默认键直接显示为生效键（无覆盖不高亮、无 ↺）", () => {
    render(<KeybindingsPage />);
    expect(inRow("editor.save", "[data-e2e='kb-effective']")?.textContent).toBe("Ctrl+KeyS");
    expect(inRow("editor.save", "[data-e2e='kb-reset']")).toBeNull();
    expect(inRow("editor.save", "[data-e2e='kb-default']")).toBeNull();
  });
});

describe("override 显示", () => {
  it("override 生效键高亮 + ↺ 复位钮 + 默认键小字", () => {
    useKeybindings.setState({ overrides: { "editor.save": "Ctrl+Alt+KeyS" } });
    render(<KeybindingsPage />);
    expect(inRow("editor.save", "[data-e2e='kb-effective']")?.textContent).toBe("Ctrl+Alt+KeyS");
    expect(inRow("editor.save", "[data-e2e='kb-reset']")).not.toBeNull();
    expect(inRow("editor.save", "[data-e2e='kb-default']")?.textContent).toBe("Ctrl+KeyS");
    // 未覆盖行不显示 ↺ 与默认小字
    expect(inRow("global.closeTab", "[data-e2e='kb-reset']")).toBeNull();
    expect(inRow("global.closeTab", "[data-e2e='kb-default']")).toBeNull();
  });

  it("override 解绑（null）→ 未绑定占位", () => {
    useKeybindings.setState({ overrides: { "editor.save": null } });
    render(<KeybindingsPage />);
    expect(inRow("editor.save", "[data-e2e='kb-unbound']")?.textContent).toBe("未绑定");
    expect(inRow("editor.save", "[data-e2e='kb-effective']")).toBeNull();
    expect(inRow("editor.save", "[data-e2e='kb-reset']")).toBeNull();
  });
});

describe("录制态", () => {
  it("行点击进入录制（行提示 + 不写入）", () => {
    render(<KeybindingsPage />);
    fireEvent.click(row("editor.save"));
    expect(document.querySelector("[data-e2e='kb-recording-hint']")).not.toBeNull();
    expect(useKeybindings.getState().overrides["editor.save"]).toBeUndefined();
  });

  it("Esc 取消录制：退出录制 + suspended 复位 + 不写入", async () => {
    render(<KeybindingsPage />);
    fireEvent.click(row("editor.save"));
    pressWindow({ code: "Escape" });
    await waitFor(() => {
      expect(document.querySelector("[data-e2e='kb-recording-hint']")).toBeNull();
    });
    expect(useKeybindings.getState().overrides["editor.save"]).toBeUndefined();
    // suspended 复位（行为断言：命令快捷键恢复消费）
    registry.pushContext("editor");
    const e = makeKeydown({ ctrlKey: true, code: "KeyS" });
    act(() => {
      window.dispatchEvent(e);
    });
    expect(e.defaultPrevented).toBe(true);
  });

  it("Backspace 解绑：setBinding(id, null) + 退出录制", async () => {
    render(<KeybindingsPage />);
    fireEvent.click(row("editor.save"));
    pressWindow({ code: "Backspace" });
    await waitFor(() => {
      expect(useKeybindings.getState().overrides["editor.save"]).toBeNull();
    });
    await waitFor(() => {
      expect(document.querySelector("[data-e2e='kb-recording-hint']")).toBeNull();
    });
    // 行显示未绑定
    expect(inRow("editor.save", "[data-e2e='kb-unbound']")).not.toBeNull();
  });

  it("Delete 同样解绑", async () => {
    render(<KeybindingsPage />);
    fireEvent.click(row("explorer.delete"));
    pressWindow({ code: "Delete" });
    await waitFor(() => {
      expect(useKeybindings.getState().overrides["explorer.delete"]).toBeNull();
    });
  });

  it("纯修饰键忽略：保持录制、不写入", () => {
    render(<KeybindingsPage />);
    fireEvent.click(row("editor.save"));
    pressWindow({ code: "ControlLeft" });
    pressWindow({ code: "ShiftRight" });
    expect(document.querySelector("[data-e2e='kb-recording-hint']")).not.toBeNull();
    expect(useKeybindings.getState().overrides["editor.save"]).toBeUndefined();
  });

  it("IME 组合态跳过（isComposing 不视为录制按键）", () => {
    render(<KeybindingsPage />);
    fireEvent.click(row("editor.save"));
    act(() => {
      window.dispatchEvent(makeKeydown({ ctrlKey: true, code: "KeyS", isComposing: true }));
    });
    expect(document.querySelector("[data-e2e='kb-recording-hint']")).not.toBeNull();
    expect(useKeybindings.getState().overrides["editor.save"]).toBeUndefined();
  });

  it("保留键 → 行内红字拒绝，不写入、保持录制", async () => {
    render(<KeybindingsPage />);
    fireEvent.click(row("terminal.copy")); // terminal 上下文
    pressWindow({ ctrlKey: true, code: "KeyC" }); // Ctrl+C 终端保留（SIGINT）
    await waitFor(() => {
      expect(document.querySelector("[data-e2e='kb-error-terminal.copy']")).not.toBeNull();
    });
    const err = document.querySelector("[data-e2e='kb-error-terminal.copy']");
    expect(err?.textContent).toContain("保留键");
    expect(useKeybindings.getState().overrides["terminal.copy"]).toBeUndefined();
    // 保持录制等下一键
    expect(document.querySelector("[data-e2e='kb-recording-hint']")).not.toBeNull();
  });

  it("冲突警告放行：同 context 生效键相同 → 警告 + 允许写入 + 退出录制", async () => {
    render(<KeybindingsPage />);
    fireEvent.click(row("editor.save"));
    pressWindow({ altKey: true, code: "KeyZ" }); // = editor.toggleWordWrap 默认键
    await waitFor(() => {
      expect(document.querySelector("[data-e2e='kb-warning-editor.save']")).not.toBeNull();
    });
    const warn = document.querySelector("[data-e2e='kb-warning-editor.save']");
    expect(warn?.textContent).toContain("与「切换自动换行」冲突");
    expect(warn?.textContent).toContain("生效按优先级派发");
    await waitFor(() => {
      expect(useKeybindings.getState().overrides["editor.save"]).toBe("Alt+KeyZ");
    });
    await waitFor(() => {
      expect(document.querySelector("[data-e2e='kb-recording-hint']")).toBeNull();
    });
  });

  it("合法键位 → setBinding(id, formatKeystroke(ks)) + 行内更新", async () => {
    render(<KeybindingsPage />);
    fireEvent.click(row("global.closeTab"));
    pressWindow({ ctrlKey: true, altKey: true, code: "KeyC" });
    await waitFor(() => {
      expect(useKeybindings.getState().overrides["global.closeTab"]).toBe("Ctrl+Alt+KeyC");
    });
    // 行内显示新键 + ↺ + 默认小字
    expect(inRow("global.closeTab", "[data-e2e='kb-effective']")?.textContent).toBe("Ctrl+Alt+KeyC");
    expect(inRow("global.closeTab", "[data-e2e='kb-reset']")).not.toBeNull();
    expect(inRow("global.closeTab", "[data-e2e='kb-default']")?.textContent).toBe("Ctrl+KeyW");
  });

  it("↺ 复位：clearBinding 回退默认键，且不进入录制", async () => {
    useKeybindings.setState({ overrides: { "global.closeTab": "Ctrl+Alt+KeyC" } });
    render(<KeybindingsPage />);
    fireEvent.click(inRow("global.closeTab", "[data-e2e='kb-reset']")!);
    await waitFor(() => {
      expect(useKeybindings.getState().overrides["global.closeTab"]).toBeUndefined();
    });
    expect(inRow("global.closeTab", "[data-e2e='kb-reset']")).toBeNull();
    expect(inRow("global.closeTab", "[data-e2e='kb-effective']")?.textContent).toBe("Ctrl+KeyW");
    expect(document.querySelector("[data-e2e='kb-recording-hint']")).toBeNull();
  });

  it("卸载兜底清 suspended：卸载后命令快捷键恢复消费", async () => {
    const { unmount } = render(<KeybindingsPage />);
    fireEvent.click(row("editor.save"));
    expect(document.querySelector("[data-e2e='kb-recording-hint']")).not.toBeNull();
    unmount();
    registry.pushContext("editor");
    // suspended 复位 → editor.save 的 Ctrl+S 恢复消费；录制监听已移除（不写入）
    const e = makeKeydown({ ctrlKey: true, code: "KeyS" });
    act(() => {
      window.dispatchEvent(e);
    });
    expect(e.defaultPrevented).toBe(true);
    expect(useKeybindings.getState().overrides["editor.save"]).toBeUndefined();
  });
});

describe("findConflict 纯函数", () => {
  const ctrlEnter = { ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, code: "Enter" };
  const getEffective = (id: string) => registry.getEffectiveKeystroke(id);

  it("同 context 他命令生效键相同 → 返回冲突命令", () => {
    const conflict = findConflict(COMMAND_CATALOG, getEffective, "terminal.copy", ctrlEnter);
    expect(conflict?.id).toBe("terminal.newline");
  });

  it("不同 context 生效键相同 → 不冲突", () => {
    const conflict = findConflict(
      COMMAND_CATALOG,
      getEffective,
      "editor.save",
      { ctrlKey: true, shiftKey: true, altKey: false, metaKey: false, code: "KeyC" }, // terminal.copy 的默认键
    );
    expect(conflict).toBeNull();
  });

  it("排除自身", () => {
    const conflict = findConflict(COMMAND_CATALOG, getEffective, "terminal.newline", ctrlEnter);
    expect(conflict).toBeNull();
  });

  it("他命令解绑（effective=null）→ 不冲突", () => {
    useKeybindings.setState({ overrides: { "terminal.newline": null } }); // wire 同步 registry
    const conflict = findConflict(COMMAND_CATALOG, getEffective, "terminal.copy", ctrlEnter);
    expect(conflict).toBeNull();
  });

  it("未知 id → null", () => {
    const conflict = findConflict(COMMAND_CATALOG, getEffective, "no.such", ctrlEnter);
    expect(conflict).toBeNull();
  });
});
