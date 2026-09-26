// agent-files-filtering.test.ts — 「Agent 全局文件」过滤/名单校验纯函数全分支测试
//
// filtering.ts 为硬约束 #12 纯函数模块（参数注入，零 store/React/IPC 依赖），
// 本文件直接构造入参断言全分支——sanitize 的孤儿判定经 profiles 数组注入（无需注册表）。

import { describe, it, expect } from "vitest";
import {
  defaultConfig,
  validateCustomName,
  sanitizeAgentGlobalFiles,
  shouldShowAtRoot,
  isRootEventRelevant,
} from "../features/agentFiles/filtering";
import type { CodingCliProfile } from "../features/cliProfiles/types";

/** light fake profile（仅 id 被 sanitize 消费） */
function fakeProfile(id: string): CodingCliProfile {
  return {
    id,
    displayName: id,
    commands: [id],
    iconSrc: `/cli-icons/${id}.png`,
    tabTitle: id,
    capabilities: {},
  };
}

const PROFILES = [fakeProfile("claude"), fakeProfile("codex")];
const RUNTIME_PATHS = ["projects", "sessions", "history.jsonl"];

describe("defaultConfig", () => {
  it("缺省 = 全部模式 + 空名单 + 不显示运行时文件", () => {
    expect(defaultConfig()).toEqual({
      mode: "all",
      customNames: [],
      showRuntimeFiles: false,
    });
  });

  it("每次调用返回新对象（消费方原地改不污染缺省）", () => {
    const a = defaultConfig();
    a.customNames.push("x");
    expect(defaultConfig().customNames).toEqual([]);
  });
});

describe("validateCustomName", () => {
  it("空白名称 → 「名称不能为空」", () => {
    expect(validateCustomName("", [])).toBe("名称不能为空");
    expect(validateCustomName("   ", [])).toBe("名称不能为空");
  });

  it("含路径分隔符（\\ 与 /）→ 「名称不能包含路径分隔符」", () => {
    expect(validateCustomName("a\\b", [])).toBe("名称不能包含路径分隔符");
    expect(validateCustomName("a/b", [])).toBe("名称不能包含路径分隔符");
  });

  it("大小写不敏感重复 → 「名称已存在」", () => {
    expect(validateCustomName("Projects", ["projects"])).toBe("名称已存在");
    expect(validateCustomName("projects", ["PROJECTS"])).toBe("名称已存在");
  });

  it("合法名称 → null（首尾空白 trim 后参与判定）", () => {
    expect(validateCustomName("settings.json", [])).toBeNull();
    expect(validateCustomName("  agents  ", ["other"])).toBeNull();
  });
});

describe("sanitizeAgentGlobalFiles", () => {
  it("段非对象 / null / 数组 → 全量空表", () => {
    expect(sanitizeAgentGlobalFiles(null, PROFILES)).toEqual({});
    expect(sanitizeAgentGlobalFiles("bad", PROFILES)).toEqual({});
    expect(sanitizeAgentGlobalFiles(["claude"], PROFILES)).toEqual({});
  });

  it("孤儿 cliId（未注册）整键丢弃", () => {
    const result = sanitizeAgentGlobalFiles(
      {
        claude: { mode: "custom", customNames: ["agents"], showRuntimeFiles: true },
        ghost: { mode: "all", customNames: [], showRuntimeFiles: false },
      },
      PROFILES,
    );
    expect(Object.keys(result)).toEqual(["claude"]);
  });

  it("合法配置原样保留", () => {
    const result = sanitizeAgentGlobalFiles(
      {
        claude: { mode: "custom", customNames: ["agents", "skills"], showRuntimeFiles: true },
      },
      PROFILES,
    );
    expect(result.claude).toEqual({
      mode: "custom",
      customNames: ["agents", "skills"],
      showRuntimeFiles: true,
    });
  });

  it("非法 mode / 非 boolean showRuntimeFiles / 非数组 customNames 逐项回退缺省", () => {
    const result = sanitizeAgentGlobalFiles(
      {
        claude: { mode: "everything", customNames: "agents", showRuntimeFiles: "yes" },
      },
      PROFILES,
    );
    expect(result.claude).toEqual(defaultConfig());
  });

  it("customNames 非法项（空白/含分隔符/非字符串）逐项剔除", () => {
    const result = sanitizeAgentGlobalFiles(
      {
        claude: {
          mode: "custom",
          customNames: ["agents", "  ", "a/b", "c\\d", 42, "skills"],
          showRuntimeFiles: false,
        },
      },
      PROFILES,
    );
    expect(result.claude.customNames).toEqual(["agents", "skills"]);
  });

  it("customNames 大小写不敏感去重（保留添加序首个）", () => {
    const result = sanitizeAgentGlobalFiles(
      {
        claude: {
          mode: "custom",
          customNames: ["Agents", "agents", "AGENTS", "skills"],
          showRuntimeFiles: false,
        },
      },
      PROFILES,
    );
    expect(result.claude.customNames).toEqual(["Agents", "skills"]);
  });

  it("单条配置非对象 → 该键回退缺省", () => {
    const result = sanitizeAgentGlobalFiles({ claude: "bad" }, PROFILES);
    expect(result.claude).toEqual(defaultConfig());
  });
});

