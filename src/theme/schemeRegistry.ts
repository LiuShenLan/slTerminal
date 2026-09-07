// SchemeRegistry — 配色方案注册表
//
// 模块级单例，管理 ColorScheme 方案的注册与 active 切换（决策 D2）。
// 项目第 6 个注册表单例（先例：CliProfileRegistry / SideViewRegistry / ShortcutRegistry /
// FileViewerRegistry / TerminalRegistry），模式同 SideViewRegistry——
// register/get/getAll/_reset（active 状态为方案系统特有）。
//
// 内置方案经 src/theme/schemes/index.ts side-effect 注册（照 sideViewDefs.ts 模式），
// 本文件不直接 import 具体方案。getActive() 回退语义依赖 linear 恒已注册。

import type { ColorScheme } from "./schemes/types";

/** 默认方案 id——linear（唯一内置方案，未注册时 setActive 的回退目标） */
const DEFAULT_SCHEME_ID = "linear";

/** 配色方案注册表——模块级单例 */
export class SchemeRegistry {
  private schemes: Map<string, ColorScheme> = new Map();
  /** 当前 active 方案 id，初始为默认 linear */
  private activeId: string = DEFAULT_SCHEME_ID;
  /** active 方案变化监听器集合（CP-039：editorTheme 订阅化） */
  private listeners = new Set<() => void>();

  /** 注册配色方案（同 id 覆盖——项目惯例） */
  register(scheme: ColorScheme): void {
    this.schemes.set(scheme.id, scheme);
  }

  /** 按 id 查询方案，未注册返回 undefined */
  get(id: string): ColorScheme | undefined {
    return this.schemes.get(id);
  }

  /** 返回所有已注册方案（注册序） */
  getAll(): ColorScheme[] {
    return Array.from(this.schemes.values());
  }

  /** 当前 active 方案——activeId 未注册（异常状态）时回退默认 linear。
   *  正常路径下 linear 恒已注册（schemes/index.ts side-effect），
   *  `!` 断言仅空表场景（测试 _reset 后）可能失效，由调用方承担。 */
  getActive(): ColorScheme {
    return this.schemes.get(this.activeId) ?? this.schemes.get(DEFAULT_SCHEME_ID)!;
  }

  /** 订阅 active 方案变化——返回取消函数（卸载/测试清理时调用；CP-039） */
  onDidChange(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** 通知全部监听器（setActive 两条路径——已知 id / 未知 id 回退 linear——均触发） */
  private notifyChange(): void {
    for (const listener of this.listeners) listener();
  }

  /** 切换 active 方案——未知 id → console.warn + 回退默认 linear */
  setActive(id: string): void {
    if (!this.schemes.has(id)) {
      console.warn(`[scheme] 未知配色方案 "${id}"，回退到默认方案 linear`);
      this.activeId = DEFAULT_SCHEME_ID;
      this.notifyChange();
      return;
    }
    this.activeId = id;
    this.notifyChange();
  }

  /** 默认方案 id（linear）——启动序列无配置时的回退目标 */
  getDefaultId(): string {
    return DEFAULT_SCHEME_ID;
  }

  /** 清空注册表 + active 复位默认 linear（仅测试用）；
   *  notifyChange 保证测试隔离一致——监听方（编辑器主题槽）也收到复位信号。
   *  顺序红线：先复位 active、通知（通知窗口内 getActive() 恒可读——若表先清空再通知，
   *  主题槽回调即时取色编译 bundle 会读到空表 undefined），最后才清空表。
   *  注意：listeners 集合本身不被 _reset 清，监听方以自己的取消函数收口。 */
  _reset(): void {
    this.activeId = DEFAULT_SCHEME_ID;
    this.notifyChange();
    this.schemes.clear();
  }
}

/** 全局单例 */
export const schemeRegistry = new SchemeRegistry();
