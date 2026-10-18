# slTerminal Runtime API

本文件是随包分发的协议事实文档;机器可读定义以同目录
[`runtime-api-v1.schema.json`](runtime-api-v1.schema.json) 为唯一真值源,
实现与能力枚举以运行中的 `runtime.describe` 响应为准。

## 传输与发现

- 控制面 = 本机 `127.0.0.1` 上的 TCP 服务,JSON Lines:首行一条请求,
  响应/事件各一行。
- token 是 128 位随机值,随端点一起经环境变量或端口文件下发;token
  不符的请求静默丢弃,不返回解析错误(未认证进程不得把错误信息当协议
  预言机)。
- 端点发现优先级:`SLTERM_PANE_ID` 所在的嵌套进程经
  `SLTERM_RUNTIME_ENDPOINT`(内容 `<port> <token>`)> 应用数据目录下的
  `runtime.port`(内容 `<port> <token> <protocol_version>`)。显式端点
  非法时不回落端口文件,避免选错实例。
- 请求体上限 128 KiB;单实例最多 64 个并发客户端;单次读写超时 5 秒,
  控制命令派发超时 30 秒(`pane.exec` 按其自身参数追加预算)。

## 信封

请求:

```json
{"protocol":"slterm.runtime","version":1,"id":"<pid>-<seq>","token":"<token>","method":"runtime.describe","params":{}}
```

响应:

```json
{"protocol":"slterm.runtime","version":1,"id":"<pid>-<seq>","ok":true,"result":{}}
```

失败响应用 `ok:false` + `error:{code,message,details?}`;`result:null` 与
缺省 `result` 是两种不同语义。请求信封拒绝未知字段
(`deny_unknown_fields`);响应身份按 `protocol`/`version`/`id` 与
`ok`/`result`/`error` 互斥逐字段校验。

`protocol` 固定 `slterm.runtime`;首发 `version = 1`,不支持其他版本时
返回 `protocol_version_mismatch` 并在 `details.supported_versions` 列出
支持版本。

## 方法与能力

`runtime.describe` 返回 `app_version`、`protocol`、`protocol_version`、
`supported_versions`、`schema`(相对安装根的
`assets/runtime-api-v1.schema.json`)、`capabilities`、`features`、`env`
与 `commands`。客户端必须查 `capabilities` 判断方法是否可用,查
`features` 判断附加参数语义(如 `pane.wait.after_seq`),不得静默假设。

方法族:

- `runtime.describe` / `runtime.snapshot` / `runtime.orchestrate`
- `events.subscribe`
- `agents.list` / `agent.start` / `agent.fork` / `agent.get` /
  `agent.delegate` / `agent.prompt` / `agent.paste` / `agent.read` /
  `agent.wait`
- `window.close` / `window.focus`
- `tab.new` / `tab.close` / `tab.rename` / `tab.move`
- `pane.split` / `pane.close` / `pane.zoom` / `pane.resize` /
  `pane.prompt` / `pane.paste` / `pane.read` / `pane.procs` /
  `pane.send_key` / `pane.run` / `pane.exec` / `pane.wait`
- `git.status` / `git.diff` / `git.rollback` / `git.unstage`

单窗口单实例定位:`window.*` 只作用于当前唯一窗口。

## 错误码

- 信封:`invalid_request` / `protocol_version_mismatch` /
  `request_too_large` / `method_not_found` / `invalid_params` /
  `target_not_found` / `ambiguous_target` / `invalid_state` /
  `action_failed`
- 生命周期:`pane_closed` / `pane_exited` / `agent_exited` /
  `agent_replaced` / `agent_closed` / `agent_name_conflict` /
  `agent_identity_mismatch`
- agent 启动:`agent_launch_unsupported` / `agent_resume_unsupported`
- 执行:`runtime_unavailable` / `runtime_timeout` /
  `invalid_runtime_response` / `submission_outcome_unknown` / `timeout`
- run:`exit_code_unavailable` / `run_start_timeout` / `run_aborted`
- git:`git_stale` / `git_unavailable` / `invalid_base` /
  `invalid_branch` / `dirty_source` / `branch_conflict` /
  `worktree_path_conflict`
- 委派:`origin_not_agent` / `origin_identity_unavailable` /
  `delegation_in_progress` / `delegation_capacity` /
  `agent_ready_timeout`
- 编排:`invalid_reference`

输入类方法(`pane.prompt`/`pane.paste`/`agent.prompt`/`agent.paste`/
`agent.delegate`)在传输或解析失败时返回 `submission_outcome_unknown`:
输入可能已送达,先读目标状态再决定是否重试,不盲目重复投递。

`agent.start`/`agent.fork` 的可冷启动 kind 集合与 schema 的
`$defs.launch_agent_kind.enum` 完全一致(单一真值源,不在此复制清单);
kind 已登记但未核实冷启动命令时返回 `agent_launch_unsupported`,未知
kind 仍返回 `invalid_params`。

## CLI 入口

同一 `slterm.exe` 兼具 GUI 与 CLI 人格:带子命令 = 客户端,无子命令 =
GUI。`slterm env` 离线报告身份、端点可达性与完整命令清单;`slterm ctl`
暴露完整协议;`window`/`tab`/`pane`/`agent` 资源动词是同一信封上的薄
别名,`--pretty` 只改排版。

## 兼容说明

首发默认不接受非 JSON 首行(`ATTACH`/`PING` 形式的旧式回落协议编译期
关闭);单实例移交与存活探测均走上述版本化 JSON 信封。旧客户端接入
诉求出现时再评估启用。
