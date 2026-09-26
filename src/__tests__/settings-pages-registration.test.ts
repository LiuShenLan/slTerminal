// settings-pages-registration.test.ts —— 真实注册触发点验证（F11，TE-04；ADR-0023）
//
// 被测：src/features/settingsCenter/pages.ts 的 side-effect 注册（硬约束 #13：
// 注册经 side-effect import 触发，禁止隐式初始化）。本文件不 mock pages 自身，
// 仅把页面组件模块 mock 为 () => null（组件实现不属本测试面），
// import 真实 pages.ts 触发注册后，断言注册表精确包含：
// - global 组三页（id/group/order 逐一核对）
// - agent 组逐 profile 枚举页（agent.<cliId>.basic 恒有 + agent.<cliId>.hooks
//   经 hasConfigEditor 门控；claude profile 真实注册，两页均应有）
// afterEach 调 _reset() 保证用例隔离（注册表家族契约）。

import { describe, it, expect, afterEach, vi } from "vitest";

// 页面组件实现不属本测试面——整模块 mock 为空组件桩，避免拉入真实组件依赖链
vi.mock("../panels/settings/pages/KeybindingsPage", () => ({ default: () => null }));
vi.mock("../panels/settings/pages/BackgroundTasksPage", () => ({ default: () => null }));
vi.mock("../panels/settings/pages/ConptyInputModesPage", () => ({ default: () => null }));
vi.mock("../panels/settings/pages/AgentBasicPage", () => ({ default: () => null }));
vi.mock("../panels/settings/pages/AgentHooksPage", () => ({ default: () => null }));

import { getSettingsPageRegistry } from "../features/settingsCenter/SettingsPageRegistry";
import { cliProfileRegistry } from "../features/cliProfiles/cliProfileRegistry";
import type { CodingCliProfile } from "../features/cliProfiles/types";
// side-effect import：import 即完成全部配置页注册（pages.ts 注册触发点；
// 内部显式 import profiles——claude profile 真实注册，agent 页枚举数据源）
import { syncAgentPagesFromProfiles } from "../features/settingsCenter/pages";

/** light fake profile（iconSrc 不被 jsdom 解析；hooks 能力按参开关） */
function fakeProfile(id: string, withHooksEditor: boolean): CodingCliProfile {
  return {
    id,
    displayName: id,
    commands: [id],
    iconSrc: `/cli-icons/${id}.png`,
    tabTitle: id,
    capabilities: withHooksEditor
      ? {
          hooks: {
            eventToStatus: () => null,
            classifyNotification: () => null,
            computeUsagePercent: () => null,
            restartHint: `${id} 提示`,
            hasConfigEditor: true,
            configEditor: () => null,
          },
        }
      : {},
  };
}

describe("配置页真实注册（pages.ts side-effect import）", () => {
  afterEach(() => {
    // 硬约束 #13 注册表契约：_reset() 清空全部条目，保证用例隔离
    getSettingsPageRegistry()._reset();
    cliProfileRegistry._reset();
  });

  it("pages.ts import 后注册表 = global 三页 + agent 组 claude 两页（id/group/cliId/order/标题逐一核对）", () => {
    const pages = getSettingsPageRegistry().getAll().map((p) => ({
      id: p.id,
      group: p.group,
      cliId: p.cliId,
      order: p.order,
    }));

    expect(pages).toEqual([
      { id: "keybindings", group: "global", cliId: undefined, order: 10 },
      { id: "backgroundTasks", group: "global", cliId: undefined, order: 20 },
      { id: "conptyInputModes", group: "global", cliId: undefined, order: 25 },
      // agent 组：基础配置恒有；hooks 页经 hasConfigEditor 门控（claude = true）
      { id: "agent.claude.basic", group: "agent", cliId: "claude", order: undefined },
      { id: "agent.claude.hooks", group: "agent", cliId: "claude", order: undefined },
    ]);

    // agent 页标题：基础配置 / Hooks 配置（同一快照断言——afterEach _reset 清空后
    // side-effect import 不会重跑，禁止拆为独立用例）
    const reg = getSettingsPageRegistry();
    expect(reg.get("agent.claude.basic")?.title).toBe("基础配置");
    expect(reg.get("agent.claude.hooks")?.title).toBe("Hooks 配置");
  });

  it("syncAgentPagesFromProfiles：动态注册 profile 幂等补注册（hooks 门控 + 重复调用不翻倍）", () => {
    // 注：本用例在 afterEach _reset 后的空注册表上跑——sync 是幂等重建（register
    // 同 id 覆盖），不依赖首用例的 side-effect 注册残留
    cliProfileRegistry.register(fakeProfile("latecli", true));
    cliProfileRegistry.register(fakeProfile("plaincli", false));

    syncAgentPagesFromProfiles();
    const reg = getSettingsPageRegistry();
    // 动态注册 profile 补出 agent 页：hooks 门控命中（latecli 两页）/未命中（plaincli 仅 basic）
    expect(reg.get("agent.latecli.basic")).toBeDefined();
    expect(reg.get("agent.latecli.hooks")).toBeDefined();
    expect(reg.get("agent.plaincli.basic")).toBeDefined();
    expect(reg.get("agent.plaincli.hooks")).toBeUndefined();

    // 幂等：再调一次页数不变（register 同 id 覆盖，不重复累积）
    const countBefore = reg.getAll().length;
    syncAgentPagesFromProfiles();
    expect(reg.getAll().length).toBe(countBefore);
  });
});
