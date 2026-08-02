// ipc-claude-history-contract.test.ts — claude 历史会话 IPC wrapper 合约测试（FE-03）
//
// 使用 mockIPC 拦截真实的 invoke 调用，照 ipc-hooks-config-contract.test.ts 模式验证
// 两命令 × 四维（命令名 / 参数结构 / 正常返回 / 异常传播）= 8 条用例
// （重命名命令已随功能整体移除——问题 7 修复）：
// 1. 命令名 snake_case 逐字（claude_history_scan 等，非驼峰）
// 2. 参数结构键集合精确匹配（{ sessionId }，防字段漂移）
// 3. 返回透传（scan 返回 HistorySession[] 全形态样例）
// 4. 异常传播不吞异常

import { describe, it, expect, afterEach, vi } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";

// 导入原始 ../ipc/claudeHistory 模块以测试真实 IPC 合约（照 hooksConfig 模式覆盖潜在全局 mock）
vi.mock("../ipc/claudeHistory", async (importOriginal) => {
  return importOriginal<typeof import("../ipc/claudeHistory")>();
});

import * as claudeHistory from "../ipc/claudeHistory";

afterEach(() => {
  clearMocks();
});

// ═══════════════════════════════════════════════════════════════════
// scanHistory — claude_history_scan
// ═══════════════════════════════════════════════════════════════════

describe("scanHistory 合约", () => {
  // 维度 1：命令名（snake_case 逐字）
  it("应调用 claude_history_scan 命令（非驼峰）", async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === "claude_history_scan") return [];
    });

    await claudeHistory.scanHistory();

    expect(spy).toHaveBeenCalledTimes(1);
    const [cmd] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(cmd).toBe("claude_history_scan");
  });

  // 维度 2：参数结构——scan 无参数（invoke payload 为空对象）
  it("调用无参数（invoke payload 为空对象）", async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === "claude_history_scan") return [];
    });

    await claudeHistory.scanHistory();

    const [, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(args).toEqual({});
  });

  // 维度 3：正常返回透传——HistorySession[] 七字段全形态样例
  it("透传 HistorySession[]（七字段全形态）", async () => {
    const mockSessions = [
      {
        sessionId: "123e4567-e89b-12d3-a456-426614174000",
        cwd: "D:\\proj\\alpha",
        title: "修复登录 bug",
        titleSource: "customTitle",
        firstPrompt: "帮我看看登录流程",
        mtimeMs: 1_752_500_000_000,
        cwdExists: true,
      },
      {
        sessionId: "223e4567-e89b-12d3-a456-426614174000",
        cwd: null,
        title: null,
        titleSource: "none",
        firstPrompt: null,
        mtimeMs: 0,
        cwdExists: false,
      },
      {
        sessionId: "323e4567-e89b-12d3-a456-426614174000",
        cwd: "D:\\gone",
        title: "AI 自动标题",
        titleSource: "aiTitle",
        firstPrompt: "短 prompt",
        mtimeMs: 1_752_000_000_000,
        cwdExists: false,
      },
    ];
    mockIPC((cmd) => {
      if (cmd === "claude_history_scan") return [...mockSessions];
    });

    const result = await claudeHistory.scanHistory();

    expect(result).toEqual(mockSessions);
    expect(result).toHaveLength(3);
  });

  // 维度 4：异常传播
  it("invoke 失败时异常应传播给调用方", async () => {
    mockIPC((cmd) => {
      if (cmd === "claude_history_scan") throw new Error("扫描失败");
    });

    await expect(claudeHistory.scanHistory()).rejects.toThrow("扫描失败");
  });
});

// ═══════════════════════════════════════════════════════════════════
// deleteHistorySession — claude_history_delete
// ═══════════════════════════════════════════════════════════════════

describe("deleteHistorySession 合约", () => {
  // 维度 1：命令名（snake_case 逐字）
  it("应调用 claude_history_delete 命令（非驼峰）", async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === "claude_history_delete") return;
    });

    await claudeHistory.deleteHistorySession("123e4567-e89b-12d3-a456-426614174000");

    expect(spy).toHaveBeenCalledTimes(1);
    const [cmd] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(cmd).toBe("claude_history_delete");
  });

  // 维度 2：参数结构——键集合精确匹配 { sessionId }（防字段漂移）
  it("payload 键集合精确为 { sessionId }", async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === "claude_history_delete") return;
    });

    await claudeHistory.deleteHistorySession("123e4567-e89b-12d3-a456-426614174000");

    const [, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(Object.keys(args as Record<string, unknown>)).toEqual(["sessionId"]);
    expect(args.sessionId).toBe("123e4567-e89b-12d3-a456-426614174000");
  });

  // 维度 3：正常返回 void
  it("正常返回 void", async () => {
    mockIPC((cmd) => {
      if (cmd === "claude_history_delete") return;
    });

    const result = await claudeHistory.deleteHistorySession(
      "123e4567-e89b-12d3-a456-426614174000",
    );
    expect(result).toBeUndefined();
  });

  // 维度 4：异常传播
  it("invoke 失败（会话不存在等）时异常应传播", async () => {
    mockIPC((cmd) => {
      if (cmd === "claude_history_delete")
        throw new Error("会话不存在");
    });

    await expect(
      claudeHistory.deleteHistorySession("123e4567-e89b-12d3-a456-426614174000"),
    ).rejects.toThrow("会话不存在");
  });
});

// ═══════════════════════════════════════════════════════════════════
// （renameHistorySession 已随重命名功能整体移除——问题 7 修复，
//   前端 wrapper、后端命令、相关测试全链路删除）
// ═══════════════════════════════════════════════════════════════════
