// claude-history-action-dialog.test.tsx — SessionActionDialog L2 测试（问题 5 修复）
//
// 覆盖：标题/消息/动作按钮渲染、动作回调触发、取消（按钮/Esc/遮罩点击）、空 actions 防御。
// 纯受控展示组件，零 IPC mock。

import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";
import { SessionActionDialog } from "../features/claudeHistory/SessionActionDialog";

afterEach(cleanup);

/** 渲染弹窗，返回 dialog 元素与 mock 回调 */
function renderDialog(
  props: Partial<Parameters<typeof SessionActionDialog>[0]> = {},
) {
  const onCancel = vi.fn();
  const action1 = vi.fn();
  const action2 = vi.fn();
  const utils = render(
    <SessionActionDialog
      title="会话运行中"
      message="该会话已在运行中。"
      actions={[
        { label: "切换到该会话操作页面", action: action1 },
        { label: "分支恢复", action: action2 },
      ]}
      onCancel={onCancel}
      {...props}
    />,
  );
  const dialog = utils.container.querySelector(
    '[data-e2e="agent-history-action-dialog"]',
  ) as HTMLElement;
  return { ...utils, dialog, onCancel, action1, action2 };
}

describe("SessionActionDialog 渲染", () => {
  it("渲染标题 + 消息 + 动作按钮 + 取消按钮", () => {
    const { getByText, dialog } = renderDialog();

    expect(dialog).toBeTruthy();
    expect(getByText("会话运行中")).toBeTruthy();
    expect(getByText("该会话已在运行中。")).toBeTruthy();
    expect(getByText("切换到该会话操作页面")).toBeTruthy();
    expect(getByText("分支恢复")).toBeTruthy();
    expect(getByText("取消")).toBeTruthy();
  });

  it("message 省略 → 不渲染消息区", () => {
    const { queryByText } = renderDialog({ message: undefined });
    expect(queryByText("该会话已在运行中。")).toBeNull();
  });

  it("空 actions → 仅渲染标题与取消按钮（不抛错）", () => {
    const { getByText, queryByText } = renderDialog({ actions: [] });
    expect(getByText("取消")).toBeTruthy();
    expect(queryByText("切换到该会话操作页面")).toBeNull();
  });
});

describe("SessionActionDialog 交互", () => {
  it("点击动作按钮 → 对应 action 回调（弹窗不自动关闭——由调用方决定）", () => {
    const { getByText, action1, action2, dialog } = renderDialog();

    fireEvent.click(getByText("切换到该会话操作页面"));
    expect(action1).toHaveBeenCalledTimes(1);
    expect(action2).not.toHaveBeenCalled();
    expect(dialog).toBeTruthy(); // 未关闭

    fireEvent.click(getByText("分支恢复"));
    expect(action2).toHaveBeenCalledTimes(1);
  });

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
