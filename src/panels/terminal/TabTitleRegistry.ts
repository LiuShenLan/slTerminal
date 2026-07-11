// TabTitleRegistry — 命令→标题/图标映射注册表
//
// 模块级单例，管理 terminal 命令运行时页签标题和图标映射。
// OSC 133 handler 通过此注册表匹配命令，不硬编码单个命令名。
// 后续新增命令只需在 tabRules.ts 追加规则，不修改核心逻辑。

/** 命令→标题/图标映射规则 */
export interface TabTitleRule {
  /** trim() 后精确匹配的命令名 */
  command: string;
  /** 命令运行时页签标题 */
  title: string;
  /** Vite import 的图标资源路径 */
  icon: string;
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

  /** 精确匹配命令名，未匹配返回 null */
  match(command: string): TabTitleRule | null {
    return this.rules.get(command) ?? null;
  }

  /** 清空所有规则（仅测试用） */
  _reset(): void {
    this.rules.clear();
  }
}

/** 全局单例 */
export const tabTitleRegistry = new TabTitleRegistry();
