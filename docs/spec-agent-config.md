# 需求规格：CLI 配置中心化重构（Agent 全局文件视图 + 设置中心 Agent 组）

- 版本：v1（2026-09-26）
- **本文档为临时文档**：开发完成并提交 git 后即删除。`CONTEXT.md`、各级 `CLAUDE.md`、代码注释**禁止引用**本文档路径（防死引用）；「§9 文档影响清单」仅列待同步点，不回链。
- 原则：未来最优——不考虑现状实现限制与历史包袱；breaking change 允许。

## 1. 背景与目标

同一编码 CLI 的配置当前散落在设置中心不同组（全局组「CLI 别名」、项目组「Hooks 配置」），且 Hooks 配置页内嵌 CLI 选择行为多 CLI 预留——归属混乱。本需求将配置按「全局 / Agent」二分重构，并新增「Agent 全局文件」侧栏视图，使每个编码 CLI 的配置与全局文件集中于一处。

目标：

1. 新增侧栏视图「Agent 全局文件」：agent 节点（图标+名称）→ 展开为该 agent 全局配置目录的文件浏览器，功能与现有「文件浏览器」完全一致；展示内容可配置。
2. 设置中心组模型改为「全局 / Agent」，删除「项目」组；CLI 别名迁入 claude 的「基础配置」，Hooks 配置迁入 claude 子页并删除 CLI 选择行。
3. 全程高内聚、低耦合、易扩展：文件树能力单点复用、agent 差异化知识全部归 profile 领地、注册表家族扩展。

## 2. 术语变更（CONTEXT.md 登记项，实现时同步）

| 变更 | 内容 |
|------|------|
| 新增别名 | UI 文案「Agent」= 领域术语「编码 CLI」（Coding CLI / CLI profile 的 UI 呈现名）。领域术语不改，仅 UI 层采用 Agent。 |
| 显示名 | claude profile `displayName`：`"claude"` → `"Claude Code"`（`tabTitle` 保持 `"claude"` 不变；别名校验消息自动跟随）。 |
| 删除 | 「项目组」（设置中心组模型不再有项目组）。 |
| 新术语 | 「Agent 全局文件」（侧栏视图）；「配置节」（设置页内以标题划分的配置块，通用约定）；「运行时文件」（agent profile 用 `runtimePaths` 声明的全局目录下的运行时产物——缓存/会话/日志等，无配置价值，默认不展示）。 |

## 3. 范围

**In**：Agent 全局文件侧栏视图（含展示内容过滤）；设置中心组模型与导航重构；claude「基础配置」页（两配置节）；Hooks 配置页迁移；配套存储段与 IPC 边界；文档同步。

**Out（非目标）**：

- 设置中心脱离项目宿主（无项目可用化）——维持现状：无项目 → toast「请先创建项目」。
- agent 动态插件化——agent 集为编码期硬编码注册。
- Hooks 编辑器自身功能变更；快捷键/后台定时任务/终端输入模式页的功能变更（仅做配置节组件对齐）。
- 注入/卸载按钮（已在 Hooks 配置页工具栏）与套餐余量频率（后台定时任务页）——位置不动。

## 4. 功能需求

### F1 「Agent 全局文件」侧栏视图

**F1.1 注册、归属与图标**

- 视图 id `agentFiles`，标题「Agent 全局文件」，经 `sideViewDefs.ts` 注册一行接入（硬约束 #13 家族契约）。
- 默认归属 top 区末尾（nav / explorer / commit 之后）；存量用户经 R9 注册表对齐自动追加，默认不打开。
- **活动栏图标选型定案**：`IconAgentFiles` = lucide `Bot`，追加至 `src/lib/icons.tsx` 单点（IC-01），继承 `makeIcon` 统一规格（15px / 1.5px 描边 / currentColor），零新增资产文件。选型理由：活动栏已有 Folder（文件浏览器）、FolderTree（导航树）两个文件夹系图标，第三个 Folder* 系辨识度差；`Bot` 直表「Agent」语义，与视图标题组合语义完整，风格与既有活动栏图标完全一致。

**F1.2 agent 节点层**

- 第一层 = agent 节点：`profile.iconSrc`（16×16）+ `displayName`，按 profile 注册序排列。
- 多根树：各 agent 节点独立展开/折叠，可同时展开多个。
- agent 节点仅作展开容器：无右键菜单、不参与 CRUD/文件快捷键。
- 节点集 = 声明了 `globalFiles` 能力的 profile 集（能力可选，未声明不出现，优雅降级）。
- 惰性加载：agent 节点展开时才首次 `readDir` 其根目录。