describe("shouldShowAtRoot", () => {
  it("all 模式 + 显示运行时关 + runtimePaths 命中 → 不展示（大小写不敏感）", () => {
    const cfg = { mode: "all" as const, customNames: [], showRuntimeFiles: false };
    expect(shouldShowAtRoot("projects", cfg, RUNTIME_PATHS)).toBe(false);
    expect(shouldShowAtRoot("PROJECTS", cfg, RUNTIME_PATHS)).toBe(false);
    expect(shouldShowAtRoot("history.jsonl", cfg, RUNTIME_PATHS)).toBe(false);
  });

  it("all 模式 + 显示运行时关 + 非运行时项 → 展示", () => {
    const cfg = { mode: "all" as const, customNames: [], showRuntimeFiles: false };
    expect(shouldShowAtRoot("settings.json", cfg, RUNTIME_PATHS)).toBe(true);
    expect(shouldShowAtRoot("agents", cfg, RUNTIME_PATHS)).toBe(true);
  });

  it("all 模式 + 显示运行时开 → runtimePaths 命中项也展示", () => {
    const cfg = { mode: "all" as const, customNames: [], showRuntimeFiles: true };
    expect(shouldShowAtRoot("projects", cfg, RUNTIME_PATHS)).toBe(true);
  });

  it("custom 模式：仅名单完全同名展示（名单即完整真值，运行时开关不生效）", () => {
    const cfg = {
      mode: "custom" as const,
      customNames: ["Agents", "settings.json"],
      showRuntimeFiles: false,
    };
    expect(shouldShowAtRoot("agents", cfg, RUNTIME_PATHS)).toBe(true);
    expect(shouldShowAtRoot("settings.json", cfg, RUNTIME_PATHS)).toBe(true);
    expect(shouldShowAtRoot("skills", cfg, RUNTIME_PATHS)).toBe(false);
    // 名单含 runtime 项时同样展示——custom 模式运行时开关不生效
    const cfg2 = { ...cfg, customNames: ["projects"] };
    expect(shouldShowAtRoot("projects", cfg2, RUNTIME_PATHS)).toBe(true);
  });
});

describe("isRootEventRelevant", () => {
  const ROOT = "C:/Users/x/.claude";
  const ALL_CFG = { mode: "all" as const, customNames: [], showRuntimeFiles: false };

  it("根外路径 → false（防御）", () => {
    expect(isRootEventRelevant("C:/other/a.ts", ROOT, ALL_CFG, RUNTIME_PATHS)).toBe(false);
  });

  it("all 模式运行时首段（深层路径取首段判定）→ false", () => {
    expect(
      isRootEventRelevant(`${ROOT}/projects/sess/abc.jsonl`, ROOT, ALL_CFG, RUNTIME_PATHS),
    ).toBe(false);
  });

  it("all 模式非运行时首段 → true", () => {
    expect(
      isRootEventRelevant(`${ROOT}/settings.json`, ROOT, ALL_CFG, RUNTIME_PATHS),
    ).toBe(true);
    expect(
      isRootEventRelevant(`${ROOT}/agents/reviewer.md`, ROOT, ALL_CFG, RUNTIME_PATHS),
    ).toBe(true);
  });

  it("all 模式 + 显示运行时开 → 运行时首段事件放行", () => {
    const cfg = { ...ALL_CFG, showRuntimeFiles: true };
    expect(
      isRootEventRelevant(`${ROOT}/projects/a.jsonl`, ROOT, cfg, RUNTIME_PATHS),
    ).toBe(true);
  });

  it("custom 模式：名单外首段事件丢弃，名单内放行", () => {
    const cfg = { mode: "custom" as const, customNames: ["agents"], showRuntimeFiles: false };
    expect(
      isRootEventRelevant(`${ROOT}/skills/x.md`, ROOT, cfg, RUNTIME_PATHS),
    ).toBe(false);
    expect(
      isRootEventRelevant(`${ROOT}/agents/x.md`, ROOT, cfg, RUNTIME_PATHS),
    ).toBe(true);
  });

  it("路径分隔符形态差异（反斜杠根 + 正斜杠事件路径）仍正确取首段", () => {
    expect(
      isRootEventRelevant("C:\\Users\\x\\.claude/settings.json", ROOT, ALL_CFG, RUNTIME_PATHS),
    ).toBe(true);
    expect(
      isRootEventRelevant("C:\\Users\\x\\.claude\\projects\\a.jsonl", ROOT, ALL_CFG, RUNTIME_PATHS),
    ).toBe(false);
  });
});
