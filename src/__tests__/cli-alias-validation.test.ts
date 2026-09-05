// cli-alias-validation.test.ts — CLI 别名校验与加载净化纯函数（aliasValidation.ts）
//
// 纯参数构造（fake profiles / aliasesByCli），零注册表依赖；语义来源 D2/D3/D7：
// 语法（空/超长/空白/引号反引号/`-` 开头）→ D3 全命名空间唯一（大小写敏感精确比较），
// sanitize = 加载兜底（孤儿 cliId/违例条目/跨源重复丢弃，先到先占）。

import { describe, it, expect } from "vitest";
import {
  validateCliAlias,
  sanitizeCliAliases,
  CLI_ALIAS_MAX_LENGTH,
} from "../features/cliProfiles/aliasValidation";
import type { CodingCliProfile } from "../features/cliProfiles/types";

/** fake profiles：claude（commands ["claude"]）+ codex（commands ["codex"]），注册序固定 */
const PROFILES: CodingCliProfile[] = [
  { id: "claude", displayName: "Claude Code", commands: ["claude"], iconSrc: "/cli-icons/claude.png", tabTitle: "claude", capabilities: {} },
  { id: "codex", displayName: "Codex", commands: ["codex"], iconSrc: "/cli-icons/codex.png", tabTitle: "codex", capabilities: {} },
];

/** 校验上下文默认夹具：claude 下已有别名 cc，codex 下已有别名 cc2 */
const CLAUDE_CTX = {
  cliId: "claude",
  aliasesByCli: { claude: ["cc"], codex: ["cc2"] },
  profiles: PROFILES,
};

function validate(raw: string) {
  return validateCliAlias(raw, CLAUDE_CTX);
}

/** 构造 n 字符字符串 */
function strOf(n: number): string {
  return "a".repeat(n);
}

describe("validateCliAlias 语法（D7）", () => {
  it("合法别名放行并返回 trim 后文本", () => {
    // 注意夹具已占用 cc/cc2/claude/codex，正例用中性别名
    expect(validate("extra")).toEqual({ ok: true, alias: "extra" });
    // 连字符/点/下划线/中文等命令名字符放行
    expect(validate("claude-code")).toEqual({ ok: true, alias: "claude-code" });
    expect(validate("c_c")).toEqual({ ok: true, alias: "c_c" });
    expect(validate("中文命令")).toEqual({ ok: true, alias: "中文命令" });
  });

  it("前后空白自动 trim（不报错），内部空白拒绝", () => {
    expect(validate("  extra  ")).toEqual({ ok: true, alias: "extra" });
    expect(validate("c c")).toEqual({ ok: false, message: "别名不能包含空格" });
    expect(validate("c\nc")).toEqual({ ok: false, message: "别名不能包含空格" });
  });

  it("空串/纯空白 → 不能为空", () => {
    expect(validate("")).toEqual({ ok: false, message: "别名不能为空" });
    expect(validate("   ")).toEqual({ ok: false, message: "别名不能为空" });
  });

  it("超长拒绝——边界 32 放行 / 33 拒绝", () => {
    expect(validate(strOf(CLI_ALIAS_MAX_LENGTH))).toEqual({
      ok: true,
      alias: strOf(CLI_ALIAS_MAX_LENGTH),
    });
    expect(validate(strOf(CLI_ALIAS_MAX_LENGTH + 1))).toEqual({
      ok: false,
      message: `别名过长（最多 ${CLI_ALIAS_MAX_LENGTH} 个字符）`,
    });
  });

  it("引号/反引号拒绝（单/双/反引号）", () => {
    expect(validate("it's")).toEqual({ ok: false, message: "别名不能包含引号或反引号" });
    expect(validate('say"hi')).toEqual({ ok: false, message: "别名不能包含引号或反引号" });
    expect(validate("back`tick")).toEqual({ ok: false, message: "别名不能包含引号或反引号" });
  });

  it("以 - 开头拒绝（旗标形态混淆）", () => {
    expect(validate("-x")).toEqual({ ok: false, message: "别名不能以 - 开头" });
  });
});

