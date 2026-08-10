// ipc-hooks-config-contract.test.ts — hooks 配置 IPC wrapper 合约测试（P3-FE-05，IHE-06 工厂化，MC-212 同步）
//
// 照 ipc-agent-hooks-contract.test.ts 模式，经共享工厂 describeIpcContract（helpers/ipc-contract.ts）
// 声明式驱动两命令 × 四维（命令名 / 参数含 cliId camelCase / 正常返回 / 异常传播）= 12 条用例：
// 1. 命令名正确（snake_case：agent_hooks_config_read / agent_hooks_config_write）
// 2. 参数结构正确（camelCase：cliId 首参 + layer / hooks / projectPath，Tauri 自动转 snake_case）
// 3. 返回透传（read 返回 hooks 子树或 null）
// 4. 异常传播
//
// ⚠️ mockIPC 盲区（IHE-01）：mockIPC 只守 JS 侧形状——camelCase/snake_case 真实转换、
// Channel 序列化、Uint8Array 处理、listen 回调运行时解包均不在 mock 层验证，
// 真实序列化由 L4 E2E 守卫（详见 src/ipc/CLAUDE.md「mockIPC 盲区声明」）。

import { afterEach, expect, vi } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { describeIpcContract } from "./helpers/ipc-contract";
import * as hooksConfig from "../ipc/hooksConfig";

afterEach(() => {
  clearMocks();
});

// 覆盖 setup.ts 全局 mock（若未来新增）——导入原始 ../ipc/hooksConfig 模块以测试真实 IPC 合约
vi.mock("../ipc/hooksConfig", async (importOriginal) => {
  return importOriginal<typeof import("../ipc/hooksConfig")>();
});

// 泛化命令 cliId 实参（合约测试固定用 claude——测试基建字面量合法）
const CLI_ID = "claude";

const mockSubtree = {
  PreToolUse: [
    {
      matcher: "Bash",
      hooks: [
        { type: "command", command: "echo hi", timeout: 5 },
      ],
    },
  ],
};

describeIpcContract("readHooksConfig 合约（agent_hooks_config_read）", [
  // 维度 1：命令名
  {
    name: "应调用 agent_hooks_config_read 命令",
    cmd: "agent_hooks_config_read",
    call: () => hooksConfig.readHooksConfig(CLI_ID, "user"),
    respond: null,
  },
  // 维度 2：参数结构——user 层无 projectPath
  {
    name: "user 层调用仅传 { layer }（无 projectPath）",
    cmd: "agent_hooks_config_read",
    call: () => hooksConfig.readHooksConfig(CLI_ID, "user"),
    respond: null,
    expectArgs: { cliId: CLI_ID, layer: "user" },
    expectExactKeys: ["cliId", "layer"],
  },
  // 维度 2：参数结构——project 层带 projectPath
  {
    name: "project 层调用传 { layer, projectPath }",
    cmd: "agent_hooks_config_read",
    call: () => hooksConfig.readHooksConfig(CLI_ID, "project", "D:/repo"),
    respond: null,
    expectArgs: { cliId: CLI_ID, layer: "project", projectPath: "D:/repo" },
    expectExactKeys: ["cliId", "layer", "projectPath"],
  },
  // 维度 3：正常返回透传——hooks 子树对象
  {
    name: "有 hooks 子树时透传原始 JSON 对象",
    cmd: "agent_hooks_config_read",
    call: () => hooksConfig.readHooksConfig(CLI_ID, "user"),
    respond: { ...mockSubtree },
    expectResult: mockSubtree,
  },
  // 维度 3：正常返回透传——null（文件不存在或无 hooks 键）
  {
    name: "文件不存在或无 hooks 键时透传 null",
    cmd: "agent_hooks_config_read",
    call: () => hooksConfig.readHooksConfig(CLI_ID, "user"),
    respond: null,
    expectResult: null,
  },
  // 维度 4：异常传播
  {
    name: "invoke 失败（JSON 损坏等）时异常应传播给调用方",
    cmd: "agent_hooks_config_read",
    call: () => hooksConfig.readHooksConfig(CLI_ID, "user"),
    mockThrow: "配置文件损坏，请先修复",
    expectReject: "配置文件损坏，请先修复",
  },
]);

describeIpcContract("writeHooksConfig 合约（agent_hooks_config_write）", [
  // 维度 1：命令名
  {
    name: "应调用 agent_hooks_config_write 命令",
    cmd: "agent_hooks_config_write",
    call: () => hooksConfig.writeHooksConfig(CLI_ID, "user", {}),
    respond: undefined,
  },
  // 维度 2：payload 键集合精确匹配——带 projectPath（local 层）
  {
    name: "local 层调用 payload 键集合精确为 { layer, hooks, projectPath }",
    cmd: "agent_hooks_config_write",
    call: () => hooksConfig.writeHooksConfig(CLI_ID, "local", mockSubtree, "D:/repo"),
    respond: undefined,
    expectArgs: { cliId: CLI_ID, layer: "local", hooks: mockSubtree, projectPath: "D:/repo" },
    expectExactKeys: ["cliId", "hooks", "layer", "projectPath"],
  },
  // 维度 2：payload 键集合精确匹配——无 projectPath（user 层）
  {
    name: "user 层调用 payload 键集合精确为 { layer, hooks }（无 projectPath）",
    cmd: "agent_hooks_config_write",
    call: () => hooksConfig.writeHooksConfig(CLI_ID, "user", { SessionStart: [] }),
    respond: undefined,
    expectArgs: { cliId: CLI_ID, layer: "user", hooks: { SessionStart: [] } },
    expectExactKeys: ["cliId", "hooks", "layer"],
  },
  // 维度 2：字段名是 hooks（非 content）
  {
    name: "payload 字段名应为 hooks（非 content）",
    cmd: "agent_hooks_config_write",
    call: () => hooksConfig.writeHooksConfig(CLI_ID, "user", {}),
    respond: undefined,
    assertArgs: (args) => {
      expect(args).toHaveProperty("hooks");
      expect(args).not.toHaveProperty("content");
    },
  },
  // 维度 3：正常返回 void
  {
    name: "正常返回 void",
    cmd: "agent_hooks_config_write",
    call: () => hooksConfig.writeHooksConfig(CLI_ID, "user", {}),
    respond: undefined,
    expectUndefined: true,
  },
  // 维度 4：异常传播
  {
    name: "invoke 失败（原文件损坏拒绝写入等）时异常应传播",
    cmd: "agent_hooks_config_write",
    call: () => hooksConfig.writeHooksConfig(CLI_ID, "user", {}),
    mockThrow: "原文件 JSON 损坏，拒绝写入",
    expectReject: "原文件 JSON 损坏，拒绝写入",
  },
]);
