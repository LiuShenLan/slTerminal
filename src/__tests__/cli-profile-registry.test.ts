// cli-profile-registry.test.ts — CliProfileRegistry（CLI profile 注册表）L2 测试
//
// 语义来源（MC-108 迁移）：原 tab-title-registry.test.ts（13 用例：register/match/
// 覆盖/_reset/单例）+ cli-icons.test.ts（12 用例：match 首 token/getSrc/覆盖/独立
// 实例）的注册表行为语义并入本文件——MC-102 首 token 解析单点化后，唯一实现即
// 注册表内部 matchByCommand。
// 覆盖：register/get/getAll 注册序/同 id 覆盖（注册序不变）/matchByCommand（多
// commands、带参变体、空命令行、仅空白、未命中、不 toLowerCase、同键冲突先注册者
// 优先）/ _reset / 独立实例 / 全局单例 + logo 资源守卫（MC-108 泛化：遍历注册表
// 全部 profile 断言 iconSrc 磁盘存在 + PNG 魔数——img 404 无报错通道，资源缺失靠
// 此守卫；含 mockcli.png 先行资源，决策 5，Stage 07 mock 夹具引用）。

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import {
  CliProfileRegistry,
  cliProfileRegistry,
} from "../features/cliProfiles/cliProfileRegistry";
import type { CodingCliProfile } from "../features/cliProfiles/types";

const here = dirname(fileURLToPath(import.meta.url));

/** 构造测试 profile（commands 缺省为 [id]） */
function makeProfile(id: string, commands: string[] = [id]): CodingCliProfile {
  return {
    id,
    displayName: id,
    commands,
    iconSrc: `/cli-icons/${id}.png`,
    tabTitle: id,
    capabilities: {},
  };
}

/** 每用例后清空全局单例（测试 profile 隔离，照 cli-icons.test.ts 模式） */
afterEach(() => {
  cliProfileRegistry._reset();
});

