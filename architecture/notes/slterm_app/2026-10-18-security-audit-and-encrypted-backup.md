# 审计事件格式、备份边界与 plan_balance 推送裁定

## Status

已执行（2026-10-18，M10.9–M10.12 实现裁定回登）。当前生效规则见 `.claude/adr.md` ADR-0034 与 ADR-0027。

## Context

凭据、审计与备份三类面跨发布长期存续，且事故不可逆：类型层泄露、日志插值、备份错纳敏感文件都会把凭据带出域。设计稿的备份类别与敏感键表基于旧键域，必须按新设置键域重定；审计事件字面格式此前未定，实现期取了字段形态。

## Evidence

- 审计通道实现 = `tracing` target `audit`，`slterm_app/Cargo.toml` 启用 `log-always`；生产不装全局 subscriber，事件无条件桥接进既有 `log` 栈。测试用作用域 `tracing-subscriber` 捕获（`slterm_app` dev-dependency）。
- 设置写事件字段：`operation` / `path` / `source` / `result`；SEC-13 对账额外带 `expected` / `actual`（SHA-256 hex 指纹）。不记录文件内容或凭据值。
- 备份格式实测：`TRMBK001 | salt(16) | nonce(12) | ciphertext`，魔数作 AES-256-GCM AAD；Argon2id v0x13（m=19456 KiB、t=2、p=1）；口令最少 8 字符。加密依赖解析到 aes-gcm 0.11.1 / argon2 0.6.0（设计稿写的 0.11.0/0.6.0-rc.8 是上游基线值）。
- 备份类别实现 = 六类：settings / session / themes / command_history / directory_history / providers；默认仅 settings。
- 敏感键表实现 = 22 项（`access_token`、`api_key`、`api-key`、`apikey`、`authorization`、`client_secret`、`credential(s)`、`password`、`passwd`、`private_key`、`refresh_token`、`secret`、`session_token`、`token` 等），去首尾空白后 ASCII 小写精确匹配；导出剥除 + 导入复验双闸。`api_key_set`/`api_key_hint` 是引用与尾四掩码，不在剥除列。
- 凭据接口实现差异：`load_generic_secret` 返回 `Option<SecretBytes>`（设计稿写 `Option<Vec<u8>>`）；`store_with_username` 拒绝空 secret 是新增语义。
- plan_balance 推送实现 = 模块级 `SNAPSHOT` + watch 语义订阅者列表；`current_snapshot()`/`subscribe()` 读值，有界 1 槽合并连续推送。
- 设计输入失实订正：上游 baseline `e537d528` 无 plan_balance 模块；行为真值源实为 slTerminal 旧栈 `src-tauri/src/plan_balance/`（c59f47f^ 终态）。

## Decision

- 审计事件沿用 `operation/path/source/result` 与 `expected/actual` 字段形态；新增事件必须复用该形态，禁自由文本插值。
- 备份类别六类与敏感键 22 项为本版边界；类别清单的默认选择保守（仅 settings）。
- 恢复序固定为「完整鉴权 → 全量校验 → 落盘」；任一条不过整单拒写，半份不可信报文零副作用。
- 备份 UI 入口归设置页安全分区（D10-4）；引擎与恢复点撤回先落，UI 接线属后续步骤。
- plan_balance 使用模块级 watch channel，多消费者各自订阅并读最新快照；订阅后先读 `current_snapshot()` 以覆盖「落槽后、扇出前」的漏唤醒窗口。
- `SecretBytes::as_bytes` 借出仅限登记白名单调用点；新增调用点先回安全域登记。

## Rejected alternatives

- 独立审计日志文件/第二 subscriber：双栈重复落点、初始化时序复杂；`log-always` 单栈最稳。
- 备份遍历 home 目录：白名单即安全边界，枚举即授权。
- 敏感键 denylist 只做导出：导入侧手改文档可绕过；双闸是必要纵深。
- `Context::notify` 直推 plan_balance：耦合组件树，无法服务未来 HUD/托盘等多消费者。

## Consequences

- 新增设置/凭据字段必须检查敏感键表漂移，并补导出剥除/导入复验用例。
- 备份类别/敏感键表变更是恢复边界变更，须评审并补往返用例。
- 审计写事件在设置高频写路径（如主题 live 预览）会产生 warn 级噪声（A16 已登记）；分级需产品裁决。
- 若未来装配全局 tracing subscriber，需防审计事件双落点（B67）。

## Validation

- `seal_open_round_trips_every_category`、`restore_to_writes_every_category_and_packet_restore_round_trips` 与敏感键剥除/复验用例覆盖备份双闸。
- `logs_never_contain_credential_plaintext` 同时断言 tracing 捕获与 tracing→log 桥接。
- `plan_balance_reads_user_settings_not_the_credential_store` 源码扫描锁死双通道隔离。
- `watch_channel_fans_out_to_multiple_subscribers` 覆盖多消费者推送。

## Supersedes

None。

## Revisit when

- 设置键域新增类别（备份类别/敏感键表需重定）或出现新的凭据载体。
- 装配全局 tracing subscriber（B67 双落点风险）。
- 备份 UI/恢复点跨重启清单接线（D10-4 落地）。
- plan_balance 订阅 API 改为「订阅即返回值」时，可删除先读快照的消费纪律。
