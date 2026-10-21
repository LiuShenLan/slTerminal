# AI Providers 与助手迁移

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

> 职责：设计 AI Providers、AI 助手的应用页入口、命令修复、凭据域、终端/编辑器集成边界，以及测试与豁免。

### Providers

- Provider 元数据并入应用设置域（漫游 `<data>/state/settings.json` 的 app 段），带 schema 版本；只含 provider ID、显示元数据与 hint，不含明文，也不含 `SecretId` 或任何凭据引用值。
  ```rust
  struct ProviderRecord {
      id: ProviderId,                 // "preset-openai" | "custom-<n>"
      kind: ProviderKind,             // 内置预设全集 + OpenAI 兼容
      name: String,
      note: String,
      website_url: String,
      base_url: String,
      model: String,
      enabled: bool,
      full_url: bool,
      key_hint: Option<String>,       // "••••abcd" 尾四掩码，仅用于识别
  }
  struct ProviderStoreV2 {
      schema_version: u32,
      active: Option<ProviderId>,
      providers: Vec<ProviderRecord>,
  }
  ```
- 凭据 target 由 provider ID 单点派生为小写 `slterm/ai/<id>`；`credential_present` 不持久化，读取时按 provider ID 实时查询 Windows 凭据管理器（`SecretStore::exists`），避免引用位与凭据库漂移。`SecretId` 与任何凭据引用值都不进入 provider 元数据、设置 DTO、备份或快照。预置 ID 稳定；旧 `Slterm/AI/<id>` 凭据按迁移映射读取一次并写回新 target，随后删除旧 target。
- 预置 provider 全集由 `ProviderKind::PRESETS` 单点定义；加载时补全缺失预置、保留自定义记录、修复无效 active。自定义 ID 只在创建时分配，不复用。
- 元数据清洗沿用单点：名称/URL/模型去控制字符与首尾空白，长度上限为契约值；两个呈现入口共用同一清洗函数。
- 保存：临时文件 + 原子替换；损坏文件回退默认预置并记录警告，不隔离（无凭据值）。删除 provider 顺序固定为“定位 → 删除凭据 → 删元数据 → 原子落盘”；凭据删除失败不得改动元数据。

### 凭据域

- 凭据域契约类型为 `SecretStore`/`SecretValue`（见 `../security.md`）；现有 `CredentialStore`/`SecretBytes`/`WindowsCredentialStore` 作为后端实现保留，迁移时收敛为契约类型的适配层；业务模块不得旁路 `platform::credentials`。
- 凭据只存 Windows 凭据管理器；provider 文件、设置、备份、日志、错误消息、崩溃快照均不得出现明文。
- 凭据读写只以 provider ID 派生 target；不向 DTO、设置、备份、快照或页面状态暴露 `SecretId` 或凭据引用值。
- 写入顺序：先写凭据，成功后更新 `key_hint` 与内存状态；失败保留旧元数据与旧 hint。
- 读取只在请求执行器内发生；UI 只拿 `exists` 与 hint。`SecretValue` 的 `Debug` 只报长度；转换字符串必须走 `Zeroizing`。
- 环境变量兜底取消：目标态 provider 是唯一凭据来源；无启用 provider 或无凭据时返回可操作错误，不再回退 `SLTERM_AI_KEY`/`OPENAI_API_KEY`。避免凭据来源双轨，也避免终端子进程环境成为隐式凭据通道。
- 凭据值不进入备份、导入导出、Runtime 快照、通知、审计字段；应用设置域中的敏感键导出剥除与导入复验继续作为第二道闸。

### AI 助手

- 助手配置从独立 `slterm_assistant.txt` 迁入应用设置，只存 provider ID 与行为开关：
  - `ai.assistant.enabled`：默认关。
  - `ai.assistant.provider`：只存 provider ID；空 = 当前 active provider。
  - `ai.assistant.ignored_exit_codes`：不触发修复的退出码列表。
