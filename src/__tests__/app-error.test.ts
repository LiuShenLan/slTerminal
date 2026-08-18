// app-error.test.ts — AppError 统一解析器测试（FE-02）
//
// 覆盖：全 11 变体（含 BE-15 新增 ConfigParse）的 parseAppError 解析 +
//       非 AppError 输入兜底 + getErrorMessage 提取/兜底。
// 后端 AppError 序列化形态见 src-tauri/src/error.rs（#[serde(rename_all = "camelCase")]）。

import { describe, it, expect } from "vitest";
import {
  parseAppError,
  getErrorMessage,
  APP_ERROR_VARIANTS,
} from "../ipc/appError";

describe("parseAppError — 全 11 变体解析", () => {
  it.each([
    // [变体名, 后端序列化形态, 期望消息]
    [
      "ioKind",
      { ioKind: { kind: "NotFound", message: "文件不存在" } },
      "文件不存在",
    ],
    ["pty", { pty: "PTY 进程崩溃" }, "PTY 进程崩溃"],
    ["git", { git: "rebase 冲突" }, "rebase 冲突"],
    ["serde", { serde: "JSON 键缺失" }, "JSON 键缺失"],
    ["unknown", { unknown: "未分类错误" }, "未分类错误"],
    ["sessionNotFound", { sessionNotFound: "uuid-12345" }, "uuid-12345"],
    ["taskJoin", { taskJoin: "join error" }, "join error"],
    ["notify", { notify: "watcher 启动失败" }, "watcher 启动失败"],
    ["validation", { validation: "非法 layer" }, "非法 layer"],
    [
      "pathNotAllowed",
      { pathNotAllowed: "C:\\outside\\project" },
      "C:\\outside\\project",
    ],
    // BE-15 新增变体：配置 JSON 损坏场景
    ["configParse", { configParse: "settings.json 损坏" }, "settings.json 损坏"],
  ] as const)("解析 %s 变体", (variant, err, expectedMessage) => {
    const parsed = parseAppError(err);
    expect(parsed).toEqual({ variant, message: expectedMessage });
  });

  it("APP_ERROR_VARIANTS 常量恰为 11 项（与后端变体数同步）", () => {
    expect(APP_ERROR_VARIANTS).toHaveLength(11);
    expect(APP_ERROR_VARIANTS).toContain("configParse");
  });

  it("IoKind 对象形态提取 message 字段而非整个对象", () => {
    const parsed = parseAppError({
      ioKind: { kind: "PermissionDenied", message: "访问被拒绝" },
    });
    expect(parsed).toEqual({ variant: "ioKind", message: "访问被拒绝" });
  });
});

describe("parseAppError — 非 AppError 输入兜底", () => {
  it.each([
    ["null", null],
    ["undefined", undefined],
    ["字符串", "some error"],
    ["数字", 42],
    ["数组", ["pty", "error"]],
    ["Error 实例", new Error("boom")],
    ["无变体键的对象", { foo: "bar" }],
    ["空对象", {}],
  ])("%s → null", (_label, err) => {
    expect(parseAppError(err)).toBeNull();
  });
});

describe("getErrorMessage", () => {
  it("命中 AppError 形态返回变体消息", () => {
    expect(getErrorMessage({ pty: "PTY 进程崩溃" })).toBe("PTY 进程崩溃");
    expect(
      getErrorMessage({ ioKind: { kind: "NotFound", message: "文件不存在" } }),
    ).toBe("文件不存在");
    expect(getErrorMessage({ configParse: "settings.json 损坏" })).toBe(
      "settings.json 损坏",
    );
  });

  it("字符串输入兜底 String(err) 原样返回", () => {
    expect(getErrorMessage("自定义错误")).toBe("自定义错误");
  });

  it("Error 实例兜底 String(err)", () => {
    expect(getErrorMessage(new Error("boom"))).toBe("Error: boom");
  });

  it("null / undefined 兜底为字面量字符串", () => {
    expect(getErrorMessage(null)).toBe("null");
    expect(getErrorMessage(undefined)).toBe("undefined");
  });

  it("非 AppError 对象兜底为 String(err)", () => {
    const obj = { foo: "bar" };
    expect(getErrorMessage(obj)).toBe(String(obj));
  });
});
