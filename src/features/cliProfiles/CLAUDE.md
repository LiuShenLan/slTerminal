# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

CLI profile 注册表（MC-1/101~108）——编码 CLI 身份域与能力策略的单点。claude 为首个 profile（MC-2：全部现有行为经 profile 驱动且零回归）。三处消费：

- **身份域**（id/displayName/commands/iconSrc/tabTitle）：OSC 133 C 命中经 `matchByCommand` 取页签标题；品牌 logo 经 `agentSession.cliId` 查 iconSrc（F9）。
- **hooks 能力**：事件→四态映射 / 通知类别判定 / computeUsagePercent / restartHint / configEditor / configLayers。
- **history 能力**：恢复命令与恢复注入内容构造。

能力域**可选**：未声明 = 该域不可用，消费方优雅降级。通用层禁止写 "claude" 字面量（AC-5 守卫），一律经注册表查询或 import `profiles/claude` 导出常量。

## 关键约束与决策

### 模块级单例

`cliProfileRegistry` 全局单例（`CliProfileRegistry` 类实例）：

- `register(profile)`：注册（同 id 覆盖旧条目，注册序不变）
- `get(id)`：按 cliId 精确查询
- `getAll()`：全部 profile，按注册序
- `matchByCommand(commandLine)`：首 token 匹配，未命中返回 null
- `_reset()`：清空全部（仅测试）

### 首 token 解析单点化（MC-102）

`trim().split(/\s+/)[0]` 全仓唯一实现——`matchByCommand` 对 `profile.commands` 逐键精确查表，覆盖带参变体；空命令行/仅空白 → null；**不 toLowerCase**；同首 token 多 profile 冲突时先注册者优先。

### profile 接口契约（`types.ts`，spec 00 §3.1）

- `CodingCliProfile`：id / displayName / commands / iconSrc / tabTitle / capabilities。
- `HooksCapability`：eventToStatus / classifyNotification / computeUsagePercent / restartHint / hasConfigEditor / configEditor / configLayers。
  - `computeUsagePercent`：用量信号 → 显示百分比（claude = 官方 `usedPercentage` 取整 + 钳位 0–100）。
  - `configEditor`（KZ-1）：hub 配置编辑器组件；`hasConfigEditor=true` 时必填。
  - `configLayers`（KZ-4）：hooks 配置分层声明；`hasConfigEditor=true` 时必填。
- `HistoryCapability`：supportsFork / buildResumeCommand / buildRestoreInput。

能力**可选**：`capabilities.hooks` / `capabilities.history` 均可缺省。

### CLAUDE_CLI_ID 常量约定（MC-205，AC-5 守卫豁免形态）

`profiles/claude/index.ts` 导出 `CLAUDE_CLI_ID = "claude"`——通用层缺省回退一律 import 本常量，禁止写 "claude" 字面量。同文件另导出 `CONTEXT_USAGE_EVENT` / `SESSION_START_EVENT` / `SESSION_END_EVENT` / `EXIT_EVENT`：claude 事件名字面量只允许出现在 `profiles/claude/`，通用层消费一律 import 常量。

### claude 合法领地（MC-213/223 前端半）

`profiles/claude/` 是 claude 身份域与 hooks/history 策略实现点——claude 字面量（id/命令名/事件名/`~/.claude` 路径）只允许出现在此目录；通用层经注册表/常量消费。

### 编辑器归域 `configEditor/`（KZ-1，F11 收编）

claude 专属 hooks 编辑器（ClaudeHooksConfigEditor + 10 文件 + schema/）**归域 `profiles/claude/configEditor/`**——不再位于 panels、不再跨模块引用（F11 迁移后 cliProfiles 零外部面板依赖）。`profiles/claude/index.ts` import `./configEditor/ClaudeHooksConfigEditor` 挂入 `capabilities.hooks.configEditor`；设置中心 Hooks 配置页（`panels/settings/pages/HooksSettingsPage`）经该字段分派渲染，hub 零直接引用（KZ-1 依赖方向不变，只是编辑器资产物理归域）。`types.ts` 仅类型 import，运行期擦除，不构成运行循环。

**schema 单点（MC-223/P3-FE-07/TE-09/TE-15）**：`configEditor/schema/` 承载 SchemaStore 官方 claude-code-settings schema 内嵌 + hooks 子 schema 提取（`properties.hooks` + 依赖 `$defs` 子集，不含 permissions 专用 permissionRule）+ Draft07 校验单例。协议知识只属于 claude profile 域，**不抽象**为通用能力——面板选择行允许其他 CLI 挂载自有编辑器，但本 schema 单点仍是 claude 专属资产。

- 升级方式：整文件替换 `configEditor/schema/claude-code-settings.json` 即可，离线可用、无网络请求（自包含性已核实：无远程 `$ref`，35 个本地 `$ref` 全指向 `#/$defs/*`）。
- `compileSchema(schema, { draft: "draft-07" })` 单例（json-schema-library 11.x）——schema 固定不变，复用避免重复编译；本 schema 无 `$schema` 字段，缺省会选 draft-2020-12，**必须显式 `draft-07` 保持旧语义**（TE-09）。
- **TE-15 债务登记（ADR-0010）**：json-schema-library 9.x/11.x 双 major 并存——codemirror-json-schema@0.8.1 锁 9.x（上游约束），主声明 11.6.2；运行时两实例并存无冲突，待上游升级消解。

### 注册触发点（side-effect import）

- barrel `index.ts` **不触发注册**（仅导出类型与注册表）。
- `profiles/index.ts` import 即注册全部 profile——生产注册触发点为 `Workspace.tsx` 显式 import（D-07）；新增 CLI 在此追加 `import "./<cli>"`。

## 测试模式

L2 测试位于 `src/__tests__/`：

- `cli-profile-registry.test.ts`：注册表行为全分支 + logo 资源守卫（MC-108 泛化：遍历注册表断言 iconSrc 磁盘存在 + PNG 魔数）。
- `cli-profile-claude.test.ts`：claude 身份域字段 + 常量一致性 + side-effect 注册 + hooks/history 策略输出。
- `mock-cli-profile.test.tsx`：AC-4 mock profile 全链路验收（OSC 133 命中 / hooks 能力真实调用 / 历史聚合 UI / hub 选择行 / 恢复注入）。
- `no-claude-literals.test.ts`：AC-5 字面量守卫（通用层七路径扫描 "claude" 字符串/事件名/`~/.claude` 路径零残留）。

## 新增 CLI 步骤

1. `public/cli-icons/<id>.png` 放图（32×32 透明底，渲染 16×16）。
2. `profiles/<cli>/` 定义 profile（含能力域；`hasConfigEditor=true` 时须同时声明 `configEditor` 组件与 `configLayers`）。
3. `profiles/index.ts` 追加 import。
4. **后端 hooks provider**：实现 `CliHooksProvider` trait 并在 `src-tauri/src/hooks/provider.rs` 的 `REGISTRY` 注册 cliId 条目（无 hooks 能力 → 注册 `None` 条目）。
5. **后端 history provider**：实现 `CliHistoryProvider` trait 并在 `src-tauri/src/agent_history/provider.rs` 的 `REGISTRY` 注册 cliId 条目（delete 前必过 `validate_session_id`，SEC-05 等价）。
6. 对应 cli-profile 测试登记字段与策略用例 + `.claude/test-exemptions.md` 同步。
