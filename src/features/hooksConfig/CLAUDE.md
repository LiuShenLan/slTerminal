# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

hooks 配置面板的 **schema 内嵌单点**（C13-1）——SchemaStore 官方 settings schema + hooks 子 schema 提取 + Draft07 校验。仅供 hooksConfig 面板（`src/panels/hooksConfig/`）消费；面板本身的架构决策（双模式编辑/事件目录/MatcherTester 等）见 @../../panels/CLAUDE.md，不在此展开。

> **claude 专属编辑器语义（MC-223，决策 2）**：本模块（及 `src/panels/hooksConfig/` 面板）是 **claude 专属编辑器**——claude hooks 协议知识（eventsCatalog 30 事件 / matcherEngine / 5 种 handler 字段矩阵 / schema 内嵌 / Draft07 校验）**不抽象**为通用能力，文件物理位置保留现状（决策 2）。协议知识只属于 claude profile 域（与 `features/cliProfiles/profiles/claude/` 的 claude 合法领地同源），不为多 CLI 泛化；面板选择行允许其他 CLI 挂载自有编辑器，但本模块的 schema 单点仍是 claude 专属资产。
>
> **层声明入 profile（KZ-4）**：hooks 配置层集合（`{ id, label, hint }[]`）定义于 claude profile 的 `capabilities.hooks.configLayers`（`features/cliProfiles/profiles/claude/`，三层值 + 文案迁自编辑器退役 LAYERS 常量）——编辑器层切换器数据源 = `profile.configLayers`，本模块与编辑器不再持有层集合常量；`HooksLayer` **FE-14 收窄为 `"user" | "project" | "local"` 联合**（`src/types/hooksConfig.ts`——当前仅 claude 三层，未来 CLI 加层再泛化；后端 parse_layer 只认这三值，TS 联合编译期防误传），值集声明与联合类型须保持一致。

> **SEC-05 保存语义（D9）**：`user` 层写入 settings.json 前前端 `confirmDialog` **二次确认**（`src/panels/hooksConfig/useHooksConfig.ts`——hooks 可执行任意命令，用户级配置影响所有项目会话，取消不写盘；project/local 层不确认）。配合后端写入语义校验：事件名白名单（HOOK_EVENTS 10 事件）、handler `type` 白名单（`"command"`）、`command` 非空字符串审查。

## 架构决策

### SchemaStore 官方 schema 内嵌（P3-FE-07）

数据源 `https://json.schemastore.org/claude-code-settings.json`（2026-08-01 下载）。**升级方式**：整文件替换 `schema/claude-code-settings.json` 即可，离线可用、无网络请求。

**自包含性已核实**：全 schema 无远程 `$ref`——35 个本地 `$ref` 全部指向 `#/$defs/*`（`$defs` 仅 permissionRule / hookCommand / hookMatcher 三键）。codemirror-json-schema / json-schema-library 仅支持本地 `$ref`，无需预打包展开远程引用。

### hooks 子 schema 提取

`properties.hooks` + 依赖的 `$defs` 子集（hookMatcher + hookCommand，**不含** permissions 专用的 permissionRule）——保证 `#/$defs/hookMatcher` 本地引用在独立 schema 中可解析。供 JsonMode（悬停/波浪线）与保存校验共用，对齐 hooks 子树编辑范围。

### Draft07 校验单例（TE-09：json-schema-library 9.x → 11.x）

`hooksSchemaNode` 为 `compileSchema(schema, { draft: "draft-07" })` 单例（json-schema-library 11.x，非 ajv）——schema 固定不变，复用避免重复编译开销。**11.x 完整重写**：旧 API `new Draft07(schema).validate(data) → JsonError[]` 已移除，新 API = `compileSchema().validate(data) → { valid, errors }`；本 schema 无 `$schema` 字段，compileSchema 缺省会选 draft-2020-12——**显式 `draft: "draft-07"` 保持旧语义**（实测九边界全等价，含 anyOf 消息形态）；errors 的 pointer 由旧 `dataPath`/`pointer` 移至 `error.data.pointer`。`validateHooksJson` 供 JsonMode 波浪线与保存路径（P3-FE-17 双校验）共用。

> **TE-09 去重评估结论（S19 登记）**：codemirror-json-schema 0.8.1（npm 最新）内部 `import { Draft04 } from "json-schema-library"` 深度绑定 9.x API，11.x 无 Draft04/Draft07 构造函数——无法统一，保留双库（本模块 11.6.2 直接依赖 + codemirror-json-schema 9.3.5 嵌套副本）。

## 文件

| 文件 | 职责 |
|------|------|
| `schema/claude-code-settings.json` | SchemaStore 官方 settings.json schema（2026-08-01 快照，升级时整文件替换） |
| `schema/index.ts` | schema 单点：`claudeCodeSettings`（完整 schema）+ `SCHEMA_ID`（$id 指纹）+ `hooksSubSchema`（hooks 子树提取）+ `JsonDiagnostic`/`JsonValidationResult` 类型 + `validateHooksJson`（json-schema-library 11.x `compileSchema` draft-07 校验，TE-09） |

## 测试模式

L2 测试位于 `src/__tests__/hooks-config-schema.test.ts`（HKC-08，用例数见 `.claude/test-inventory.md`）：直测 `validateHooksJson` 边界——合法 / 缺 hooks 键 / 非法 matcher / 未知事件告警。面板层测试见 @../../panels/CLAUDE.md hooksConfig 节。
