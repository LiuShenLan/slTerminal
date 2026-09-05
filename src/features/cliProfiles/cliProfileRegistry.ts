// cliProfileRegistry.ts — CLI profile 注册表（模块级单例，项目第 6 个注册表）
//
// 命令→profile 映射注册表。首 token 解析单点化（MC-102）：`trim().split(/\s+/)[0]`
// 全仓唯一实现（原 cliIcons.ts / TabTitleRegistry.ts 两份拷贝收敛于此）。
// OSC 133 handler 经此注册表匹配命令行取 profile（tabTitle/iconSrc），不硬编码命令名。
// 后续新增 CLI 只需在 profiles/<cli>/ 定义 profile 并注册，不修改核心逻辑。
//
// 用户别名（cliAliases 段，见 aliasValidation.ts）以旁路快照参与匹配：
// profile.commands 保持「内置命令静态声明」语义（D3 命名空间计算的唯一真值源），
// 别名经 setAliases 平铺进 aliasByToken 逆映射，matchByCommand 内置未命中后回退查表
// （ADR-0015）——别名命中的效果与内置完全一致（返回同一 profile 对象）。

import type { CodingCliProfile } from "./types";

/** CLI profile 注册表（模块级单例，register/get/getAll/matchByCommand/setAliases/_reset） */
export class CliProfileRegistry {
  private profiles: Map<string, CodingCliProfile> = new Map();
  /** 别名快照：别名 token → cliId（逆映射平铺）。仅经 setAliases 全量重建 */
  private aliasByToken: Map<string, string> = new Map();

  /** 注册一个 CLI profile（同 id 覆盖旧条目，注册序不变） */
  register(profile: CodingCliProfile): void {
    this.profiles.set(profile.id, profile);
  }

  /** 按 cliId 精确查询，未注册返回 undefined */
  get(id: string): CodingCliProfile | undefined {
    return this.profiles.get(id);
  }

  /** 全部 profile，按注册序返回 */
  getAll(): CodingCliProfile[] {
    return Array.from(this.profiles.values());
  }

  /**
   * 全量替换别名快照（cliAliases store 变更即调，低频；空对象 = 清空）。
   * 平铺重建逆映射；别名语义与 profile.commands 相同（首词精确、大小写敏感）。
   */
  setAliases(byCliId: Readonly<Record<string, readonly string[]>>): void {
    this.aliasByToken.clear();
    for (const [cliId, aliases] of Object.entries(byCliId)) {
      for (const alias of aliases) this.aliasByToken.set(alias, cliId);
    }
  }

  /**
   * 首 token 匹配命令行：取 commandLine.trim().split(/\s+/)[0] 后对
   * profile.commands 逐键精确查表，内置未命中再查别名快照，未匹配返回 null。
   * 覆盖 claude --resume / claude -p 等带参变体；空命令行/仅空白 → null；
   * 不 toLowerCase（大小写敏感）；同首 token 多 profile 冲突时先注册者优先。
   * 别名命中返回映射的 profile 对象（消费方按 cliId 归位，与内置命中零差异）。
   */
  matchByCommand(commandLine: string): CodingCliProfile | null {
    const firstToken = commandLine.trim().split(/\s+/)[0];
    for (const profile of this.profiles.values()) {
      if (profile.commands.includes(firstToken)) return profile;
    }
    const cliId = this.aliasByToken.get(firstToken);
    return cliId !== undefined ? (this.profiles.get(cliId) ?? null) : null;
  }

  /** 清空全部注册态（profile + 别名快照；仅测试用） */
  _reset(): void {
    this.profiles.clear();
    this.aliasByToken.clear();
  }
}

/** 全局单例 */
export const cliProfileRegistry = new CliProfileRegistry();
