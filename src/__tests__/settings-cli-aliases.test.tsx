// settings-cli-aliases.test.tsx — 设置中心「CLI 别名」配置节 L2 测试（ADR-0023 迁移）
//
// 被测 = CliAliasSection（单 cliId 形态，自原 CliAliasesPage 分区抽取——分区头
// logo/displayName 职责上移至 Agent 组分节标题，本节只管 chips + 添加行）。
// mock 策略：真实 store + 真实校验纯函数 + 真实注册表（beforeEach 注册 light fake
// profile claude/codex——跨 cli 冲突校验需要双 profile；afterEach 隔离）。
// store loaded 守卫默认 false——addAlias/removeAlias 不触发 debounce 落盘（零 IPC，
// 照 settings-keybindings.test.tsx 先例）。
// 覆盖：内置 chip 只读 / 别名 chip 增删 / Enter 与按钮提交（按钮用例走真实鼠标手势
// 序列：focus→输入→blur→click——blur 先于 click 是按钮路径的真实时序，FC-01）/
// 校验失败行内红字保留输入 / blur 保留草稿与错误 / 跨 cli 冲突文案 / 未注册防御。

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";
import { cliProfileRegistry } from "../features/cliProfiles/cliProfileRegistry";
import type { CodingCliProfile } from "../features/cliProfiles/types";
import { useCliAliases } from "../stores/cliAliases";
import CliAliasSection from "../panels/settings/pages/CliAliasSection";

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

function renderSection(cliId = "claude") {
  return render(<CliAliasSection cliId={cliId} />);
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

describe("渲染", () => {
  it("内置命令 chip 只读渲染 + 命名空间占用提示，chip 内无删除钮", () => {
    renderSection();
    expect(byE2e("cli-aliases-builtin-claude-claude").textContent).toBe("claude");
    expect(
      document.querySelector("[data-e2e^='cli-aliases-builtin-'] [data-e2e^='cli-aliases-remove-']"),
    ).toBeNull();
    expect(byE2e("cli-aliases-group-claude").textContent).toContain(
      "内置命令已占用命名空间，不可配置为别名",
    );
  });

  it("别名 chips 按 store 渲染，空名单显示暂无别名", () => {
    useCliAliases.setState({ aliases: { claude: ["cc"] } });
    renderSection();
    expect(byE2e("cli-aliases-alias-claude-cc").textContent).toContain("cc");
    expect(byE2e("cli-aliases-alias-claude-cc").querySelector("button")).not.toBeNull();

    cleanup();
    useCliAliases.setState({ aliases: {} });
    renderSection("codex");
    expect(byE2e("cli-aliases-group-codex").textContent).toContain("暂无别名");
  });

  it("profile 未注册 → 「未知 CLI」防御占位", () => {
    renderSection("ghost");
    expect(document.body.textContent).toContain("未知 CLI");
  });
});

describe("添加别名", () => {
  it("真实鼠标手势点按钮（mousedown 失焦 blur 先于 click）→ 添加成功不被清空草稿所败", () => {
    renderSection();
    const input = byE2e("cli-aliases-input-claude");
    // 真实手势序列：mousedown 转移焦点先触发 input blur，随后 mouseup/click 触发提交。
    // 回归防护：blur 曾无条件清空草稿 → click 提交空串报「别名不能为空」添加失败（FC-01）
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "cc" } });
    fireEvent.blur(input);
    fireEvent.click(byE2e("cli-aliases-add-claude"));
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc"] });
    expect((byE2e("cli-aliases-input-claude") as HTMLInputElement).value).toBe("");
  });

  it("Enter 提交等价按钮", () => {
    renderSection();
    fireEvent.change(byE2e("cli-aliases-input-claude"), { target: { value: "c" } });
    fireEvent.keyDown(byE2e("cli-aliases-input-claude"), { key: "Enter" });
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["c"] });
  });

  it("连续添加追加不覆盖", () => {
    useCliAliases.setState({ aliases: { claude: ["cc"] } });
    renderSection();
    fireEvent.change(byE2e("cli-aliases-input-claude"), { target: { value: "c" } });
    fireEvent.keyDown(byE2e("cli-aliases-input-claude"), { key: "Enter" });
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc", "c"] });
  });

  it("校验失败 → 行内红字（本 cli 内置），input 保留可改，store 不变", () => {
    renderSection();
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
    renderSection("codex");
    fireEvent.change(byE2e("cli-aliases-input-codex"), { target: { value: "cc" } });
    fireEvent.keyDown(byE2e("cli-aliases-input-codex"), { key: "Enter" });
    expect(byE2e("cli-aliases-error-codex").textContent).toBe(
      "「cc」已被「Claude Code」的别名使用",
    );
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc"] });
  });

  it("失败后改输入重试成功 → 错误清除 + chip 出现", () => {
    renderSection();
    fireEvent.change(byE2e("cli-aliases-input-claude"), { target: { value: "claude" } });
    fireEvent.keyDown(byE2e("cli-aliases-input-claude"), { key: "Enter" });
    expect(byE2e("cli-aliases-error-claude")).not.toBeNull();
    // 改输入即清错（onChange 清 error）
    fireEvent.change(byE2e("cli-aliases-input-claude"), { target: { value: "cc" } });
    expect(document.querySelector("[data-e2e='cli-aliases-error-claude']")).toBeNull();
    fireEvent.keyDown(byE2e("cli-aliases-input-claude"), { key: "Enter" });
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc"] });
  });

  it("blur 保留 input 草稿与错误（不提交，FC-01 防复发）", () => {
    renderSection();
    fireEvent.change(byE2e("cli-aliases-input-claude"), { target: { value: "claude" } });
    fireEvent.keyDown(byE2e("cli-aliases-input-claude"), { key: "Enter" });
    expect(byE2e("cli-aliases-error-claude")).not.toBeNull();
    fireEvent.blur(byE2e("cli-aliases-input-claude"));
    // 反向断言：blur 不清空草稿（输入可继续改）、不清错误、不提交——曾无条件清空致
    // 按钮点击先失焦后提交必败（FC-01），此三条断言即防该回归
    expect((byE2e("cli-aliases-input-claude") as HTMLInputElement).value).toBe("claude");
    expect(byE2e("cli-aliases-error-claude")).not.toBeNull();
    expect(useCliAliases.getState().aliases).toEqual({});
  });
});

describe("删除别名", () => {
  it("chip × 点击 → store 移除", () => {
    useCliAliases.setState({ aliases: { claude: ["cc", "c"] } });
    renderSection();
    fireEvent.click(byE2e("cli-aliases-remove-claude-cc"));
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["c"] });
    // DOM 同步：chips 更新
    expect(document.querySelector("[data-e2e='cli-aliases-alias-claude-cc']")).toBeNull();
  });
});
