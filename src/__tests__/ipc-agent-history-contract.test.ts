// ipc-agent-history-contract.test.ts — agent 历史会话 IPC wrapper 合约测试（FE-03，IHE-06 工厂化）
//
// 照 ipc-hooks-config-contract.test.ts 模式，经共享工厂 describeIpcContract（helpers/ipc-contract.ts）
// 声明式驱动三命令 × 四维（命令名 / 参数结构 / 正常返回 / 异常传播）= 18 条用例
// （重命名命令已随功能整体移除——问题 7 修复；readHistoryTitle = 人工验证问题 3 新增；
// scanAgentHistory(cliId, force) = BE-19 新增；scanHistory 无参聚合导出已删除——
// 后端 agent_history_scan 的 cli_id 必填（S12 起），无参 invoke 恒 reject，历史区
// 「暂无历史会话」根因，契约断链修复）：
// 1. 命令名 snake_case 逐字（agent_history_scan 等，非驼峰）
// 2. 参数结构键集合精确匹配（scanAgentHistory { cliId, force? } / delete { cliId, sessionId } /
//    readTitle { cliId, sessionId }，防字段漂移）
// 3. 返回透传（scan 返回 AgentHistorySession[] 八键全形态样例；readTitle 返回两键）
// 4. 异常传播不吞异常
//
// ⚠️ mockIPC 盲区（IHE-01）：mockIPC 只守 JS 侧形状——camelCase/snake_case 真实转换、
// Channel 序列化、Uint8Array 处理、listen 回调运行时解包均不在 mock 层验证，
// 真实序列化由 L4 E2E 守卫（详见 src/ipc/CLAUDE.md「mockIPC 盲区声明」）。

import { afterEach, vi } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { describeIpcContract } from "./helpers/ipc-contract";
import * as agentHistory from "../ipc/agentHistory";

// 导入原始 ../ipc/agentHistory 模块以测试真实 IPC 合约（照 hooksConfig 模式覆盖潜在全局 mock）
vi.mock("../ipc/agentHistory", async (importOriginal) => {
  return importOriginal<typeof import("../ipc/agentHistory")>();
});

afterEach(() => {
  clearMocks();
});

const SESSION_ID = "123e4567-e89b-12d3-a456-426614174000";
const CLI_ID = "claude";

const mockSessions = [
  {
    sessionId: SESSION_ID,
    cwd: "D:\\proj\\alpha",
    title: "修复登录 bug",
    titleSource: "customTitle",
    firstPrompt: "帮我看看登录流程",
    mtimeMs: 1_752_500_000_000,
    cwdExists: true,
    cliId: CLI_ID,
  },
  {
    sessionId: "223e4567-e89b-12d3-a456-426614174000",
    cwd: null,
    title: null,
    titleSource: "none",
    firstPrompt: null,
    mtimeMs: 0,
    cwdExists: false,
    cliId: CLI_ID,
  },
  {
    sessionId: "323e4567-e89b-12d3-a456-426614174000",
    cwd: "D:\\gone",
    title: "AI 自动标题",
    titleSource: "aiTitle",
    firstPrompt: "短 prompt",
    mtimeMs: 1_752_000_000_000,
    cwdExists: false,
    cliId: CLI_ID,
  },
];

describeIpcContract("scanAgentHistory 合约（agent_history_scan 带 cliId + force，BE-19）", [
  // 维度 1：命令名（snake_case 逐字）
  {
    name: "应调用 agent_history_scan 命令（非驼峰）",
    cmd: "agent_history_scan",
    call: () => agentHistory.scanAgentHistory(CLI_ID),
    respond: [],
  },
  // 维度 2：参数结构——force 缺省 undefined（toEqual 忽略 undefined 键；
  // 真实序列化 JSON.stringify 丢弃该键，后端 Option 缺省 = 缓存命中路径）
  {
    name: "payload = { cliId }（force 缺省 undefined）",
    cmd: "agent_history_scan",
    call: () => agentHistory.scanAgentHistory(CLI_ID),
    respond: [],
    expectArgs: { cliId: CLI_ID },
  },
  {
    name: "force=true 透传（显式刷新/恢复完成——绕过缓存强制重扫）",
    cmd: "agent_history_scan",
    call: () => agentHistory.scanAgentHistory(CLI_ID, true),
    respond: [],
    expectArgs: { cliId: CLI_ID, force: true },
    expectExactKeys: ["cliId", "force"],
  },
  {
    name: "force=false 透传（显式声明走缓存）",
    cmd: "agent_history_scan",
    call: () => agentHistory.scanAgentHistory(CLI_ID, false),
    respond: [],
    expectArgs: { cliId: CLI_ID, force: false },
    expectExactKeys: ["cliId", "force"],
  },
  // 维度 3：正常返回透传——AgentHistorySession[] 八键全形态样例
  {
    name: "透传 AgentHistorySession[]（八键全形态含 cliId）",
    cmd: "agent_history_scan",
    call: () => agentHistory.scanAgentHistory(CLI_ID),
    respond: [...mockSessions],
    expectResult: mockSessions,
  },
  // 维度 4：异常传播
  {
    name: "invoke 失败时异常应传播给调用方",
    cmd: "agent_history_scan",
    call: () => agentHistory.scanAgentHistory(CLI_ID),
    mockThrow: "扫描失败",
    expectReject: "扫描失败",
  },
]);

