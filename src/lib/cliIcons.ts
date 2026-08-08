// cliIcons.ts — 编码 CLI → 品牌 logo 注册表（模块级单例）
//
// 三处 emoji 状态指示（终端页签 / 活跃会话 / 历史会话）在 emoji 后显示
// 当前 CLI 的品牌 logo 小图。CLI 通过命令行首 token 识别（与 TabTitleRegistry
// 同款匹配逻辑，覆盖 claude --resume 等带参变体）。
//
// 新增编码 CLI 只需两步（高内聚/低耦合/易扩展，项目注册表单例）：
//   1. 图片放 public/cli-icons/<命令>.png（32×32 透明底，渲染 16×16，随
//      frontendDist 内嵌 exe，根绝对路径同源加载——CSP img-src 'self' 放行）
//   2. 下方追加一行 cliIconRegistry.register({ command, src })

/** CLI logo 注册条目 */
export interface CliIconEntry {
  /** 命令行首 token（精确匹配键），如 "claude" */
  command: string;
  /** logo 图片根绝对路径（public/ 硬编码，如 "/cli-icons/claude.png"） */
  src: string;
}

/** CLI → logo 注册表（模块级单例，同 TabTitleRegistry 模式） */
export class CliIconRegistry {
  private entries: Map<string, string> = new Map();

  /** 注册一条 CLI logo（同 command 覆盖） */
  register(entry: CliIconEntry): void {
    this.entries.set(entry.command, entry.src);
  }

  /** 首 token 匹配命令行：取 command.trim().split(/\s+/)[0] 后精确查表，未匹配返回 null */
  match(commandLine: string): string | null {
    const firstToken = commandLine.trim().split(/\s+/)[0];
    return this.entries.get(firstToken) ?? null;
  }

  /** 精确键查询（侧栏恒 claude 场景用），未注册返回 null */
  getSrc(command: string): string | null {
    return this.entries.get(command) ?? null;
  }

  /** 清空所有条目（仅测试用） */
  _reset(): void {
    this.entries.clear();
  }
}

/** 全局单例（内嵌注册：任何消费方 import 即注册，测试零副作用） */
export const cliIconRegistry = new CliIconRegistry();
cliIconRegistry.register({ command: "claude", src: "/cli-icons/claude.png" });
