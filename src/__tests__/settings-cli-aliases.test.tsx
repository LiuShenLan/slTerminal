// settings-cli-aliases.test.tsx — 设置中心「CLI 别名」配置页 L2 测试
//
// mock 策略：真实 store + 真实校验纯函数 + 真实注册表（beforeEach 注册 light fake
// profile claude/codex 两分区，afterEach 隔离）。store loaded 守卫默认 false——
// addAlias/removeAlias 不触发 debounce 落盘（零 IPC，照 settings-keybindings.test.tsx 先例）。
// 覆盖：分区渲染 / 内置 chip 只读 / 别名 chip 增删 / Enter 与按钮提交 / 校验失败行内红字
// 保留输入 / blur 清空 / 跨 cli 冲突文案 / 空注册表空态。

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";
import { cliProfileRegistry } from "../features/cliProfiles/cliProfileRegistry";
import type { CodingCliProfile } from "../features/cliProfiles/types";
import { useCliAliases } from "../stores/cliAliases";
import CliAliasesPage from "../panels/settings/pages/CliAliasesPage";

/** light fake profile（iconSrc 不被 jsdom 解析） */
function fakeProfile(id: string, displayName: string): CodingCliProfile {
  return {
    id,
    displayName,
    commands: [id],
    iconSrc: `/cli-icons/${id}.png`,
    tabTitle: id,
    capabilities: {},
  };
}

/** data-e2e 查询 */
function byE2e(name: string): HTMLElement {
  const el = document.querySelector(`[data-e2e="${name}"]`);
  expect(el, `data-e2e=${name} 应存在`).not.toBeNull();
  return el as HTMLElement;
}

beforeEach(() => {
  cliProfileRegistry._reset();
  cliProfileRegistry.register(fakeProfile("claude", "Claude Code"));
  cliProfileRegistry.register(fakeProfile("codex", "Codex"));
  useCliAliases.setState({ aliases: {}, loaded: false });
});

afterEach(() => {
  cleanup();
  cliProfileRegistry._reset();
  useCliAliases.setState({ aliases: {}, loaded: false });
});

describe("分区渲染", () => {
  it("注册表驱动分区：claude + codex 两分区头（logo + displayName）", () => {
    render(<CliAliasesPage />);
    expect(document.querySelectorAll("[data-e2e^='cli-aliases-group-']")).toHaveLength(2);
    expect(byE2e("cli-aliases-group-claude").textContent).toContain("Claude Code");
    expect(byE2e("cli-aliases-group-codex").textContent).toContain("Codex");
    expect(byE2e("cli-aliases-group-claude").querySelector("img")).not.toBeNull();
  });

  it("内置命令 chip 只读渲染 + 命名空间占用提示，chip 内无删除钮", () => {
    render(<CliAliasesPage />);
    expect(byE2e("cli-aliases-builtin-claude-claude").textContent).toBe("claude");
    expect(byE2e("cli-aliases-builtin-codex-codex").textContent).toBe("codex");
    expect(
      document.querySelector("[data-e2e^='cli-aliases-builtin-'] [data-e2e^='cli-aliases-remove-']"),
    ).toBeNull();
    expect(byE2e("cli-aliases-group-claude").textContent).toContain(
      "内置命令已占用命名空间，不可配置为别名",
    );
  });

  it("别名 chips 按 store 渲染，空分区显示暂无别名", () => {
    useCliAliases.setState({ aliases: { claude: ["cc"] } });
    render(<CliAliasesPage />);
    expect(byE2e("cli-aliases-alias-claude-cc").textContent).toContain("cc");
    expect(byE2e("cli-aliases-alias-claude-cc").querySelector("button")).not.toBeNull();
    expect(byE2e("cli-aliases-group-codex").textContent).toContain("暂无别名");
  });
});