describe("validateCliAlias 唯一性（D3，大小写敏感 D2）", () => {
  it("同 cli 别名重复 → 已存在", () => {
    expect(validate("cc")).toEqual({ ok: false, message: "别名「cc」已存在" });
  });

  it("撞本 cli 内置命令 → 本 CLI 的内置命令（含 claude 名下加 claude 通用覆盖）", () => {
    expect(validate("claude")).toEqual({
      ok: false,
      message: "「claude」是本 CLI 的内置命令",
    });
  });

  it("撞其它 cli 内置命令 → 指向其 displayName", () => {
    expect(validate("codex")).toEqual({
      ok: false,
      message: "「codex」是「Codex」的内置命令",
    });
  });

  it("撞其它 cli 别名 → 指向其 displayName", () => {
    expect(validate("cc2")).toEqual({
      ok: false,
      message: "「cc2」已被「Codex」的别名使用",
    });
  });

  it("大小写敏感：CC ≠ cc、Codex ≠ codex，均可配置", () => {
    expect(validate("CC")).toEqual({ ok: true, alias: "CC" });
    expect(validate("Codex")).toEqual({ ok: true, alias: "Codex" });
  });
});

describe("sanitizeCliAliases（加载兜底）", () => {
  it("非对象输入（null/数组/字符串/数字）→ 空表", () => {
    expect(sanitizeCliAliases(null, PROFILES)).toEqual({});
    expect(sanitizeCliAliases(["cc"], PROFILES)).toEqual({});
    expect(sanitizeCliAliases("cc", PROFILES)).toEqual({});
    expect(sanitizeCliAliases(42, PROFILES)).toEqual({});
  });

  it("合法段原样保留（含空数组键）", () => {
    const raw = { claude: ["cc", "c"], codex: [] };
    expect(sanitizeCliAliases(raw, PROFILES)).toEqual(raw);
  });

  it("孤儿 cliId 键（无注册 profile）→ 整键丢弃", () => {
    const out = sanitizeCliAliases({ claude: ["cc"], ghost: ["gg"] }, PROFILES);
    expect(out).toEqual({ claude: ["cc"] });
  });

  it("值非数组 → 整键丢弃", () => {
    const out = sanitizeCliAliases({ claude: "cc", codex: 42 }, PROFILES);
    expect(out).toEqual({});
  });

  it("单条非字符串 / 语法不过 → 逐条丢弃，其余保留", () => {
    const raw = {
      claude: ["cc", 123, null, "c c", "'q'", "-x", "", "   ", "ok"],
    };
    expect(sanitizeCliAliases(raw, PROFILES)).toEqual({ claude: ["cc", "ok"] });
  });

  it("撞任何 profile 内置命令 → 丢弃", () => {
    const out = sanitizeCliAliases({ claude: ["claude", "codex", "cc"] }, PROFILES);
    expect(out).toEqual({ claude: ["cc"] });
  });

  it("跨 cli 重复 → 先到先占（claude 在 codex 前按注册序），后到者丢弃", () => {
    const raw = { claude: ["cc"], codex: ["cc", "xx"] };
    expect(sanitizeCliAliases(raw, PROFILES)).toEqual({ claude: ["cc"], codex: ["xx"] });
  });

  it("同 cli 数组内重复 → 首个保留", () => {
    expect(sanitizeCliAliases({ claude: ["cc", "cc"] }, PROFILES)).toEqual({
      claude: ["cc"],
    });
  });

  it("保留条目 trim 后落表", () => {
    expect(sanitizeCliAliases({ claude: ["  cc  "] }, PROFILES)).toEqual({
      claude: ["cc"],
    });
  });

  it("孤儿与违例并存时合法部分仍工作（组合兜底）", () => {
    const out = sanitizeCliAliases(
      { claude: ["claude", "ok", "cc", "bad name"], ghost: ["gg"], codex: [] },
      PROFILES,
    );
    expect(out).toEqual({ claude: ["ok", "cc"], codex: [] });
  });
});
