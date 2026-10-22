# AI Providers 与助手迁移

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

> 职责：只登记 AI Providers/助手的迁移增量：旧代码与旧配置删除、落点、迁移顺序、增量验证与不可自动化面。Provider 元数据、助手配置与开关归 `../settings/app-page.md`；凭据域归 `../security.md`；AI 修复条与终端增强行为归 `../panels/terminal.md`。本篇不重复定义目标态契约。

### Providers

- 删除旧 `providers.json` 读取与写入；Provider 元数据并入应用设置域，DTO、字段、默认值与加载规则由 `../settings/app-page.md` 定义。
- 旧 provider 记录不导入；预置 provider 由新加载规则补全，自定义记录按新 ID 重建。旧 ID 到新 ID 的映射只存在于迁移代码，不进入设置 schema。
- 凭据 target 迁移：按 `../../spec-issues.md` 的凭据 target 裁决写入新 target；写入后经 `../security.md` 定义的 `SecretStore::exists` 校验可读，成功再删除旧 target。任一步失败保留旧 target 并在应用页提示，不删除元数据。
- `credential_present` 不持久化，按 provider ID 实时查询凭据管理器。
- 旧 `provider_test.rs` 与旧测试入口并入 `AiGateway` 测试；不保留第二请求器或第二凭据读取通道。
- 迁移输入、日志、错误与测试夹具不得包含明文凭据；旧配置文件不读不删，凭据按显式迁移步骤读取一次。

### 凭据域

- 旧 `CredentialStore`/`SecretBytes`/`WindowsCredentialStore` 收敛为 `../security.md` 定义的 `SecretStore`/`SecretValue` 适配层；业务模块不得旁路 `platform::credentials`。
- `SecretValue` 的 `Debug`、字符串化与错误脱敏契约以 `../security.md` 为准；迁移代码不得新增长度、前缀或哈希输出。
- 删除环境变量兜底 `SLTERM_AI_KEY`/`OPENAI_API_KEY`；迁移后无启用 provider 或无凭据时返回可操作错误。
- 凭据值不进入设置、备份、日志、错误、Runtime 快照或迁移测试夹具；测试只用假值。

### AI 助手

- 删除 `slterm_assistant.txt`、`.bak` 与损坏副本读取；助手配置迁入应用设置域，键名、默认值与作用域由 `../settings/app-page.md` 定义。
- 旧配置中的 provider 标识按新 provider ID 映射；无法映射的记录丢弃并提示重新配置，不保留旧文件解析路径。
- 旧助手请求、回答收件箱与修复状态机保留实现，归属键从 `PaneId` 改为 `PanelId`；目标行为由 `../panels/terminal.md` 与 `../settings/app-page.md` 定义。
- 删除旧回答归属中的 pane 字段与旧会话文件读取；回答正文不写日志、不进备份、不落盘。
- 迁移后 `AiGateway` 为唯一请求入口；Provider 测试与修复请求共用，不保留旧直连通道。

### 应用页入口与所有权

- 应用页“AI Providers 与助手”分节的控件、字段、默认值与保存模式归 `../settings/app-page.md`；本篇只登记旧入口删除与新入口接线。
- 删除旧设置入口、旧 provider/assistant 页面与旧快捷键；新分节迁移前保持禁用或占位，迁移完成后开放。
- 未知 provider kind 的兼容显示由 `../settings/app-page.md` 定义；迁移验证旧记录不会导致页面崩溃。

### 终端与编辑器集成

- 终端 AI 修复条的触发、按键、建议状态与开关归 `../panels/terminal.md`；本篇只登记从旧 pane 接线迁到面板 host 的删除与改接。
- 删除旧 `agent_host`/`ai_sessions`/`ai_agents` 的 `PaneId` 路由与旧 hook 字段；改接 `PanelId` 后由终端面板消费。
- 编辑器不新增 AI 命令、内联补全或请求路径；删除旧编辑器 AI 接线与快捷键注册。

### 测试与验收

- 迁移：旧 `providers.json`/`slterm_assistant.txt` 不再被读取；旧凭据 target 迁移成功与失败分支；旧设置入口与旧 pane 接线不存在。
- 凭据：迁移全程无明文落盘；`Debug`、错误与日志脱敏按 `../security.md`；真实凭据管理器用例默认跳过并登记豁免。
- 助手：`PanelId` 归属、迟到丢弃、旧会话文件不读；关闭开关后无请求。
- 应用页接线：占位或禁用态在迁移前可见，迁移后入口可用；未知 provider 不崩溃。
- 豁免登记：不可自动化面按 `.claude/docs/testing/exemptions.md` 登记。

### 依赖里程碑

里程碑依赖只在 `plan.md` 的“里程碑”表定义；本文件不复制依赖列表。Runtime/通知只消费 agent 状态，不反向依赖 AI 服务。

### 迁移顺序

1. 应用设置域接管 Provider 元数据与助手配置；删除旧 `providers.json`/`slterm_assistant.txt` 写路径。
2. 凭据 target 迁移：先写新 target、校验可读、再删旧 target；失败保留旧 target 并提示。
3. 建立 `AiGateway` 唯一请求器，收敛 Provider 测试与修复请求。
4. 应用页 AI 分节接线，去掉占位或禁用态。
5. 终端 AI 修复条改 `PanelId` 与 `CommandFinished` 事件；回答收件箱改面板 ID。
6. 删除旧配置读取、环境变量兜底、旧设置入口、旧 pane 接线与旧测试。

### 不可自动化面

- 真实模型回答质量、危险命令判定准确率与语言一致性。
- 真实 provider 的认证、配额、网络策略与代理行为。
- Windows 凭据管理器的真实交互与无凭据库服务账户。
- 真实 AI CLI 的 hook 事件时序与会话文件形态。