describe("CliProfileRegistry", () => {
  let registry: CliProfileRegistry;

  // 每个 test case 使用全新实例（行为层隔离，照 tab-title-registry.test.ts 模式）
  beforeEach(() => {
    registry = new CliProfileRegistry();
  });

  describe("注册与查询", () => {
    it("register 后 get 返回对应 profile", () => {
      registry.register(makeProfile("claude"));
      const profile = registry.get("claude");
      expect(profile).not.toBeUndefined();
      expect(profile!.id).toBe("claude");
      expect(profile!.displayName).toBe("claude");
      expect(profile!.tabTitle).toBe("claude");
    });

    it("get 未注册 id → undefined", () => {
      registry.register(makeProfile("claude"));
      expect(registry.get("codex")).toBeUndefined();
    });

    it("register 同 id 覆盖旧 profile（取最后一条）", () => {
      registry.register({ ...makeProfile("claude"), tabTitle: "claude-v1" });
      registry.register({ ...makeProfile("claude"), tabTitle: "claude-v2" });
      expect(registry.get("claude")!.tabTitle).toBe("claude-v2");
    });

    it("getAll 按注册序返回全部 profile", () => {
      registry.register(makeProfile("claude"));
      registry.register(makeProfile("codex"));
      registry.register(makeProfile("gemini"));
      expect(registry.getAll().map((p) => p.id)).toEqual([
        "claude",
        "codex",
        "gemini",
      ]);
    });

    it("getAll 同 id 覆盖后注册序不变（仅值更新）", () => {
      registry.register(makeProfile("claude"));
      registry.register(makeProfile("codex"));
      registry.register(makeProfile("claude")); // 覆盖 claude 条目
      expect(registry.getAll().map((p) => p.id)).toEqual(["claude", "codex"]);
      expect(registry.getAll()[0]!.tabTitle).toBe("claude");
    });
  });

  describe("matchByCommand（首 token 精确匹配）", () => {
    it("精确命令命中对应 profile", () => {
      registry.register(makeProfile("claude"));
      const profile = registry.matchByCommand("claude");
      expect(profile).not.toBeNull();
      expect(profile!.id).toBe("claude");
    });

    it("多 commands 命中非首键（键集 [claude, cc]，cc --flag 命中）", () => {
      registry.register(makeProfile("claude", ["claude", "cc"]));
      const profile = registry.matchByCommand("cc --flag");
      expect(profile).not.toBeNull();
      expect(profile!.id).toBe("claude");
    });

    it("带参变体命中——claude --resume abc / claude -p 'hello'", () => {
      registry.register(makeProfile("claude"));
      expect(registry.matchByCommand("claude --resume abc")!.id).toBe("claude");
      expect(registry.matchByCommand("claude -p \"hi\"")!.id).toBe("claude");
    });

    it("前导空白 + 参数仍命中", () => {
      registry.register(makeProfile("claude"));
      expect(registry.matchByCommand("  claude --model opus")!.id).toBe("claude");
    });

    it("空命令行 → null", () => {
      registry.register(makeProfile("claude"));
      expect(registry.matchByCommand("")).toBeNull();
    });

    it("仅空白 → null", () => {
      registry.register(makeProfile("claude"));
      expect(registry.matchByCommand("   ")).toBeNull();
    });

    it("未命中命令 → null（npm install）", () => {
      registry.register(makeProfile("claude"));
      expect(registry.matchByCommand("npm install")).toBeNull();
    });

    it("大小写敏感（Claude/CLAUDE ≠ claude）→ null，不 toLowerCase", () => {
      registry.register(makeProfile("claude"));
      expect(registry.matchByCommand("Claude")).toBeNull();
      expect(registry.matchByCommand("CLAUDE")).toBeNull();
    });

    it("同首 token 多 profile 冲突时先注册者优先（注册序确定性）", () => {
      registry.register(makeProfile("a", ["claude"]));
      registry.register(makeProfile("b", ["claude"]));
      expect(registry.matchByCommand("claude")!.id).toBe("a");
    });
  });

  describe("生命周期", () => {
    it("_reset 清空 → get/getAll/matchByCommand 全空", () => {
      registry.register(makeProfile("claude"));
      registry._reset();
      expect(registry.get("claude")).toBeUndefined();
      expect(registry.getAll()).toEqual([]);
      expect(registry.matchByCommand("claude")).toBeNull();
    });

    it("_reset 后可重新 register 新 profile", () => {
      registry.register(makeProfile("claude"));
      registry._reset();
      registry.register(makeProfile("codex"));
      expect(registry.get("claude")).toBeUndefined();
      expect(registry.get("codex")).not.toBeUndefined();
    });
  });

  describe("单例", () => {
    it("全局单例存在且为 CliProfileRegistry 实例", () => {
      // 模块顶层 import 的 cliProfileRegistry 验证单例模式
      expect(cliProfileRegistry).toBeDefined();
      expect(cliProfileRegistry).toBeInstanceOf(CliProfileRegistry);
    });

    it("独立实例互不共享（new CliProfileRegistry 空表）", () => {
      cliProfileRegistry.register(makeProfile("claude"));
      const fresh = new CliProfileRegistry();
      expect(fresh.get("claude")).toBeUndefined();
      expect(fresh.getAll()).toEqual([]);
    });
  });

  describe("logo 资源守卫（MC-108 泛化，自 cli-profile-claude.test.ts 移入）", () => {
    it("遍历注册表全部 profile：iconSrc 对应磁盘文件存在 + PNG 魔数", () => {
      // mockcli 资源本 Stage 先行放入（决策 5），Stage 07 mock 夹具引用——
      // 此处临时注册 mockcli 形态 profile，证明守卫对每个 profile 生效（零特例）
      cliProfileRegistry.register({
        id: "mockcli",
        displayName: "mockcli",
        commands: ["mockcli"],
        iconSrc: "/cli-icons/mockcli.png",
        tabTitle: "mockcli",
        capabilities: {},
      });

      const profiles = cliProfileRegistry.getAll();
      expect(profiles.length).toBeGreaterThan(0);
      for (const profile of profiles) {
        const iconPath = resolve(here, `../../public${profile.iconSrc}`);
        // 资源缺失则构建产物 img 404 且无报错通道——靠此守卫兜底
        expect(
          existsSync(iconPath),
          `${profile.id} 的 logo ${profile.iconSrc} 磁盘文件缺失`,
        ).toBe(true);
        // PNG 魔数校验（防误放非 PNG 文件）
        const head = readFileSync(iconPath).subarray(0, 8);
        expect(
          Array.from(head),
          `${profile.id} 的 ${profile.iconSrc} 非合法 PNG`,
        ).toEqual([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
      }
    });
  });
});
