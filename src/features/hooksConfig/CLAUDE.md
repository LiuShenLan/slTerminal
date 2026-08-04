# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

hooks 配置面板的 **schema 内嵌单点**（C13-1）——SchemaStore 官方 settings schema + hooks 子 schema 提取 + Draft07 校验。仅供 hooksConfig 面板（`src/panels/hooksConfig/`）消费；面板本身的架构决策（双模式编辑/事件目录/MatcherTester 等）见 @../../panels/CLAUDE.md，不在此展开。

## 架构决策

### SchemaStore 官方 schema 内嵌（P3-FE-07）

数据源 `https://json.schemastore.org/claude-code-settings.json`（2026-08-01 下载）。**升级方式**：整文件替换 `schema/claude-code-settings.json` 即可，离线可用、无网络请求。

**自包含性已核实**：全 schema 无远程 `$ref`——35 个本地 `$ref` 全部指向 `#/$defs/*`（`$defs` 仅 permissionRule / hookCommand / hookMatcher 三键）。codemirror-json-schema / json-schema-library 仅支持本地 `$ref`，无需预打包展开远程引用。

### hooks 子 schema 提取

`properties.hooks` + 依赖的 `$defs` 子集（hookMatcher + hookCommand，**不含** permissions 专用的 permissionRule）——保证 `#/$defs/hookMatcher` 本地引用在独立 schema 中可解析。供 JsonMode（补全/悬停/波浪线）与保存校验共用，对齐 hooks 子树编辑范围。

### Draft07 校验单例

`hooksDraft` 为 `Draft07` 单例（json-schema-library，非 ajv）——schema 固定不变，复用避免重复编译开销。`validateHooksJson` 供 JsonMode 波浪线与保存路径（P3-FE-17 双校验）共用。

## 文件

| 文件 | 职责 |
|------|------|
| `schema/claude-code-settings.json` | SchemaStore 官方 settings.json schema（2026-08-01 快照，升级时整文件替换） |
| `schema/index.ts` | schema 单点：`claudeCodeSettings`（完整 schema）+ `SCHEMA_ID`（$id 指纹）+ `hooksSubSchema`（hooks 子树提取）+ `JsonDiagnostic`/`JsonValidationResult` 类型 + `validateHooksJson`（Draft07 校验） |

## 测试模式

L2 测试位于 `src/__tests__/hooks-config-schema.test.ts`（HKC-08，用例数见 `.claude/test-inventory.md`）：直测 `validateHooksJson` 边界——合法 / 缺 hooks 键 / 非法 matcher / 未知事件告警。面板层测试见 @../../panels/CLAUDE.md hooksConfig 节。
