# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

CLI profile 注册表（MC-1/101~108）——编码 CLI 身份域与能力策略的单点。claude 为首个 profile（MC-2：全部现有行为经 profile 驱动且零回归）。三处消费：

- **身份域**（id/displayName/commands/iconSrc/tabTitle）：OSC 133 C 命中（`useCommandDetection`）经 `matchByCommand` 取页签标题与品牌 logo（MC-105）
- **hooks 能力**（HooksCapability）：事件→四态映射 / 通知类别判定 / contextLimit / restartHint / hasConfigEditor——`useXterm`、agentStatus、notifications 经 profile 委托消费（MC-403/410/412/420）
- **history 能力**（HistoryCapability）：恢复命令与恢复注入内容构造——agentHistory 右键菜单/恢复编排经 profile 委托消费（MC-315/316）

能力域**可选**：未声明 = 该域不可用，消费方优雅降级（MC-1）。通用层禁止写 "claude" 字面量（AC-5 守卫），一律经注册表查询或 import `profiles/claude` 导出常量。

## 架构决策

### 模块级单例（项目第 6 个注册表，照 TabTitleRegistry 模式）

`cliProfileRegistry` 为全局单例（`CliProfileRegistry` 类实例）：

- `register(profile)`：注册（同 id 覆盖旧条目，注册序不变）
- `get(id)`：按 cliId 精确查询，未注册返回 undefined
- `getAll()`：全部 profile，按注册序
- `matchByCommand(commandLine)`：首 token 匹配，未命中返回 null
- `_reset()`：清空全部（仅测试）

### 首 token 解析单点化（MC-102）

`trim().split(/\s+/)[0]` **全仓唯一实现**（原 cliIcons.ts / TabTitleRegistry.ts 两份拷贝收敛于此）——`matchByCommand` 对 profile.commands 逐键精确查表，覆盖 `claude --resume` / `claude -p` 等带参变体；空命令行/仅空白 → null；**不 toLowerCase**（大小写敏感）；同首 token 多 profile 冲突时先注册者优先。

### profile 接口契约（types.ts，跨边界契约 spec 00 §3.1）

| 类型 | 字段 | 说明 |
|------|------|------|
| `CodingCliProfile` | id / displayName / commands / iconSrc / tabTitle / capabilities | cliId 公共键（如 "claude"）；commands 支持多首 token（如 `["claude","cc"]`）；iconSrc 品牌 logo 根绝对路径（如 `/cli-icons/claude.png`） |
| `HooksCapability` | eventToStatus / classifyNotification / contextLimit / restartHint / hasConfigEditor / configEditor / configLayers | hooks 能力域——协议知识实现留在 `profiles/<cli>/`，本文件仅签名；`configEditor`（KZ-1）：hub 配置编辑器组件（`HooksConfigEditorProps` 泛化自 ClaudeHooksConfigEditorProps），**hasConfigEditor=true 时必填**，缺失 → hub 编辑器槽空态占位防御；`configLayers`（KZ-4）：hooks 配置分层声明（`{ id, label, hint }[]`——编辑器层切换器数据源），**hasConfigEditor=true 时必填**，claude = user/project/local 三层现值 |
| `HistoryCapability` | supportsFork / buildResumeCommand / buildRestoreInput | history 能力域——历史会话恢复策略 |

能力**可选**：`capabilities.hooks` / `capabilities.history` 均可缺省——未声明 = 该域不可用，消费方优雅降级（无 hooks 能力 profile 的事件行不建/不通知/不置图标；supportsFork 缺省 false 不展示「分支恢复」菜单；恢复编排对无 history 能力 profile 防御性失败 toast）。

### CLAUDE_CLI_ID 常量约定（MC-205，AC-5 守卫豁免形态）

`profiles/claude/index.ts` 导出 `CLAUDE_CLI_ID = "claude"`——通用层缺省回退（MC-205 三级解析第三级、旧数据无 cliId 的兼容分支）一律 import 本常量，**禁止写 "claude" 字面量**。同文件另导出会话生命周期事件名常量 `SESSION_END_EVENT` / `EXIT_EVENT`：claude 事件名字面量只允许出现在 `profiles/claude/`（claude 合法领地），通用层消费一律 import 常量。

### claude 合法领地（MC-213/223 前端半）

`profiles/claude/` 是 claude 身份域与 hooks/history 策略实现点——claude 字面量（id/命令名/事件名字面量/`~/.claude` 路径）只允许出现在此目录；通用层经注册表/常量消费。策略实现（`strategies.ts`）迁自 lib 层状态映射（MC-401 前端半）、notifications 类别判定（MC-422）与 agentHistory 恢复策略（MC-315/316），行为零改动（AQ-1 例外：`buildResumeCommand` cwd 单引号按 PowerShell 规则转义为 `''`）。

### features→panels 依赖方向合法化（KZ-1）

