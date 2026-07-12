// activePointer.ts — 泛型聚焦指针工厂
//
// 解决多面板共享命令时的派发问题：命令 handler 不闭包捕获特定实例，
// 而是经此指针派发到当前聚焦实例（focusin 时设置，focusout 时清除）。
// 与 activeTerminal.ts / activeEditor.ts 的模块级指针语义一致，
// 提取为泛型工厂消除重复。

/** 聚焦实例指针工厂，返回 { setActive, clearActive, getActive } 三元组 */
export function createActivePointer<T>() {
  let active: T | null = null;

  return {
    /** 设置当前聚焦实例 */
    setActive(a: T): void {
      active = a;
    },

    /**
     * 清除当前聚焦实例。
     * 传入 a 时仅当 active === a 才清（防竞态：A blur → B focus 后 A 的 blur 不清 B）。
     * 不传入时无条件清空。
     */
    clearActive(a?: T): void {
      if (!a || active === a) {
        active = null;
      }
    },

    /** 获取当前聚焦实例（无聚焦实例时返回 null） */
    getActive(): T | null {
      return active;
    },
  };
}
