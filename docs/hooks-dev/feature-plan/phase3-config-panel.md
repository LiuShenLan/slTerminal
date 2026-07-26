# 阶段 3：配置管理

> F6 一项功能，体量最大，单独成阶段。
> 标注约定：**【已确认】** = 用户问答拍板；**【推导默认】** = 已确认决策的直接推论，开发前可复核。

## F6 hooks 双模式配置面板

### 功能目标

图形化编辑 Claude Code 的 hooks 配置（settings.json `hooks` 字段），替代手写 JSON：30+ 事件名、5 种 handler 字段、matcher 正则语法不再靠背，写错当场提示而非运行时静默失效。

### 调研依据

- 配置三层嵌套（事件 → matcher 组 → handler 数组），5 种 handler 字段各异（D2 §1/§3）
- 官方 JSON Schema 已存在：SchemaStore `claude-code-settings.json`（D2 §9.1）
- VS Code settings 双模式编辑（GUI 表单 + JSON）是最适合参照（D2 §10.4）；JetBrains Master-Detail + 启停 checkbox（D2 §11.4）
- hooks 改动需重启 claude 会话生效（D5b §3.4 教训 #3）

### UI 形态

新 Dockview 面板类型（硬约束 #5：`panels/<newtype>/` 目录 → `panelRegistry.ts` 注册 → `PANEL_TYPES` 追加），顶部切换 `GUI 表单 | JSON` 两模式，实时同步编辑同一份配置【已确认 Q9】。

**双模式同步规则**【推导默认】：

- 任一模式的合法编辑立即反映到另一模式
- JSON 模式内容**非法时禁止切换**到 GUI 模式并提示先修复——防止表单视图以残缺理解覆盖用户数据
- 两模式共享同一份打开中的文件与脏状态，保存动作唯一

### 编辑对象：三层配置【已确认 Q12】

| 层级 | 路径 | 作用域 | 优先级 |
|------|------|--------|--------|
| local | `<项目>/.claude/settings.local.json` | 单项目个人（应 gitignore） | 最高 |
| project | `<项目>/.claude/settings.json` | 单项目（可提交 git 共享） | 中 |
| user | `~/.claude/settings.json` | 所有项目 | 最低 |

- 面板顶部层级切换器，标注上述优先级关系
- 仅切换编辑，**不支持**跨层复制/移动【已确认 Q12】
- 文件不存在时按需创建（含父目录）【推导默认】
- managed（企业策略）层不展示、不可编辑【推导默认——企业管控内容第三方工具不应碰】

### JSON 模式

- CM6 编辑器 + JSON Schema 补全/校验（悬停文档、错误波浪线、属性自动补全）
- Schema 来源：SchemaStore 官方 schema **内嵌打包**进 slTerminal【推导默认：离线可用；版本随 slTerminal 发布更新；D2 §9.5 已知 schema 同步滞后问题可接受】
- 事件类型导航侧栏：30+ 事件按九大分组（会话生命周期/用户交互/工具调用/权限系统/通知/Agent 子代理/上下文压缩/环境变更/MCP 交互，D1 §1.1），点击跳转到对应 JSON 段落
- **matcher 实时测试工具**：输入 matcher 模式 + 工具名，即时显示命中/不命中及走了哪条匹配路径

**matcher 语义**（测试工具必须严格按此实现，D2 §4.1）：

| 模式 | 判定 | 示例 |
|------|------|------|
| 仅含字母/数字/`_`/`-`/空格/`\|`/`,` | 精确匹配，`\|` 和 `,` 为 OR | `Edit\|Write` |
| 含其他字符 | JavaScript 正则（非锚定） | `^mcp__` |
| `"*"` / `""` / 省略 | 匹配全部 | — |
| 大小写 | **敏感**——`"bash"` 不匹配 `Bash` | — |
| FileChanged / StopFailure | 窄字符集（仅字母/数字/`_`/`\|`），其他字符强制走正则 | — |

### GUI 表单模式

**布局**：Master-Detail（JetBrains External Tools 模式）——左 = 事件分组列表（事件 → matcher 组 → handler 三级树），右 = 选中 handler 的专用表单 + 启停 checkbox。

