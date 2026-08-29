// SettingsPageRegistry.ts —— 配置页注册表（F11，硬约束 #13 注册表家族契约）
//
// 模块级单例：register 同 id 幂等覆盖 / getAll(group?) 按 order ?? 注册序 /
// get(id) / _reset() 仅测试（beforeEach/afterEach 调 _reset 保证用例隔离）。
// 注册经 side-effect import 触发（pages.ts——SettingsPanel 显式 import 即注册，
// 禁止隐式初始化）。

import type { SettingsPage, SettingsPageGroup } from "./types";

/** 配置页注册表（F11）——settingsCenter 模块级单例 */
class SettingsPageRegistry {
  private pages = new Map<string, SettingsPage>();

  /** 注册配置页——同 id 幂等覆盖（后注册者胜，注册序不重复累计） */
  register(page: SettingsPage): void {
    this.pages.set(page.id, page);
  }

  /**
   * 返回全部配置页（可选分组过滤），按 `order ?? 注册序` 排序——
   * 有 order 的页按数值升序在前，缺省 order 的页视作无穷大且稳定排序保持注册序。
   */
  getAll(group?: SettingsPageGroup): SettingsPage[] {
    const list = group
      ? [...this.pages.values()].filter((p) => p.group === group)
      : [...this.pages.values()];
    return list.sort(
      (a, b) =>
        (a.order ?? Number.MAX_SAFE_INTEGER) - (b.order ?? Number.MAX_SAFE_INTEGER),
    );
  }

  /** 按 id 获取配置页（未注册 → undefined） */
  get(id: string): SettingsPage | undefined {
    return this.pages.get(id);
  }

  /** 清空全部条目（仅测试用） */
  _reset(): void {
    this.pages.clear();
  }
}

/** 模块级单例 */
let instance: SettingsPageRegistry | null = null;

/** 获取 SettingsPageRegistry 单例（惰性初始化，照 getShortcutRegistry 先例） */
export function getSettingsPageRegistry(): SettingsPageRegistry {
  if (!instance) {
    instance = new SettingsPageRegistry();
  }
  return instance;
}