`profiles/claude/index.ts` import `panels/hooksConfig/ClaudeHooksConfigEditor` 并挂入 `capabilities.hooks.configEditor`——features/cliProfiles → panels 新方向合法化理由：`profiles/claude/` 是 claude 合法领地，编辑器组件是 claude 专属资产（MC-223 决策 2），hub 经 profile 的 configEditor 字段分派渲染（新增 CLI 自带编辑器组件即可接入 hub，hub 零改动）；`types.ts` 仅类型 import（`HooksConfigEditorProps` 的 `React.ComponentType`/`React.MutableRefObject` 类型引用，运行期擦除）不构成运行循环——打包图循环由 `npx vite build` 验证。

### 注册触发点（side-effect import，照 tabRules 先例）

- barrel `index.ts` **不触发注册**（仅导出类型与注册表；缺省回退常量经 `./profiles/claude` 直接引用）
- `profiles/index.ts` import 即注册全部 profile——生产注册触发点为 `Workspace.tsx` 显式 import（D-07）；新增 CLI 在此追加 `import "./<cli>"`，不修改核心逻辑

## 文件

| 文件 | 职责 |
|------|------|
| `types.ts` | profile 接口契约：`CodingCliProfile` + `HooksCapability`（含 `HooksConfigEditorProps`/`configEditor`，KZ-1）+ `HistoryCapability`（跨边界契约，spec 00 §3.1） |
| `cliProfileRegistry.ts` | `CliProfileRegistry` 类 + `cliProfileRegistry` 全局单例（register/get/getAll/matchByCommand/_reset） |
| `index.ts` | barrel export（类型 + 注册表），不触发注册 |
| `profiles/index.ts` | profile 注册触发点（side-effect import，生产入口 Workspace.tsx） |
| `profiles/claude/index.ts` | claude 身份域定义 + `CLAUDE_CLI_ID` / `SESSION_END_EVENT` / `EXIT_EVENT` 常量 + hooks/history 能力挂载（side-effect 注册）；`configEditor` 挂载 claude 专属编辑器 `panels/hooksConfig/ClaudeHooksConfigEditor`（KZ-1，依赖方向合法化见上）+ `configLayers` 声明 user/project/local 三层（KZ-4，值 + 文案迁自编辑器退役 LAYERS 常量） |
| `profiles/claude/strategies.ts` | claude hooks/history 策略实现：`eventToStatus`（10 事件映射）/ `classifyNotification`（五映射）/ `buildResumeCommand` / `buildRestoreInput`（输出与迁出源逐字一致；差异点 = cwd 单引号转义为 `''`（AQ-1）） |

## 测试模式

L2 测试位于 `src/__tests__/`（用例数见 `.claude/test-inventory.md`）：

| 文件 | 覆盖范围 |
|------|---------|
| `cli-profile-registry.test.ts` | 注册表行为全分支（register/get/getAll 注册序/同 id 覆盖/matchByCommand 多 commands/带参变体/空命令行/仅空白/未命中/不 toLowerCase/同键冲突先注册者优先/_reset/独立实例/全局单例）+ **logo 资源守卫**（MC-108 泛化：遍历注册表全部 profile 断言 iconSrc 磁盘存在 + PNG 魔数——img 404 无报错通道，资源缺失靠此守卫；含 mockcli.png 先行资源，Stage 07 mock 夹具引用） |
| `cli-profile-claude.test.ts` | claude 身份域字段（MC-104）+ CLAUDE_CLI_ID 常量一致性 + side-effect 注册 + hooks 能力字段（MC-214 前端半）+ history 能力字段（MC-315/316）；hooks 策略用例（eventToStatus 26 用例迁自 claude-status.test.ts、classifyNotification 五映射迁自 notifications.test.ts）；history 策略用例（输出与迁出源逐字一致——断言漂移即实现有误；差异点 = cwd 单引号转义为 `''`（AQ-1），含回归用例） |
| `mock-cli-profile.test.tsx` | AC-4 mock profile 全链路验收（Stage 07 五点全表：OSC 133 命中 / hooks 能力真实调用 / 历史聚合 UI / hub 选择行 / 恢复注入），夹具 `helpers/mockCliProfile.ts` |
| `no-claude-literals.test.ts` | **AC-5 字面量守卫**：通用层七路径（src/lib、src/panels/terminal、features/agentStatus、features/agentHistory、features/notifications、src/ipc、src/types）递归扫描——"claude" 字符串/事件名字面量/`~/.claude` 路径字面量零残留（豁免：指向 profiles/claude 的 import 路径、标识符与注释） |

**新增 CLI 步骤**：`public/cli-icons/<id>.png` 放图（32×32 透明底，渲染 16×16，随 frontendDist 内嵌 exe）→ `profiles/<cli>/` 定义 profile（含能力域，实现随 claude 先例）→ `profiles/index.ts` 追加 import → 对应 cli-profile 测试登记字段与策略用例。
