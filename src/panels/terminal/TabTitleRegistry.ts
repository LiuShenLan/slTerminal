// TabTitleRegistry — 命令→标题/图标映射注册表
//
// 模块级单例，管理 terminal 命令运行时页签标题和图标映射。
// OSC 133 handler 通过此注册表匹配命令，不硬编码单个命令名。
// 后续新增命令只需在 tabRules.ts 追加规则，不修改核心逻辑。

/** 命令→标题/图标映射规则 */
export interface TabTitleRule {
  /** 命令行首 token（精确匹配键），如 "claude" */
  command: string;
  /** 命令运行时页签标题 */
  title: string;
  /** Vite import 的图标资源路径（可选） */
  icon?: string;
}

/** 页签状态变化事件 */
export interface TabState {
  /** 命令是否启动（true=启动, false=退出） */
  active: boolean;
  /** 命令运行时标题（active=true 时有效） */
  title?: string;
  /** 命令运行时图标（active=true 时有效，null=无图标） */
  icon?: string | null;
}

/** 命令→页签标题/图标注册表（模块级单例） */
export class TabTitleRegistry {
  private rules: Map<string, TabTitleRule> = new Map();

  /** 注册一条命令规则 */
  register(rule: TabTitleRule): void {
    this.rules.set(rule.command, rule);
  }

  /** 首 token 匹配命令行：取 command.trim().split(/\s+/)[0] 后精确查表，未匹配返回 null。
   *  覆盖 claude --resume / claude -p 等带参变体。 */
  match(command: string): TabTitleRule | null {
    const firstToken = command.trim().split(/\s+/)[0];
    return this.rules.get(firstToken) ?? null;
  }

  /** 清空所有规则（仅测试用） */
  _reset(): void {
    this.rules.clear();
  }
}

/** 全局单例 */
export const tabTitleRegistry = new TabTitleRegistry();