**F1.3 文件树（复用文件浏览器全量能力）**

- 展开 agent 节点 = 该 agent 全局配置目录的文件树，能力与「文件浏览器」完全一致：新建/删除/重命名（右键菜单 + F2/Del 快捷键）、双击经 FileViewerRegistry 分派打开面板、git 状态着色、fs 事件增量刷新（200ms debounce + FE-15 已知路径子树刷新）、虚拟化滚动、横向滚动、展开态跨挂载持久化。
- 全局目录路径：profile 声明**相对用户主目录**的路径（claude = `.claude`，字面量仅允许出现在 `profiles/claude/`，AC-5），后端经 `crate::home` 解析为绝对路径——天然吸收用户名差异；L1 测试复用 HomeDirGuard 指向 tempdir。
- **CRUD 全量开放**，含 `settings.json` 等核心文件，不设特殊保护。
- 双击打开文件需 Dockview 宿主：无项目 → toast「请先创建项目」（与配置钮同语义）；有项目 → 照 explorer 分派到当前活跃页。
- git 着色不做特殊处理：`~/.claude` 非 git 仓库时 `gitStatus` 返回空、自然无色。
- `.credentials.json` 等凭据文件在「全部」模式下可见可打开，不做特殊处理（SEC-18 仅约束 git 追踪文件）。
- symlink 不特殊处理：跟随/监听行为与 explorer 现状一致（`~/.claude` 常被软链到 dotfiles 仓库，属合法用法）。
- 快捷键焦点语义：agent 视图与 explorer 同开时，`explorer.*` 快捷键经 activeExplorer 指针派发给最后聚焦实例（现状语义自洽，不新增 context）。

**F1.4 展示内容过滤（与 F3.1 配置联动）**

