# 补全应用层

> 补全来源编排、后台计算、取消与呈现接线；匹配引擎归 `slterm_completions`。

## 决策与红线

- **补全不上按键热路径**：按键路径只更新输入快照与缓存键；候选在后台 `Request::calculate` 计算。任何新来源都不得在按键路径做磁盘/子进程 I/O。原因：按键延迟是终端交互的核心体验。边界：来源发现与读取一律后台执行。
- **来源缓存跟随 pane**：`Session` 持有 Git/scripts 缓存，以 PaneId 关联；界面不决定来源适用条件。提交命令事件后必须 `Session::invalidate()`。
- **取消令牌全链路**：文件遍历、历史扫描、Git 子进程统一为 `Cancellation`（`Arc<AtomicBool>`）；取消后不得回填缓存或呈现部分结果。
- **源收敛为本地**：远程/SSH/WSL 来源与执行环境整支不迁；`SuggestEnv` 只有 Local。原因：产品定位是 Windows 本地终端。边界：新增远程能力时先改补全设计，不得在本模块私开分支。
- **执行元数据只描述本机**：`metadata::Execution` 的 `prepare()` 是本地直通；Git 读取必须经 `git::command` 构造口（`--no-optional-locks`），不得另起执行体。
- **呈现接线单点**：ghost/popup、按键（Esc/Tab/上下/Enter）与 `suppressed_line` 关闭语义由壳侧统一接线；来源模块只产出候选。
- **路径转义归属**：shell 引用与转义唯一归 `slterm_completions::command_context`；本模块只决定目录、匹配与拼写。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 虚拟窗口：`cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`。
- 必测面：按键路径零来源 I/O、缓存随 pane 失效、取消后不回填、本地来源收敛、Git 命令经 facade 构造、ghost/popup 与关闭语义。

## 操作

- 新增来源：在后台实现 `Completer` 并注册到 `Session`；同步补取消与缓存失效测试。
- 修改呈现：在壳侧统一接线，不在来源模块直接操作视图。