describeIpcContract("deleteHistorySession 合约（agent_history_delete）", [
  // 维度 1：命令名（snake_case 逐字）
  {
    name: "应调用 agent_history_delete 命令（非驼峰）",
    cmd: "agent_history_delete",
    call: () => agentHistory.deleteHistorySession(CLI_ID, SESSION_ID),
    respond: undefined,
  },
  // 维度 2：参数结构——键集合精确匹配 { cliId, sessionId }（防字段漂移）
  {
    name: "payload 键集合精确为 { cliId, sessionId }",
    cmd: "agent_history_delete",
    call: () => agentHistory.deleteHistorySession(CLI_ID, SESSION_ID),
    respond: undefined,
    expectArgs: { cliId: CLI_ID, sessionId: SESSION_ID },
    expectExactKeys: ["cliId", "sessionId"],
  },
  // 维度 3：正常返回 void
  {
    name: "正常返回 void",
    cmd: "agent_history_delete",
    call: () => agentHistory.deleteHistorySession(CLI_ID, SESSION_ID),
    respond: undefined,
    expectUndefined: true,
  },
  // 维度 4：异常传播
  {
    name: "invoke 失败（未知 cliId/会话不存在等）时异常应传播",
    cmd: "agent_history_delete",
    call: () => agentHistory.deleteHistorySession(CLI_ID, SESSION_ID),
    mockThrow: "会话不存在",
    expectReject: "会话不存在",
  },
  // 维度 5：SEC-05 非法 sessionId 负例（后端 validate_session_id 前置拒绝；
  // mockThrow/expectReject 透传后端消息形态「非法 sessionId: <id>」——
  // 与 src-tauri/src/agent_history/claude/ops.rs 一致；payload 键集合保持精确）
  {
    name: "SEC-05：空 sessionId 后端拒绝 → 异常透传",
    cmd: "agent_history_delete",
    call: () => agentHistory.deleteHistorySession(CLI_ID, ""),
    mockThrow: "非法 sessionId: ",
    expectReject: "非法 sessionId",
    expectExactKeys: ["cliId", "sessionId"],
  },
  {
    name: "SEC-05：路径穿越 sessionId 后端拒绝 → 异常透传",
    cmd: "agent_history_delete",
    call: () => agentHistory.deleteHistorySession(CLI_ID, "../etc/passwd"),
    mockThrow: "非法 sessionId: ../etc/passwd",
    expectReject: "非法 sessionId",
    expectExactKeys: ["cliId", "sessionId"],
  },
  {
    name: "SEC-05：非 UUID sessionId 后端拒绝 → 异常透传",
    cmd: "agent_history_delete",
    call: () => agentHistory.deleteHistorySession(CLI_ID, "not-a-uuid"),
    mockThrow: "非法 sessionId: not-a-uuid",
    expectReject: "非法 sessionId",
    expectExactKeys: ["cliId", "sessionId"],
  },
]);

describeIpcContract("readHistoryTitle 合约（agent_history_read_title，人工验证问题 3）", [
  // 维度 1：命令名（snake_case 逐字）
  {
    name: "应调用 agent_history_read_title 命令（非驼峰）",
    cmd: "agent_history_read_title",
    call: () => agentHistory.readHistoryTitle(CLI_ID, SESSION_ID),
    respond: { title: null, titleSource: "none" },
  },
  // 维度 2：参数结构——键集合精确匹配 { cliId, sessionId }（防字段漂移）
  {
    name: "payload 键集合精确为 { cliId, sessionId }",
    cmd: "agent_history_read_title",
    call: () => agentHistory.readHistoryTitle(CLI_ID, SESSION_ID),
    respond: { title: null, titleSource: "none" },
    expectArgs: { cliId: CLI_ID, sessionId: SESSION_ID },
    expectExactKeys: ["cliId", "sessionId"],
  },
  // 维度 3：正常返回透传——AgentHistoryTitle 两键（标题 + 来源；null 标题形态）
  {
    name: "透传 AgentHistoryTitle（两键：title/titleSource）",
    cmd: "agent_history_read_title",
    call: () => agentHistory.readHistoryTitle(CLI_ID, SESSION_ID),
    respond: { title: "修复登录 bug", titleSource: "customTitle" },
    expectResult: { title: "修复登录 bug", titleSource: "customTitle" },
  },
  // 维度 4：异常传播（未知 cliId / 非法 sessionId → 调用方 catch 兜底）
  {
    name: "invoke 失败时异常应传播给调用方",
    cmd: "agent_history_read_title",
    call: () => agentHistory.readHistoryTitle(CLI_ID, SESSION_ID),
    mockThrow: "未知 cliId",
    expectReject: "未知 cliId",
  },
  // 维度 5：SEC-05 非法 sessionId 负例（read_title 同受 validate_session_id 强制前置）
  {
    name: "SEC-05：非 UUID sessionId 后端拒绝 → 异常透传",
    cmd: "agent_history_read_title",
    call: () => agentHistory.readHistoryTitle(CLI_ID, "not-a-uuid"),
    mockThrow: "非法 sessionId: not-a-uuid",
    expectReject: "非法 sessionId",
    expectExactKeys: ["cliId", "sessionId"],
  },
]);

// （renameHistorySession 已随重命名功能整体移除——问题 7 修复，
//   前端 wrapper、后端命令、相关测试全链路删除）