- 模式二选一：**全部** / **自定义**。
- 全部：展示全局目录根层全部条目；叠加「显示运行时文件」开关（见 F1.5）。
- 自定义：仅展示根层与名单**完全同名**的文件/文件夹；名单即完整真值——不受运行时开关影响，名单显式列出 `projects` 即展示并监听。
- 匹配规则：
  - 大小写**不敏感**（与 NTFS 语义一致）；
  - 仅匹配根层——不递归、不匹配子路径；
  - 匹配上的目录，内部完整递归展示（过滤仅作用于根层）；
  - 名单中磁盘不存在的项 → 静默忽略；
  - 名单校验：去重 + 拒空白 + 拒含 `\` `/`（名称非路径）；chips 按添加序展示。
- 自定义 + 空名单 → 空态文案「未配置展示项」。
- 「不读取」语义：未匹配/被排除的目录不递归 `readDir`（根层 `readDir` 仍执行一次以枚举候选）。
- 配置变更（模式/名单/开关）→ 视图经 store 订阅即时刷新，树重建走 `reloadPreservingExpanded` 语义（保留未受影响展开态）。
- 解耦声明：视图过滤仅作用于展示，不影响 history provider 对 `~/.claude/projects/` 的既有扫描链路。

**F1.5 「显示运行时文件」开关**

- 语义 = 「展示 + 监听」：勾选 → 运行时条目正常展示并纳入事件刷新（200ms debounce + FE-15 局部刷新）；不勾选 → **整项不读取、不展示、不监听**（根层即排除，目录节点本身不出现）。
- 排除规则：名单项为**根层名称**（文件/目录同规则），完全同名（大小写不敏感）命中即整项排除——与自定义名单同一匹配语义。
- claude 的 `runtimePaths` 初始清单（11 项，官方运行时产物）：`projects`、`sessions`、`shell-snapshots`、`history.jsonl`、`telemetry`、`stats-cache.json`、`ide`、`paste-cache`、`cache`、`file-history`、`session-env`。
  - 保留展示（来源/价值不确定或属配置资产，宁缺毋滥）：`backups`、`daemon*`、`jobs`、`tasks`、`plans`、`agent-memory`、`plugins`、`agents`、`skills`、`CLAUDE.md`、`settings.json` 等。
  - 清单为 profile 单点常量（`profiles/claude/` 领地，AC-5），随 claude 版本演进补齐；未来 agent 各自声明。
- 开关仅「全部」模式生效；「自定义」模式下 UI 禁用置灰并附说明，**值持久保留**（切回全部模式恢复原勾选态）。
- 默认值 = **不勾选**（默认呈现干净配置目录）。
- 实现注：watcher 为目录级注册、无法排除子树——不勾选时命中条目的变更事件在事件处理层丢弃（性能观察点登记）。

**F1.6 状态持久化与监听生命周期**

- 视图跨挂载状态经 sideViewRegistry 状态槽（照 CP-016 先例）：各 agent 节点展开态 + 各已展开 agent 文件树的 expandedPaths（以其 rootPath 为域键）。
- 监听惰性化：仅已展开的 agent 节点启动其目录监听，折叠即停；折叠期间的外部变更在再展开重新 `readDir` 时自然补齐。

**F1.7 空态与错误**

- 无已注册 agent（防御分支，当前不发生）→ 空态占位。
- agent 全局目录不存在（未安装该 CLI）→ 该节点下占位「目录不存在」；运行时消失同此占位，重现经重展开/重载恢复。
- 根层 readDir 失败 → 错误占位（照 explorer 首帧失败先例）。

### F2 设置中心结构重构

**F2.1 组模型**

- `SettingsPage.group` 值集 → `"global" | "agent"`；组序 全局 → Agent；「项目」组删除。
- 全局组保留不变：快捷键 / 后台定时任务 / 终端输入模式；「CLI 别名」页从全局组移除。
- 无项目 → 设置中心整体不可达（toast）的定位不变。
- 壳其余机制不变：dirty 汇聚与导航圆点、params 持久化单点、SC-FE-08 切项目自动关闭、corrupted 警示条。
- Agent 组空态：无已注册 agent 时 Agent 组整体不渲染（防御分支）。

**F2.2 导航形态**

- Agent 组内：agent 行 = 组内分节标题（图标 + 名称，**不可点击、无折叠**），其下缩进平铺该 agent 的子配置页。

**F2.3 子配置页声明机制**

- 「基础配置」= 框架内建必有页，按 cliId 参数化渲染——每注册一个 agent 自动出现。
- 「Hooks 配置」= 能力可选：profile `capabilities.hooks.hasConfigEditor === true` 才挂载。
- 未来 agent 专属配置页 = profile 声明注入，框架零改动。

**F2.4 配置节通用约定**

- 抽共享「配置节」组件：节 = 节标题 + 内容块，样式蓝本 = KeybindingsPage 分组标题形态；固化为设置页通用划分方式。
- 存量页（快捷键等）同步切换至共享组件——同源防漂移。

**F2.5 页 id 与深链**

- Agent 子页 id 采用层级命名空间：`agent.<cliId>.<page>`（`agent.claude.basic` / `agent.claude.hooks`）；全局组页 id 不变。
- 旧页 id（`cliAliases` / `hooks`）废弃；settings 面板 params.selectedPage 失效值 → 静默回退首组首页。

### F3 claude「基础配置」页

页 = 配置节序列；框架内建两节（所有 agent 的基础配置页都含此两节），未来 agent 专属节经 profile 声明注入。

**F3.1 配置节：Agent 全局文件侧栏展示内容**

- 「全部 / 自定义」单选 + 自定义名单编辑器（chips 添加/删除，交互照 CLI 别名页先例：Enter/按钮提交、行内红字校验、blur 保留草稿）+「全部」模式下「显示运行时文件（projects 等）」checkbox（默认不勾选；自定义模式禁用置灰）。
- 立即生效：store 变更 → 视图实时响应 + debounce 落盘（照前端消费型配置先例）。

**F3.2 配置节：CLI 别名**

- 功能与现「CLI 别名」页相同，改为单 agent 形态（仅本 cliId：内置命令只读 chips + 别名 chips + 添加行）。
- 校验 `validateCliAlias` 语义零变化——D3 全命名空间唯一**已含跨 agent 冲突检测**（接口已就绪）。
- 存储段 `cliAliases`（cliId → 别名数组）不变，存量数据无迁移。

### F4 Hooks 配置页迁移

- 迁至 Agent 组 claude 节点下，页 id `agent.claude.hooks`。
- 删除 CLI 选择行及其配套代码（选择行渲染、selectedCli 页参数、切 CLI dirty 守卫）；页直接渲染 claude profile 的 `configEditor`。
- 编辑器三层（user/project/local）保留不变（`configLayers` 声明驱动，与设置中心分组无关）；dirty 守卫、注入工具栏、重启提示等编辑器内部行为不变。
- `data-e2e` 选择器语义重订，E2E 用例适配并登记 wdio specs。

## 5. 数据与存储

- settings.json 新增段 `agentGlobalFiles`（建议名）：

```json
{
  "agentGlobalFiles": {
    "claude": { "mode": "all", "customNames": [], "showRuntimeFiles": false }
  }
}
```

  - `mode`: `"all" | "custom"`；`customNames`: 字符串数组；`showRuntimeFiles`: 运行时文件展示+监听开关（默认 `false`，自定义模式下值保留仅 UI 禁用）。缺省起步、无迁移。
  - 加载 sanitize：段键过滤未注册 cliId（与 R9 注册表对齐哲学一致）；段缺失/类型错误回退默认。
- DTO 单源（硬约束 #4）：Rust `#[derive(TS)]` → ts-rs 导出 `src/types/`；前端消费型配置（`save_settings` 段写 + store debounce 落盘）。
- profile 能力扩展：`capabilities.globalFiles = { configDir: ".claude", runtimePaths: [...] }`（可选能力；路径字面量仅 `profiles/claude/`）。
- `cliAliases` 段不变。