- 助手不持久化 `SecretId`、凭据引用值、base URL 或 model；这些由所选 provider 提供，凭据状态按 provider ID 实时查询。
- 触发门链保持：每面板冷却（5 秒）→ 开关 → 规则表。规则表排除成功、用户中止码、忽略码、空命令、帮助屏、裸工具、交互式程序；失败边（hook 自愈失败）走同一门链，默认关。
- 请求上下文：项目/面板 ID、命令、退出码、cwd、分支、输出尾部。输出尾部从终端网格抓取，最多 24 行/2000 字符，跳过宽字符占位，去首尾空行；出境前经密钥打码（连续 40+ base64/hex 串替换为 `[redacted]`）。
- 请求目标组装：启用 provider 优先；`requires_api_key` 时经凭据域读取；协议族按 provider kind 分派（OpenAI 兼容、Anthropic、Google、Azure）；全局 30 秒超时；头拼接串使用 `Zeroizing`。请求失败只写日志，不打断终端。
- 响应解析：取首个 `{` 至末个 `}`；`command` 必须非空且单行；`danger` = 模型标志 OR 本地危险词表。空 command 视为刻意沉默。
- 建议状态机：`Pending {seq}` → `Ready {seq, fix}` / `Failed {seq, message}`；响应按面板 + seq 落位，过期丢弃。`Ctrl+.` 只贴入不执行；`Esc` 关闭；可打印输入撤条；Pending 时按 `Ctrl+.` 不丢请求。
- 助手回答投影：保留 `AssistantAnswer` 三态（完整、缺失、超大）与 `AnswerInbox` 的 provider/session/pane 归属；目标归属键改为面板 ID，超限报尺寸不截断。回答原文不写日志、不进备份、不落盘。

### 应用页入口与所有权

- 应用页“AI Providers 与助手”分节只放入口与显示：控件、展示字段与交互归 `settings/app-page.md`；Provider/助手数据模型与凭据边界归本节，页面不重复定义字段或默认值。
- 页面不因 Provider 未注册而丢失未知记录；未知 kind 记录保留并只读显示。
- 全局配置作用于全部项目与面板；项目设置中心不重复一份 Provider 配置。

### 终端与编辑器集成

- 终端面板实现完整 AI 修复条：命令完成事件进入触发链；建议条覆盖在终端底部，不修改网格；接受时把命令写入输入通道但不追加回车。AI 修复条开关关闭或助手关闭时不注册按键、不发起请求、不显示条。
- 终端面板同时是 `AgentPaneHost` 的宿主：hook 事件按面板 ID 路由，维护 agent 状态、回答收件箱与会话索引；通知服务消费 TurnDone/NeedsAttention 状态。
- 编辑器集成边界：目标态不新增编辑器 AI 命令或内联补全。编辑器只共享全局 provider 配置与 `AiGateway` 接口；保留 `EditorAssist` 扩展枚举但默认不注册命令、不进入快捷键页、不产生请求。若后续需要编辑器 AI，另立规格，不在本次迁移中暗加。
- 终端与编辑器共用同一 `AiGateway`，不得各自读取应用设置域或凭据库；Provider 测试与修复请求走同一请求器。

### 测试与验收

- Provider：预置全集完整性、ID 稳定、加载补全、损坏回退、原子写、删除顺序、凭据失败不动元数据、序列化无明文且无 `SecretId`/凭据引用值。
- 凭据：门面三原语、`SecretValue` Debug/字符串化、日志与错误不泄露、源码扫描禁止旁路后端；真实凭据管理器用例默认跳过，显式环境变量开启。
- 助手：门链顺序与冷却、规则表、打码、危险词、请求目标四种协议、响应解析、seq 过期、按键语义、失败边默认关。
- 回答：归属校验、会话切换、迟到丢弃、超限不截断、Debug 不含正文。
- 应用页：只写不读、hint 更新、测试结果、未知 CLI/Provider 防御、自动保存失败提示。
- 豁免登记：不可自动化面按 `.claude/docs/testing/exemptions.md` 登记，并用 mock 服务、假凭据与人工冒烟兜底。

### 依赖里程碑

里程碑依赖只在 `plan.md` 的“里程碑”表定义；本文件不复制依赖列表。Runtime/通知只消费 agent 状态，不反向依赖 AI 服务。

### 迁移顺序

1. 将 Provider 元数据并入应用设置域，改为存在性驱动凭据状态；凭据 target 迁到 `slterm/ai/<id>`。
2. 助手配置迁入应用设置；删除旧 txt 配置、旧 `.bak`/corruption 路径。
3. 建立 `AiGateway`，统一 provider 解析、请求、测试与错误映射。
4. 应用页 AI 分节接入；完成 Provider 编辑、凭据、测试与助手开关。
5. 终端 AI 修复条改 `PanelId` 与 `CommandFinished` 事件；回答收件箱改面板 ID。
6. 删除旧配置读取入口、环境变量兜底、旧设置入口与旧测试。

### 不可自动化面

- 真实模型回答质量、危险命令判定准确率与语言一致性。
- 真实 provider 的认证、配额、网络策略与代理行为。
- Windows 凭据管理器的真实交互与无凭据库服务账户。
- 真实 AI CLI 的 hook 事件时序与会话文件形态。
