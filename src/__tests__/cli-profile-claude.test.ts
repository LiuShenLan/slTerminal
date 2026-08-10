// cli-profile-claude.test.ts — claude profile 身份域 L2 测试
//
// 语义来源（MC-104 迁移）：原 tab-rules.test.ts（6 用例：side-effect 注册/手动
// 注册/_reset 恢复语义）。
// 覆盖：claude 身份域字段断言（MC-104）+ CLAUDE_CLI_ID 常量一致性 + side-effect
// 注册。
// logo 资源守卫（MC-108）已移至 cli-profile-registry.test.ts（遍历注册表全部
// profile 断言 iconSrc 磁盘存在 + PNG 魔数——img 404 无报错通道，资源缺失靠此
// 守卫；含 mockcli.png 先行资源，决策 5，Stage 07 mock 夹具引用）。

import { describe, it, expect, afterEach } from "vitest";
// 导入触发 side-effect 注册（照 tab-rules.test.ts 先例，模块加载即注册 claude）
import {
  CLAUDE_CLI_ID,
  claudeProfile,
} from "../features/cliProfiles/profiles/claude";
import { cliProfileRegistry } from "../features/cliProfiles/cliProfileRegistry";

/** 每用例后恢复 claude 注册（全局单例隔离，照 cli-icons.test.ts 模式） */
afterEach(() => {
  cliProfileRegistry._reset();
  cliProfileRegistry.register(claudeProfile);
});

describe("claude profile 身份域（MC-104）", () => {
  it("side-effect 注册生效：import profiles/claude 后 matchByCommand('claude') 命中", () => {
    // tabRules 先例——模块顶层 import 已执行注册，此处直接 match 验证副作用
    const profile = cliProfileRegistry.matchByCommand("claude");
    expect(profile).not.toBeNull();
    expect(profile!.id).toBe(CLAUDE_CLI_ID);
  });

  it('CLAUDE_CLI_ID 常量 = "claude"（缺省回退常量定义）', () => {
    expect(CLAUDE_CLI_ID).toBe("claude");
  });

  it("CLAUDE_CLI_ID 与注册 profile.id 一致（防常量与注册漂移）", () => {
    const profile = cliProfileRegistry.get(CLAUDE_CLI_ID);
    expect(profile).not.toBeUndefined();
    expect(profile!.id).toBe(CLAUDE_CLI_ID);
  });

  it("身份域字段完整：id/displayName/commands/iconSrc/tabTitle/capabilities", () => {
    const profile = cliProfileRegistry.get(CLAUDE_CLI_ID)!;
    expect(profile).toEqual({
      id: "claude",
      displayName: "claude",
      commands: ["claude"],
      iconSrc: "/cli-icons/claude.png",
      tabTitle: "claude",
      capabilities: {},
    });
  });

  it("capabilities 本 Stage 为空（hooks 能力 Stage 02 迁入、history 能力 Stage 05 迁入）", () => {
    const profile = cliProfileRegistry.get(CLAUDE_CLI_ID)!;
    expect(profile.capabilities.hooks).toBeUndefined();
    expect(profile.capabilities.history).toBeUndefined();
  });

  it("matchByCommand 带参变体命中 claude（claude --resume xxx）", () => {
    const profile = cliProfileRegistry.matchByCommand("claude --resume abc123");
    expect(profile).not.toBeNull();
    expect(profile!.tabTitle).toBe(CLAUDE_CLI_ID);
  });

  it("_reset 清空后手动重新注册恢复", () => {
    cliProfileRegistry._reset();
    expect(cliProfileRegistry.matchByCommand("claude")).toBeNull();
    cliProfileRegistry.register(claudeProfile);
    expect(cliProfileRegistry.matchByCommand("claude")).not.toBeNull();
  });
});