**5 种 handler 字段表单**（D2 §3，必填项标 \*）：

| 类型 | 字段 |
|------|------|
| `command` | `command`\*、`args[]`、`async`、`asyncRewake`、`shell`、`timeout`、`if`、`allowedEnvVars[]` |
| `http` | `url`\*、`method`、`headers{}`、`body`、`timeout`、`allowedEnvVars[]` |
| `mcp_tool` | `server`\*、`tool`\*、`args{}`、`timeout` |
| `prompt` | `prompt`\*、`timeout` |
| `agent` | `prompt`\*、`description`、`subagent_type`、`model`（sonnet/opus/haiku/fable）、`timeout` |

**事件 → handler 支持矩阵**（D1 §1.2，表单必须约束可选类型）：

| 事件 | 支持的 handler 类型 |
|------|--------------------|
| Notification、SessionEnd、PreCompact、PostCompact | command、http、mcp_tool |
| SessionStart、Setup | command、mcp_tool |
| 其余全部 | 5 种全支持 |

**单条启停开关**【已确认 Q13，详见 ADR-0002】：

- 禁用状态存 slTerminal 侧 settings（`~/.slterminal/settings.json`），保存时从用户 settings.json 中剔除被禁用条目
- 禁用条目按（层级 + 事件 + matcher + command）四元组标识
- 用户文件被外部修改导致四元组失配时，该禁用记录在 UI 标记为「失效的禁用记录」，不静默丢弃
- GUI 显眼位置常驻提示：「禁用条目由 slTerminal 托管，不出现在配置文件中」

### 保存安全【已确认 Q10】

1. 保存前 JSON 语法 + Schema 双重校验，**不通过拒绝保存**（配置永远不处于损坏状态）
2. 原子写入（临时文件 + rename），写一半断电不留坏文件
3. 保存成功后提示条：「hooks 改动需重启 claude 会话生效」
4. 不做 .bak 备份（校验保证写不坏 + 编辑器有撤销）

### 与 F2 的关系

一键注入/卸载按钮**并入本面板**（阶段 1 的临时入口迁移至此；面板内同时显示当前注入状态：已注入/未注入/版本过旧）【推导默认】。

### 明确不做

- 社区模板库（一键插入预设 hook）【已确认排除】
- 跨层复制/移动【已确认 Q12】
- 四层级合并可视化（managed/local/project/user 生效视图）【已确认排除，D2 标高难度】
- YAML 模式、Import/Export、宏变量选择器（D2 P3 项，本轮不纳入）
- 编辑 `hooks` 之外的 settings.json 其他字段（permissions、env 等）【推导默认——本面板专精 hooks，其他字段用户用普通编辑器】

### 验收要点

1. JSON 模式：输入 `"Pre` 自动补全 `PreToolUse`；字段名写错出波浪线；matcher 测试 `Edit|Write` + `Edit` 显示命中（精确匹配路径）
2. GUI 模式：新建 PreToolUse + Bash matcher + command handler → 保存 → settings.json 出现正确三层嵌套结构；Notification 事件的类型下拉不含 prompt/agent
3. 双模式：GUI 新增一条 → 切 JSON 可见对应段落；JSON 改非法 → 禁止切回 GUI
4. 三层切换：user 层写入的配置与 project 层互不干扰，优先级标注可见
5. 启停：禁用一条 hook → settings.json 中该条目消失 + slTerminal 侧记录存在 + UI 提示可见；重新启用 → 条目回到原位置
6. 保存安全：改坏 JSON 保存被拒；保存成功出现"需重启 claude 会话生效"提示
7. F2 并入：面板内可完成注入/卸载，注入状态可见

## 阶段 3 验收（端到端）

1. 上述验收要点全部通过
2. 真实 claude 验证：GUI 配置一条 PreToolUse 拦截 hook → 重启 claude → 拦截真实生效（端到端证明配置写对了）
3. L2/L3 测试按项目测试策略补齐（表单↔JSON 双向同步、Schema 校验、启停往返）
