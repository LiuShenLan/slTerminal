// SchemeRegistry — 配色方案注册表
//
// 模块级单例，管理 ColorScheme 方案的注册与 active 切换（决策 D2）。
// 项目第 6 个注册表单例（先例：CliProfileRegistry / SideViewRegistry / ShortcutRegistry /
// FileViewerRegistry / TerminalRegistry），模式同 SideViewRegistry——
// register/get/getAll/_reset（active 状态为方案系统特有）。
//
// 内置方案经 src/theme/schemes/index.ts side-effect 注册（照 sideViewDefs.ts 模式），
// 本文件不直接 import 具体方案。getActive() 回退语义依赖 darcula 恒已注册。

import type { ColorScheme } from "./schemes/types";

/** 默认方案 id——darcula（唯一内置方案，未注册时 setActive 的回退目标） */
const DEFAULT_SCHEME_ID = "darcula";

/** 配色方案注册表——模块级单例 */
export class SchemeRegistry {
  private schemes: Map<string, ColorScheme> = new Map();
  /** 当前 active 方案 id，初始为默认 darcula */
  private activeId: string = DEFAULT_SCHEME_ID;

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

  /** 当前 active 方案——activeId 未注册（异常状态）时回退默认 darcula。
   *  正常路径下 darcula 恒已注册（schemes/index.ts side-effect），
   *  `!` 断言仅空表场景（测试 _reset 后）可能失效，由调用方承担。 */
  getActive(): ColorScheme {
    return this.schemes.get(this.activeId) ?? this.schemes.get(DEFAULT_SCHEME_ID)!;
  }

  /** 切换 active 方案——未知 id → console.warn + 回退默认 darcula */
  setActive(id: string): void {
    if (!this.schemes.has(id)) {
      console.warn(`[scheme] 未知配色方案 "${id}"，回退到默认方案 darcula`);
      this.activeId = DEFAULT_SCHEME_ID;
      return;
    }
    this.activeId = id;
  }

  /** 默认方案 id（darcula）——启动序列无配置时的回退目标 */
  getDefaultId(): string {
    return DEFAULT_SCHEME_ID;
  }

  /** 清空注册表 + active 复位默认 darcula（仅测试用） */
  _reset(): void {
    this.schemes.clear();
    this.activeId = DEFAULT_SCHEME_ID;
  }
}

/** 全局单例 */
export const schemeRegistry = new SchemeRegistry();
