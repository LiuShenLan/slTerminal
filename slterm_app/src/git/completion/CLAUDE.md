# Git 补全只读面

> 经 `GitFacade` 提供本地 refs/remotes、跟踪推导与有界代次缓存；不暴露通用 argv。

## 决策与红线

- **语义入口唯一**：消费者只调用 `GitFacade::refs`／`remotes`；命令名、参数、配置哨兵与环境护栏只能由本模块组装。原因：通用 argv 会绕过只读与禁网边界。
- **双预算共享**：一次 refs 读取的配置与引用命令共用 3 秒总时限和 1 MiB 输出上限；remotes 使用同一上限。启动、取消、超时、退出码、超量与 UTF-8 失败保持结构化分类，补全来源可将任一失败降级为空来源。
- **本地只读**：固定 `--no-optional-locks`、`--literal-pathspecs`、`GIT_TERMINAL_PROMPT=0`、`GIT_NO_LAZY_FETCH=1` 与空 `GIT_ALLOW_PROTOCOL`；不得执行网络或写命令。配置只读取 checkout 与 refspec 推导所需键，`completion.snapshot=true` 保证空配置仍是成功结果。
- **快照代次**：facade 随补全 pane 持有 2 秒 TTL 缓存；失效递增代次并清槽，旧请求不得回填。pane 环境在创建时冻结，因此缓存键只含读取类型、cwd 与目录上下文。

## 验收与测试

- 命令测试固定 argv、禁网环境与共享剩余额度；真实临时仓库覆盖 refs、remotes、refspec、worktree、缓存失效与目录上下文。
- 进程测试覆盖取消后收尸、真实超时与输出上限；facade 测试分别识别启动、取消、超时、退出码与超量。
