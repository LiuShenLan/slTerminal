// E2E helper 注入总开关。
//
// 背景：E2E helper（__slterm_e2e_* / __e2e_*）曾用 `import.meta.env.DEV` 单独门控，
// 但 E2E 所用的 `tauri build --debug` 前端仍走 `vite build`（production，DEV=false），
// 导致 helper 被 tree-shake、二进制不含就绪信号，wdio 全部卡在"Workspace 未就绪"。
// 改由本开关统一门控：dev serve 时 DEV=true；E2E 构建经 VITE_E2E=1 打开；
// 生产发布构建两者皆为编译期字面量 false → 整块 tree-shake，二进制不含测试后门。

/** 纯逻辑：供单测全表覆盖（无需 env stub）。 */
export function computeE2eEnabled(
  dev: boolean,
  viteE2e: string | undefined,
): boolean {
  return dev || viteE2e === "1";
}

// 注意：此处**内联** import.meta.env 表达式而非调用 computeE2eEnabled——
// 函数调用会阻碍 Rollup 跨模块 DCE，可能导致生产误带 helper。
// 两者逻辑等价，由 e2e-enabled.test.ts 断言一致性防漂移。
export const E2E_ENABLED =
  import.meta.env.DEV || import.meta.env.VITE_E2E === "1";
