# Hooks 编辑、注入与进程桥

> 规定 Hooks 单一能力目录、草稿与子树保存、受管注入事务，以及独立命名管道事件桥。

## 决策与红线

### 能力目录与适配器

- `slterm_hooks` 拥有事件目录、handler schema、子树编辑模型、托管识别和 CLI 适配器声明；application HookService 独占授权、锁、备份、原子写和注入执行。
- 保留 Claude、Codex、OpenCode、Cursor、Kimi、Pi、OhMyPi、Copilot、Grok 的注入适配；GUI／JSON 三层编辑只由明确声明的 Claude 提供，其他 CLI 不获得隐式编辑界面。
- 事件目录是 GUI、JSON schema、保存校验和注入识别的唯一来源。handler 类型封闭为 command、http、mcp_tool、prompt、agent；事件声明可用类型、matcher 能力和字段。
- Claude 的 User、Project、Local 层级按 Local > Project > User 解释；User 编辑用户级文件，Project／Local 绑定 Settings 所属项目根，切活跃项目不改变面板所属项目。
- 目录中的事件与字段覆盖 featureSpec 声明的完整集合；未知但 schema 合法的字段原样保留，未声明 CLI 的既有设置命名空间不因页面未显示或恢复默认而删除。

### 草稿与 JSON 子树

- 草稿按“面板实例＋CLI＋层级”隔离，持有原始文件基线指纹、hooks 子树模型、JSON 文本和诊断。草稿不持久化，关闭、切层、外部重载、项目移除和根重定位都经保存／丢弃／取消守卫。
- 默认 JSON，可切 GUI；两种模式编辑同一草稿。JSON 或 schema 非法时保留最后合法模型、禁用 GUI 与保存，显示首条诊断和完整诊断入口。
- 解析器必须检测重复键；重复键、坏 JSON、非法 UTF-8、权限和锁失败都拒绝，不当空配置。文件不存在或没有 hooks 才按空配置进入编辑。
- 保存只替换 hooks 子树：写前重新读取完整文件、比较基线指纹、取得操作锁并创建上一代有效备份，再以子树字节范围替换保留其他字段和字节内容。父目录按需创建，目标原子写并保留 ACL。
- 外部修改、锁冲突、备份失败或写失败均保留草稿和重试，不自动合并或覆盖。成功后清除草稿并提示重启对应 CLI 会话生效。
- 含 slTerminal 托管标识的 handler 在 GUI 中只读，包含它的 matcher 与事件不得经 GUI 删除；普通编辑不能静默移除托管内容。

### 注入与卸载事务

- 注入可安装 slTerminal 托管 handler、脚本和状态行桥接；卸载只移除托管内容，保留同组用户 handler，并按备份恢复或移除状态行桥接。
- 注入／卸载以用户级设置文件提交为边界，使用 ManagedOperationJournal 记录准备材料、文件基线、前后指纹和清理步骤：提交前失败回滚准备材料；提交后清理失败保留已提交事实并提供重试，不回滚成虚假未注入。
- 目标 JSON 损坏时卸载失败并保留脚本与信号材料，避免配置残留而执行体已删除。完成后刷新状态并重读 User；已有草稿时先走外部修改守卫。
- 用户原有可疑下载或任意执行命令命中时先暂停并显示确认条，未确认前设置文件零写入。命令原文只在确认界面显示，不进入日志或诊断。
- 注入状态区分查询中、已注入、未注入、版本过旧和查询失败；失败提供重试，不把未知显示成未注入。注入只影响之后启动的 CLI 会话。

### hook 小进程与事件桥

- `slterm_hook` 保持零生产依赖、独立小进程和有界 stdin；每条路径 exit 0，单条输入上限 1 MiB、处理上限 2 秒。不得因 hook 输入错误让 CLI 失败或泄露正文。
- Hooks 事件继续走独立 Windows 命名管道，通过 `SLTERM_NOTIFY_PIPE` 传递端点；不并入 Runtime JSON-RPC，不在普通端口文件写 token。
- application HookEventService 校验事件来源、AgentKind、TerminalId、PTY 代次和事件代次，更新 Agent 状态与标题；隐藏视图仍接收事件，但不抢焦。
- 事件载荷按白名单解析，正文、命令和可疑内容不进入日志；非法或过期事件只丢弃并产生受限健康计数，不伪造 CLI 状态。
- Hooks 开关与调度配置进入 SettingsRegistry，不再依赖旧 AssistantConfig；恢复默认不修改外部 CLI 文件或托管注入事实。

## 验收与测试

- 能力目录测试覆盖事件、matcher、handler 类型、GUI 可用性和托管识别；GUI、JSON schema 与保存校验必须从同一目录生成。
- JSON 测试覆盖重复键、未知合法字段、外部修改、字节保留、锁冲突、备份失败、ACL 失败和子树边界；任何失败不得覆盖原文件。
- 草稿测试覆盖三层隔离、切页／切层／根重定位／关闭守卫、非法模型禁用保存和无项目浮层 User 限定。
- 注入故障注入覆盖准备、用户配置提交、脚本清理、卸载损坏 JSON 和提交后清理失败；结果必须区分已提交与未提交。
- 真实命名管道测试覆盖多条事件、超大输入、畸形 JSON、超时、进程退出和每条路径 exit 0；不泄露命令正文。
- Hooks 事件与 Agent 状态测试验证隐藏视图仍接收、旧 PTY 代次丢弃和 Runtime 管道完全隔离。

## 操作

- 新增 CLI Hooks 适配器先声明文件目标、schema、托管识别和注入恢复器，再开放 UI 或 Runtime。
- 修改托管格式必须同步备份、卸载、状态查询和崩溃对账；不得只修改注入脚本。
- 所有 Hooks 写入经 HookService 的受限 PathPolicy 能力，不能由文件目标派生目录浏览、任意写入或终端授权。
