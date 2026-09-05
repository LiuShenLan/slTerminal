// aliasValidation.ts —— CLI 别名校验与加载净化纯函数（D7/D3）
//
// 归属：cliProfiles 域。别名是「命令首 token 命名空间」的用户态扩展，命名空间知识
// （内置命令集 = 各 profile.commands、displayName）的唯一知识源在 cliProfiles 域——
// 校验/净化收于此，注册表/store/页面共享同一真值源。
// 纪律：本文件零 import 注册表/store/React（依赖经参数注入），保持纯函数可测；
// 也正因如此 store 可安全引用本文件（硬约束 #12：校验不进 store，store 只存状态与转换）。

import type { CodingCliProfile } from "./types";

/** 别名长度上限（shell 命令名惯例远小于此，防误输入堆积） */
export const CLI_ALIAS_MAX_LENGTH = 32;

/** 校验结果：ok 时携带提交用的 trim 后别名；失败携带用户可见中文消息（直接渲染） */
export type AliasValidation =
  | { ok: true; alias: string }
  | { ok: false; message: string };

/** 校验上下文：所在 cliId + 全量别名表 + 全部注册 profile（内置命令名来源） */
export interface AliasValidationCtx {
  cliId: string;
  aliasesByCli: Readonly<Record<string, readonly string[]>>;
  profiles: readonly CodingCliProfile[];
}

/// 语法判定（作用于 trim 后文本）——D7：只禁 空白/引号/反引号/以 `-` 开头，其余放行；
/// 空与超长也在此拦（空 = trim 后为空串）
function syntaxError(alias: string): string | null {
  if (alias.length === 0) return "别名不能为空";
  if (alias.length > CLI_ALIAS_MAX_LENGTH)
    return `别名过长（最多 ${CLI_ALIAS_MAX_LENGTH} 个字符）`;
  if (/\s/.test(alias)) return "别名不能包含空格";
  if (/['"`]/.test(alias)) return "别名不能包含引号或反引号";
  if (alias.startsWith("-")) return "别名不能以 - 开头";
  return null;
}

/**
 * 校验一个待添加别名（D2/D3/D7）。
 * 判定顺序：语法（空/超长/空白/引号/`-` 开头）→ D3 全命名空间唯一性。
 * 唯一性为大小写敏感精确比较（D2：cc ≠ CC）；trim 后提交（前后空白自动去除）。
 * 错误消息区分四类冲突来源，均含用户可见别名/归属，直接渲染行内红字。
 */
export function validateCliAlias(
  raw: string,
  ctx: AliasValidationCtx,
): AliasValidation {
  const alias = raw.trim();

  const syntaxMsg = syntaxError(alias);
  if (syntaxMsg !== null) return { ok: false, message: syntaxMsg };

  // D3①：同 cli 别名数组内重复
  if ((ctx.aliasesByCli[ctx.cliId] ?? []).includes(alias)) {
    return { ok: false, message: `别名「${alias}」已存在` };
  }
  // D3②：撞任何 profile 的内置命令（区分本 cli 与他 cli，含「claude 名下加 claude」通用覆盖）
  for (const profile of ctx.profiles) {
    if (profile.commands.includes(alias)) {
      return {
        ok: false,
        message:
          profile.id === ctx.cliId
            ? `「${alias}」是本 CLI 的内置命令`
            : `「${alias}」是「${profile.displayName}」的内置命令`,
      };
    }
  }
  // D3③：撞其它 cli 的别名
  for (const [otherCliId, aliases] of Object.entries(ctx.aliasesByCli)) {
    if (otherCliId !== ctx.cliId && aliases.includes(alias)) {
      const other = ctx.profiles.find((p) => p.id === otherCliId);
      const displayName = other?.displayName ?? otherCliId;
      return { ok: false, message: `「${alias}」已被「${displayName}」的别名使用` };
    }
  }
  return { ok: true, alias };
}

/**
 * 加载净化（loadFromDisk 兜底，settings.json 可能被手改/版本残留出违例数据）：
 * 1. 顶层非对象 → 空表；孤儿 cliId 键（无对应注册 profile）→ 丢弃整键；
 * 2. 值非数组 → 丢弃整键；
 * 3. 单条非字符串或语法不过 → 丢弃该条；
 * 4. D3 去重：撞任何 profile 内置命令 → 丢弃；撞先前已保留的别名 token
 *    （含本 cli 与其它 cli）→ 丢弃，先出现者保留（cliId 按 profiles 注册序、数组按文件序）；
 * 5. 空数组键保留（与运行期「删光别名后键仍存在」语义一致，防 store/磁盘形态抖动）。
 */
export function sanitizeCliAliases(
  raw: unknown,
  profiles: readonly CodingCliProfile[],
): Record<string, string[]> {
  const out: Record<string, string[]> = {};
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) return out;

  const rawByCli = raw as Record<string, unknown>;
  const reserved: string[] = []; // 各 profile 内置命令 + 已保留别名 token，先到先占
  for (const profile of profiles) reserved.push(...profile.commands);

  for (const profile of profiles) {
    const value = rawByCli[profile.id];
    if (!Array.isArray(value)) continue; // 缺失/非数组 → 键不出现（或整键丢弃）
    const kept: string[] = [];
    for (const item of value) {
      if (typeof item !== "string") continue;
      const alias = item.trim();
      if (syntaxError(alias) !== null) continue; // 语法不过丢弃（净化不做逐条报错）
      if (reserved.includes(alias)) continue; // 撞内置/先前别名 → 丢弃
      reserved.push(alias);
      kept.push(alias);
    }
    out[profile.id] = kept; // 空数组也保留（规则 5）
  }
  return out;
}
