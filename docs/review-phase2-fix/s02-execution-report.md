# S02 执行报告（TE-07 TypeScript 7 妥协方案背书）

> 本文档为 verify/stage-02.md TE-07 断言的背书载体：妥协结论（含升级触发条件）的正式记录。TE-07 验证循环中止于「妥协正式化」形态——主 typescript 字段直改 ^7.0.2 不可行，D14 决策树三支 fallback 全部实测走尽，最终保留 side-by-side 形态并登记工程债务。
> 生成于 Stage 2 修复循环（妥协正式化）；ADR 登记义务由 S10-C 收口（stages.md S10-C「TE-07 结果 + TE-15 债务」），本报告只留结论不写 ADR。

## 1. 妥协摘要

**TE-07（TypeScript 主字段直改 ^7.0.2）判定为不可行，正式化妥协：双 TS 并存（side-by-side）。**

- `package.json:80` `"typescript": "npm:@typescript/typescript6@^6.0.2"`（TS6 包装器，保留）——typescript-eslint 8.x 全系（含最新 8.67.0）只接受 `<6.1.0`
- `package.json:59` `"@typescript/native": "npm:typescript@^7.0.2"`（保留）——tsc bin 由它提供，`npx tsc --version` = 7.0.2，编译器实际为 TS7
- `package.json:81` `"typescript-eslint": "^8.67.0"`（保留）
- 该形态下全门禁绿：`npx tsc --noEmit` / `npx eslint src/` / `npm test` / `npx tauri build --debug --no-bundle`

## 2. 兼容性实查证据

- `npm view typescript-eslint@latest` = **8.67.0**（D14 要求执行前实查，禁凭印象）
- `npm view typescript-eslint@8.67.0 peerDependencies` = `{ eslint: '^8.57.0 || ^9.0.0 || ^10.0.0', typescript: '>=4.8.4 <6.1.0' }`
- **结论：最新版也不支持 TS7**，且 8.67.0 模块加载期硬拒绝 `ts.versionMajorMinor >= 7`（与 type-aware 规则开关无关，崩在加载期）

## 3. D14 fallback 三支实测走尽

review 原文（docs/review-phase2/01-依赖升级.md P1#2）允许的退路逐支核实：

| 分支 | 实测结果 |
|------|----------|
| 升级 typescript-eslint 至支持 TS7 的版本 | 不存在——8.67.0 已是 latest，peerDependencies 上限 `<6.1.0`，全系拒绝 TS7 |
| `overrides` 钉兼容组合 | npm 机制不可行——peer override 与根依赖 `^7.0.2` 冲突，typescript-eslint 只能解析单实例 typescript，钉 6.x 即失掉 TS7 主字段 |
| 暂停 type-aware 规则 | 无效——拒绝发生在模块加载期（`ts.versionMajorMinor >= 7` 硬校验），与规则开关无关 |

三支走尽后，唯一不破坏门禁且保住 TS7 编译器能力的形态即当前 side-by-side。

## 4. 实际安装状态（2026-08-22 实跑）

```
npx tsc --version            → Version 7.0.2
npm ls typescript            → 单实例 npm:@typescript/typescript6@6.0.2，无 invalid 标记
                               （typescript-eslint 8.67.0 树全部 deduped）
npm ls @typescript/native    → npm:typescript@7.0.2，单实例
```

## 5. 升级触发条件（移除 TS6 包装器的先决条件）

同时满足以下两条后，执行 TE-07 原目标（主 typescript 直改 ^7.x、删除 TS6 包装器与类型别名）：

1. **typescript-eslint issue [#10940](https://github.com/typescript-eslint/typescript-eslint/issues/10940) 闭环**——官方发布支持 TS7 的版本（peerDependencies 上限放开 ≥7.x，加载期校验同步放宽）
2. **TS7.1 稳定发布**——避让 7.0.x 首版窗口期风险，以 7.1 为稳定基线

触发后操作：`package.json` 删 `@typescript/native` 别名与 `npm:@typescript/typescript6` 包装器，`"typescript"` 直改 `^7.1.0`，`npm install` 刷 lock，全量门禁回归。