describe("添加别名", () => {
  it("输入 + 按钮提交 → store 追加 + input 清空", () => {
    render(<CliAliasesPage />);
    const input = byE2e("cli-aliases-input-claude");
    fireEvent.change(input, { target: { value: "cc" } });
    fireEvent.click(byE2e("cli-aliases-add-claude"));
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc"] });
    expect((byE2e("cli-aliases-input-claude") as HTMLInputElement).value).toBe("");
  });

  it("Enter 提交等价按钮", () => {
    render(<CliAliasesPage />);
    fireEvent.change(byE2e("cli-aliases-input-claude"), { target: { value: "c" } });
    fireEvent.keyDown(byE2e("cli-aliases-input-claude"), { key: "Enter" });
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["c"] });
  });

  it("连续添加追加不覆盖", () => {
    useCliAliases.setState({ aliases: { claude: ["cc"] } });
    render(<CliAliasesPage />);
    fireEvent.change(byE2e("cli-aliases-input-claude"), { target: { value: "c" } });
    fireEvent.keyDown(byE2e("cli-aliases-input-claude"), { key: "Enter" });
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc", "c"] });
  });

  it("校验失败 → 行内红字（本 cli 内置），input 保留可改，store 不变", () => {
    render(<CliAliasesPage />);
    fireEvent.change(byE2e("cli-aliases-input-claude"), { target: { value: "claude" } });
    fireEvent.keyDown(byE2e("cli-aliases-input-claude"), { key: "Enter" });
    expect(byE2e("cli-aliases-error-claude").textContent).toBe(
      "「claude」是本 CLI 的内置命令",
    );
    expect((byE2e("cli-aliases-input-claude") as HTMLInputElement).value).toBe("claude");
    expect(useCliAliases.getState().aliases).toEqual({});
  });

  it("跨 cli 冲突 → 行内红字指向占用的 displayName", () => {
    useCliAliases.setState({ aliases: { claude: ["cc"] } });
    render(<CliAliasesPage />);
    fireEvent.change(byE2e("cli-aliases-input-codex"), { target: { value: "cc" } });
    fireEvent.keyDown(byE2e("cli-aliases-input-codex"), { key: "Enter" });
    expect(byE2e("cli-aliases-error-codex").textContent).toBe(
      "「cc」已被「Claude Code」的别名使用",
    );
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc"] });
  });

  it("失败后改输入重试成功 → 错误清除 + chip 出现", () => {
    render(<CliAliasesPage />);
    fireEvent.change(byE2e("cli-aliases-input-claude"), { target: { value: "claude" } });
    fireEvent.keyDown(byE2e("cli-aliases-input-claude"), { key: "Enter" });
    expect(byE2e("cli-aliases-error-claude")).not.toBeNull();
    // 改输入即清错（onChange 清 error）
    fireEvent.change(byE2e("cli-aliases-input-claude"), { target: { value: "cc" } });
    expect(document.querySelector("[data-e2e='cli-aliases-error-claude']")).toBeNull();
    fireEvent.keyDown(byE2e("cli-aliases-input-claude"), { key: "Enter" });
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc"] });
  });

  it("blur 清空 input 与错误（不提交）", () => {
    render(<CliAliasesPage />);
    fireEvent.change(byE2e("cli-aliases-input-claude"), { target: { value: "claude" } });
    fireEvent.keyDown(byE2e("cli-aliases-input-claude"), { key: "Enter" });
    expect(byE2e("cli-aliases-error-claude")).not.toBeNull();
    fireEvent.blur(byE2e("cli-aliases-input-claude"));
    expect((byE2e("cli-aliases-input-claude") as HTMLInputElement).value).toBe("");
    expect(document.querySelector("[data-e2e='cli-aliases-error-claude']")).toBeNull();
    expect(useCliAliases.getState().aliases).toEqual({});
  });
});

describe("删除别名", () => {
  it("chip × 点击 → store 移除", () => {
    useCliAliases.setState({ aliases: { claude: ["cc", "c"] } });
    render(<CliAliasesPage />);
    fireEvent.click(byE2e("cli-aliases-remove-claude-cc"));
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["c"] });
    // DOM 同步：chips 更新
    expect(document.querySelector("[data-e2e='cli-aliases-alias-claude-cc']")).toBeNull();
  });
});

describe("空态", () => {
  it("注册表空（无 CLI）→ 整页空态文案", () => {
    cliProfileRegistry._reset();
    render(<CliAliasesPage />);
    expect(document.querySelector("[data-e2e='settings-cli-aliases-page']")?.textContent).toContain(
      "无可用 CLI",
    );
  });
});
