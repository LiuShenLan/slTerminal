# 补全应用层

## 决策与红线

- **补全不上按键热路径**(08 篇边界 9):按键路径只更新输入快照与缓存键;
  候选在后台 `Request::calculate` 计算。任何新来源都不得在按键路径做磁盘/
  子进程 I/O。
- **来源缓存跟随 pane**:`Session` 持有 Git/scripts 缓存,以 PaneId 关联;
  界面不决定来源适用条件。提交命令事件后必须 `Session::invalidate()`。
- **取消令牌全链路**:文件遍历、历史扫描、Git 子进程统一为 `Cancellation`
  (`Arc<AtomicBool>`);取消后不得回填缓存或呈现部分结果。
- **源收敛单态**:远程/SSH/WSL 来源与执行环境整支不迁;`SuggestEnv` 只有
  Local。新增远程能力时先改 08 篇设计,不得在本模块私开分支。
- **执行元数据只描述本机**:`metadata::Execution` 的 `prepare()` 是本地直通;
  Git 读取必须经 07 篇 `git::command` 构造口(`--no-optional-locks`),
  不得另起执行体。

## 步骤接缝

- M9.1 落引擎 + 应用层;ghost/popup 呈现、按键(Esc/Tab/上下/Enter)与
  `suppressed_line` 关闭语义的壳侧接线归 M9.5。
- `paths` 的 shell 引用与转义唯一归 `slterm_completions::command_context`;
  本模块只决定目录、匹配与拼写(边界 3)。
