// cliProfileRegistry.ts — CLI profile 注册表（模块级单例，项目第 6 个注册表）
//
// 命令→profile 映射注册表。首 token 解析单点化（MC-102）：`trim().split(/\s+/)[0]`
// 全仓唯一实现（原 cliIcons.ts / TabTitleRegistry.ts 两份拷贝收敛于此）。
// OSC 133 handler 经此注册表匹配命令行取 profile（tabTitle/iconSrc），不硬编码命令名。
// 后续新增 CLI 只需在 profiles/<cli>/ 定义 profile 并注册，不修改核心逻辑。

import type { CodingCliProfile } from "./types";

/** CLI profile 注册表（模块级单例，同 TabTitleRegistry 模式） */
export class CliProfileRegistry {
  private profiles: Map<string, CodingCliProfile> = new Map();

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
   * 首 token 匹配命令行：取 commandLine.trim().split(/\s+/)[0] 后对
   * profile.commands 逐键精确查表，未匹配返回 null。
   * 覆盖 claude --resume / claude -p 等带参变体；空命令行/仅空白 → null；
   * 不 toLowerCase（大小写敏感）；同首 token 多 profile 冲突时先注册者优先。
   */
  matchByCommand(commandLine: string): CodingCliProfile | null {
    const firstToken = commandLine.trim().split(/\s+/)[0];
    for (const profile of this.profiles.values()) {
      if (profile.commands.includes(firstToken)) return profile;
    }
    return null;
  }

  /** 清空所有 profile（仅测试用） */
  _reset(): void {
    this.profiles.clear();
  }
}

/** 全局单例 */
export const cliProfileRegistry = new CliProfileRegistry();
