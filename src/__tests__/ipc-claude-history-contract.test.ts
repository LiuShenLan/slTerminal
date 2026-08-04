// ipc-claude-history-contract.test.ts — claude 历史会话 IPC wrapper 合约测试（FE-03，IHE-06 工厂化）
//
// 照 ipc-hooks-config-contract.test.ts 模式，经共享工厂 describeIpcContract（helpers/ipc-contract.ts）
// 声明式驱动两命令 × 四维（命令名 / 参数结构 / 正常返回 / 异常传播）= 8 条用例
// （重命名命令已随功能整体移除——问题 7 修复）：
// 1. 命令名 snake_case 逐字（claude_history_scan 等，非驼峰）
// 2. 参数结构键集合精确匹配（{ sessionId }，防字段漂移）
// 3. 返回透传（scan 返回 HistorySession[] 全形态样例）
// 4. 异常传播不吞异常
//
// ⚠️ mockIPC 盲区（IHE-01）：mockIPC 只守 JS 侧形状——camelCase/snake_case 真实转换、
// Channel 序列化、Uint8Array 处理、listen 回调运行时解包均不在 mock 层验证，
// 真实序列化由 L4 E2E 守卫（详见 src/ipc/CLAUDE.md「mockIPC 盲区声明」）。

import { afterEach, vi } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { describeIpcContract } from "./helpers/ipc-contract";
import * as claudeHistory from "../ipc/claudeHistory";

// 导入原始 ../ipc/claudeHistory 模块以测试真实 IPC 合约（照 hooksConfig 模式覆盖潜在全局 mock）
vi.mock("../ipc/claudeHistory", async (importOriginal) => {
  return importOriginal<typeof import("../ipc/claudeHistory")>();
});

afterEach(() => {
  clearMocks();
});

const SESSION_ID = "123e4567-e89b-12d3-a456-426614174000";

const mockSessions = [
  {
    sessionId: SESSION_ID,
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

describeIpcContract("scanHistory 合约（claude_history_scan）", [
  // 维度 1：命令名（snake_case 逐字）
  {
    name: "应调用 claude_history_scan 命令（非驼峰）",
    cmd: "claude_history_scan",
    call: () => claudeHistory.scanHistory(),
    respond: [],
  },
  // 维度 2：参数结构——scan 无参数（invoke payload 为空对象）
  {
    name: "调用无参数（invoke payload 为空对象）",
    cmd: "claude_history_scan",
    call: () => claudeHistory.scanHistory(),
    respond: [],
    expectArgs: {},
  },
  // 维度 3：正常返回透传——HistorySession[] 七字段全形态样例
  {
    name: "透传 HistorySession[]（七字段全形态）",
    cmd: "claude_history_scan",
    call: () => claudeHistory.scanHistory(),
    respond: [...mockSessions],
    expectResult: mockSessions,
  },
  // 维度 4：异常传播
  {
    name: "invoke 失败时异常应传播给调用方",
    cmd: "claude_history_scan",
    call: () => claudeHistory.scanHistory(),
    mockThrow: "扫描失败",
    expectReject: "扫描失败",
  },
]);

describeIpcContract("deleteHistorySession 合约（claude_history_delete）", [
  // 维度 1：命令名（snake_case 逐字）
  {
    name: "应调用 claude_history_delete 命令（非驼峰）",
    cmd: "claude_history_delete",
    call: () => claudeHistory.deleteHistorySession(SESSION_ID),
    respond: undefined,
  },
  // 维度 2：参数结构——键集合精确匹配 { sessionId }（防字段漂移）
  {
    name: "payload 键集合精确为 { sessionId }",
    cmd: "claude_history_delete",
    call: () => claudeHistory.deleteHistorySession(SESSION_ID),
    respond: undefined,
    expectArgs: { sessionId: SESSION_ID },
    expectExactKeys: ["sessionId"],
  },
  // 维度 3：正常返回 void
  {
    name: "正常返回 void",
    cmd: "claude_history_delete",
    call: () => claudeHistory.deleteHistorySession(SESSION_ID),
    respond: undefined,
    expectUndefined: true,
  },
  // 维度 4：异常传播
  {
    name: "invoke 失败（会话不存在等）时异常应传播",
    cmd: "claude_history_delete",
    call: () => claudeHistory.deleteHistorySession(SESSION_ID),
    mockThrow: "会话不存在",
    expectReject: "会话不存在",
  },
]);

// （renameHistorySession 已随重命名功能整体移除——问题 7 修复，
//   前端 wrapper、后端命令、相关测试全链路删除）
