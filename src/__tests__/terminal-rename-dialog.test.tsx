// terminal-rename-dialog.test.tsx — TerminalRenameDialog L2 测试
//
// 覆盖：标题/输入框预填/确定/取消（按钮/Esc/遮罩）、Enter 提交（trim）、
// 空名拒绝（行内错误 + 弹窗保持打开 + onConfirm 未调）、错误清除、
// initialTitle 变化跟随。纯受控展示组件，零 IPC mock。

import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";
import { TerminalRenameDialog } from "../workspace/TerminalRenameDialog";

afterEach(cleanup);

/** 渲染弹窗，返回 dialog/input 元素与 mock 回调 */
function renderDialog(initialTitle = "terminal-0") {
  const onConfirm = vi.fn();
  const onCancel = vi.fn();
  const utils = render(
    <TerminalRenameDialog
      initialTitle={initialTitle}
      onConfirm={onConfirm}
      onCancel={onCancel}
    />,
  );
  const dialog = utils.container.querySelector(
    '[data-e2e="terminal-rename-dialog"]',
  ) as HTMLElement;
  const input = utils.container.querySelector(
    '[data-e2e="terminal-rename-input"]',
  ) as HTMLInputElement;
  return { ...utils, dialog, input, onConfirm, onCancel };
}

/** 输入新值并提交（模拟输入 + Enter） */
function typeAndEnter(input: HTMLInputElement, value: string) {
  fireEvent.change(input, { target: { value } });
  fireEvent.keyDown(input, { key: "Enter" });
}

describe("TerminalRenameDialog 渲染", () => {
  it("渲染标题 + 输入框 + 确定/取消按钮 + data-e2e 属性", () => {
    const { getByText, dialog, input } = renderDialog();

    expect(dialog).toBeTruthy();
    expect(getByText("重命名终端")).toBeTruthy();
    expect(input).toBeTruthy();
    expect(getByText("确定")).toBeTruthy();
    expect(getByText("取消")).toBeTruthy();
  });

  it("输入框预填 initialTitle", () => {
    const { input } = renderDialog("我的终端");
    expect(input.value).toBe("我的终端");
  });

  it("输入框受控：change 更新 value", () => {
    const { input } = renderDialog();
    fireEvent.change(input, { target: { value: "新名" } });
    expect(input.value).toBe("新名");
  });

  it("initialTitle 变化 → 预填跟随（rerender）", () => {
    const { rerender, input } = renderDialog("terminal-0");
    rerender(
      <TerminalRenameDialog
        initialTitle="terminal-1"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(input.value).toBe("terminal-1");
  });
});

describe("TerminalRenameDialog 提交", () => {
  it("Enter 提交 → onConfirm(trimmed)", () => {
    const { input, onConfirm } = renderDialog();
    typeAndEnter(input, "我的终端");
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onConfirm).toHaveBeenCalledWith("我的终端");
  });

  it("提交值自动 trim（'  a  ' → 'a'）", () => {
    const { input, onConfirm } = renderDialog();
    typeAndEnter(input, "  a  ");
    expect(onConfirm).toHaveBeenCalledWith("a");
  });

  it("点击确定按钮 → onConfirm", () => {
    const { input, getByText, onConfirm } = renderDialog();
    fireEvent.change(input, { target: { value: "x" } });
    fireEvent.click(getByText("确定"));
    expect(onConfirm).toHaveBeenCalledWith("x");
  });

  it("空串提交 → 错误文案 + onConfirm 未调 + 弹窗保持打开", () => {
    const { input, getByText, onConfirm, dialog } = renderDialog();
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(getByText("名称不能为空")).toBeTruthy();
    expect(onConfirm).not.toHaveBeenCalled();
    expect(dialog).toBeTruthy();
  });

  it("纯空白提交 → 同样拒绝（trim 后空）", () => {
    const { input, getByText, onConfirm } = renderDialog();
    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(getByText("名称不能为空")).toBeTruthy();
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("错误出现后输入合法字符 → 错误清除", () => {
    const { input, queryByText } = renderDialog();
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(queryByText("名称不能为空")).toBeTruthy();

    fireEvent.change(input, { target: { value: "y" } });
    expect(queryByText("名称不能为空")).toBeNull();
  });
});

describe("TerminalRenameDialog 取消", () => {
  it("点击取消按钮 → onCancel", () => {
    const { getByText, onCancel } = renderDialog();
    fireEvent.click(getByText("取消"));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("Esc 键 → onCancel", () => {
    const { onCancel } = renderDialog();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("遮罩点击 → onCancel（点面板内部不触发）", () => {
    const { dialog, onCancel } = renderDialog();
    // 遮罩自身命中（e.target === e.currentTarget）
    fireEvent.mouseDown(dialog);
    expect(onCancel).toHaveBeenCalledTimes(1);

    // 面板内部点击不取消
    onCancel.mockClear();
    const panel = dialog.querySelector("div") as HTMLElement;
    fireEvent.mouseDown(panel);
    expect(onCancel).not.toHaveBeenCalled();
  });
});