## 6. 架构约束

1. **文件树单点复用**：文件树能力下沉为共享组件（入参 = rootPath + 根层过滤器 + 能力开关）；「文件浏览器」与「Agent 全局文件」各为薄壳——同一逻辑不维护两处。**红线：explorer 视图行为零回归**。
2. **沙箱边界**：agent 全局目录位于项目根沙箱之外——新增 IPC 域按 profile 声明目录做白名单校验（读取/CRUD/监听同边界），禁止任意路径穿透。
3. **注册表家族**：视图注册（sideViewDefs）/ 设置页注册（pages.ts）/ profile 注册三处各一条，框架零改动（硬约束 #13）。
4. **平台与测试**：路径解析经 `crate::home`；L1 复用 HomeDirGuard；过滤/名单校验为纯函数全量 L2 覆盖；L4 覆盖关键路径（视图展开、双击打开、设置页迁移、过滤联动）。
5. 术语与字面量纪律：通用层禁写 `"claude"` / `~/.claude` 字面量（AC-5），一律经注册表/profile 常量消费。

## 7. 兼容与迁移

- 无数据迁移：`cliAliases` 存量保留；`agentGlobalFiles` 缺省起步。
- sideBar 持久化：R9 注册表对齐自动追加 `agentFiles` 至 top 区末尾，无失效项。
- settings 面板 params.selectedPage 旧 id 失效 → 回退首组首页。
- E2E：settings/hooks 相关选择器与 specs 登记同步适配。

## 8. 验收标准（关键场景 checklist）

1. 侧栏出现「Agent 全局文件」视图（活动栏图标 = Bot，风格与既有一致），claude 节点可展开为 `~/.claude` 文件树；CRUD/双击打开/git 无色降级/增量刷新与文件浏览器一致。
2. 无项目时双击文件 → toast「请先创建项目」。
3. 「全部」+ 运行时开关不勾选（默认）→ `projects` 等 11 项运行时不出现；勾选 → 出现且变更实时刷新；「自定义」→ 仅名单同名根层条目（大小写不敏感），名单含 `projects` 时不受开关影响；开关勾选态跨模式切换保留。
4. 设置中心 = 全局（快捷键/后台定时任务/终端输入模式）+ Agent（Claude Code → 基础配置 / Hooks 配置）两组；无「项目」组；无注册 agent 时 Agent 组不渲染。
5. 基础配置页两配置节可用；别名跨 agent 唯一性校验保持（构造冲突别名验证错误消息指向冲突方 displayName）。
6. Hooks 配置页无 CLI 选择行，三层切换/注入工具栏/dirty 守卫行为不变。
7. explorer 视图行为零回归（现有测试全绿）。
8. 静态门禁全绿：tsc / eslint / clippy / rustfmt；L1/L2 全量串行回归。

## 9. 文档影响清单（实现时同步；禁止回链本文档）

- `CONTEXT.md`：§2 术语变更全部登记。
- `src/features/settingsCenter/CLAUDE.md`（组模型/导航形态/页 id 命名空间）、`src/features/sideViews/CLAUDE.md`（新视图）、`src/features/explorer/CLAUDE.md`（文件树共享组件化）、`src/features/cliProfiles/CLAUDE.md`（globalFiles 能力、displayName 变更）、`src/panels/CLAUDE.md`（settings 壳）、`src/lib/CLAUDE.md`（IconAgentFiles 选型登记）同步。
- ADR 建议：组模型重构（全局/Agent 二分）与 agent 目录 IPC 白名单边界——实现立项时各登记一条（难逆转 + 有真实权衡）。
