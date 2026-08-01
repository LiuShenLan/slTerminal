// claude-history-input-dialog.test.tsx — InputDialog L2 测试（FE-07 / FE-10）
//
// 覆盖：初始值渲染、Enter 提交（trim 后）、Escape/遮罩点击/取消按钮 → onCancel、
// 空值禁确认（按钮 disabled + Enter 双保险）、挂载自动 focus + 全选。

import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";
import { InputDialog } from "../features/claudeHistory/InputDialog";

afterEach(cleanup);

/** 渲染弹窗，返回 input/按钮元素与 mock 回调 */
function renderDialog(props: Partial<Parameters<typeof InputDialog>[0]> = {}) {
  const onSubmit = vi.fn();
  const onCancel = vi.fn();
  const utils = render(
    <InputDialog
      title="重命名会话"
      initialValue="旧标题"
      onSubmit={onSubmit}
      onCancel={onCancel}
      {...props}
    />,
  );
  const root = utils.container.querySelector(
    '[data-e2e="agent-history-input-dialog"]',
  ) as HTMLElement;
  const input = utils.container.querySelector("input") as HTMLInputElement;
  const confirmBtn = utils
    .getByText("确认")
    .closest("button") as HTMLButtonElement;
  const cancelBtn = utils
    .getByText("取消")
    .closest("button") as HTMLButtonElement;
  return { ...utils, root, input, confirmBtn, cancelBtn, onSubmit, onCancel };
}

describe("InputDialog 渲染", () => {
  it("渲染标题与初始值", () => {
    const { getByText, input } = renderDialog();

    expect(getByText("重命名会话")).toBeTruthy();
    expect(input.value).toBe("旧标题");
  });

  it("挂载自动 focus + 全选", () => {
    const selectSpy = vi.spyOn(HTMLInputElement.prototype, "select");
    const { input } = renderDialog();

    // focus：当前活动元素为输入框
    expect(document.activeElement).toBe(input);
    // 全选：select 被调用
    expect(selectSpy).toHaveBeenCalled();
  });
});

describe("InputDialog 提交与取消", () => {
  it("Enter 提交 trim 后的值", () => {
    const { input, onSubmit } = renderDialog();

    fireEvent.change(input, { target: { value: "  新标题  " } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith("新标题");
  });

  it("空值 Enter 不提交（trim 后为空）", () => {
    const { input, onSubmit } = renderDialog();

    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("Escape → onCancel", () => {
    const { input, onCancel } = renderDialog();

    fireEvent.keyDown(input, { key: "Escape" });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("点击遮罩自身 → onCancel", () => {
    const { root, onCancel } = renderDialog();

    fireEvent.mouseDown(root);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("点击面板内部 → 不触发 onCancel", () => {
    const { root, onCancel } = renderDialog();

    // 面板是遮罩根元素的子节点，鼠标事件命中面板自身而非遮罩
    fireEvent.mouseDown(root.firstChild as HTMLElement);
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("取消按钮 → onCancel", () => {
    const { cancelBtn, onCancel } = renderDialog();

    fireEvent.click(cancelBtn);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("确认按钮 → onSubmit(trim 后值)", () => {
    const { input, confirmBtn, onSubmit } = renderDialog();

    fireEvent.change(input, { target: { value: "  新标题  " } });
    fireEvent.click(confirmBtn);

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith("新标题");
  });
});

describe("InputDialog 空值禁确认", () => {
  it("初始值为空 → 确认按钮 disabled", () => {
    const { confirmBtn } = renderDialog({ initialValue: "" });

    expect(confirmBtn.disabled).toBe(true);
  });

  it("输入非空 → 确认按钮启用", () => {
    const { input, confirmBtn } = renderDialog({ initialValue: "" });

    fireEvent.change(input, { target: { value: "新标题" } });
    expect(confirmBtn.disabled).toBe(false);
  });

  it("仅空白字符 → 确认按钮仍 disabled", () => {
    const { input, confirmBtn } = renderDialog({ initialValue: "" });

    fireEvent.change(input, { target: { value: "   " } });
    expect(confirmBtn.disabled).toBe(true);
  });
});
