# Runtime 控制面

> 本机 loopback 控制面与 CLI 双人格；外部契约以随包 schema 与 `runtime.describe` 为准。

## 决策与红线

- **Windows 子系统 = console**：同一 `slterm.exe` 双人格——带子命令 = CLI 客户端，无子命令 = GUI。原因：单文件发布与标准流正确性优先。边界：从 pwsh/cmd 直接启动 GUI 会阻塞父 shell；快捷方式与发布说明必须告知日常启动用快捷方式或 `Start-Process slterm`。
- **控制面协议**：本机 `127.0.0.1` TCP，JSON Lines；信封 `{"protocol":"slterm.runtime","version":1,...}`，请求 `deny_unknown_fields`，响应按 `ok/result/error` 互斥校验。原因：外部客户端需要稳定、可校验的契约。
- **鉴权与限额**：token 为 128 位随机 hex，缺失/不符静默丢弃（先于参数校验）；请求体上限 128 KiB；单实例最多 64 个并发客户端；读写超时 5 秒，控制派发 30 秒（`pane.exec` 按参数追加预算）。原因：本机控制面仍需防误用与资源耗尽。边界：契约数值变更须评审并同步 schema。
- **端点发现**：进程内 `CHILD_ENDPOINT` > 显式 `SLTERM_RUNTIME_ENDPOINT`（子进程经环境继承）> 数据目录 `runtime.port`；显式端点非法时不回落端口文件，避免选错实例。
- **契约数值**：等待上限 600 秒、短超时 30 秒、`RUN_START_GRACE` 3 秒、订阅有界 16、请求/响应上限与截断值等以 schema 与 `runtime.describe` 为准；发布后即成外部契约。
- **单窗收编**：单窗行为硬编码，`window.*` 只作用于当前唯一窗口。
- **legacy ATTACH 默认关闭**：首行非 JSON 默认静默丢弃；`legacy-attach` 编译 feature 开启后才接受 `ATTACH/PING`，token 不符仍静默丢弃。单实例移交与存活探测走版本化 JSON：第二实例发 `window.focus` + `tab.new`。原因：首发收缩审查面。边界：旧客户端接入诉求出现时再评估启用。
- **强类型单请求编排**：模型只提交一次意图，Runtime 在本地完成确定性控制步骤；不得把多步控制逻辑推给客户端。
- **随包事实文档**：`assets/runtime-control-api.md` 与 `assets/runtime-api-v1.schema.json` 是随包契约真值源；实现能力以 `runtime.describe` 为准。新增方法/错误码须同步 schema、describe 与文档。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 必测面：信封校验、token 静默丢弃、限额与超时、端点发现优先级、单实例移交、legacy ATTACH 默认拒绝、`runtime.describe` 与 schema 一致。
- 真实 GUI 进程与双进程移交由 `runtime_conformance` 条件跳过用例兜底，见 `.claude/docs/testing/exemptions.md`。

## 操作

- 新增方法：同步 schema、`runtime.describe` 与文档；补本地服务用例。
- 新增错误码：沿用现有域分类，不自由扩展字符串。
- 调试：优先用 `slterm ctl` / `slterm env` 与 `runtime.describe`。
