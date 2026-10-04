# pebrel 重构优化 spec · 分片 08：AI 辅助（补全引擎 / AI assistant / 数学渲染）

baseline commit：`e537d528c508e8607d0f5f9fd25e5902f40d661e`（pebrel，Rust + GPUI 模块化单体）

## 优化面

本分片覆盖六件事：

1. **补全引擎**——`nebula-completions/` crate 整体（nushell nu-cli 血统的独立轻量补全库）：`CandidateMatcher` 三算法匹配、`Completer` 可插拔 trait、file/directory/static 内建源、`CommandContext` 纯字面量 shell 语法解析、semantic 模块的 git/包管理器语义源（纯描述不做 I/O）。
2. **补全应用层**——`completion.rs` 的 Session/Request 分层与按 pane 来源缓存、`nebula_history.rs` fish 式作用域历史、`directory_history.rs` frecency 目录智能、`git_completion.rs` 有界本地 git 发现、`project_scripts.rs` 静态读 package.json、`display/suggest_engine.rs` ghost 余量 + 弹窗双形态（CompletionStyle 三态）。
3. **AI assistant**——`ai_assistant.rs` 错误自动恢复（OSC 133;D 非零退出 → LLM 建议条，默认关闭）+ `ai_providers.rs` provider 元数据/凭据体系（独立文件 + Zeroizing + Windows 凭据管理器）。
4. **数学渲染**——`math/` 约 5k 行自研 TeX 子集引擎（parser→validate→IR→layout→compile→后端无关绘制指令 + rasterizer/bitmap/cache）、终端覆盖层 `display/terminal_math/`（有界网格扫描）、markdown 阅读器数学管线（引擎全在 app 侧）、`scientific_corpus.rs` 回归体系。
5. **runtime skills**——`ai_hook/local/runtime_skills.rs` 给 claude/codex 装托管 Skill 的投放机制（分片 03 已登记挂点，本分片展开）。
6. **slTerminal 侧现状对照**——三个能力面在 slTerminal 均为全新增；`plan_balance`（套餐余量查询）与 AI assistant 的能力边界（细节归分片 10）。

已定全局决策：补全引擎照抄保留（历史 ghost + git/scripts 语义源 + 弹窗）；AI assistant（LLM 错误恢复，默认关闭）照抄保留；数学内联渲染照抄保留（终端覆盖层 + markdown 数学双管线）；化学/生物渲染连同语料砍；SSH 不采纳（补全的 SSH 连接源、远端元数据、远端历史池砍）；WSL 作用域本分片裁定砍（历史只留本地单池）；多平台、Lua 不采纳；隐私闸门（默认关 + redact_secrets + ≤2000 字符）照抄；provider 凭据独立文件 + Zeroizing 照抄（与分片 10 交界）；`nebula_*` → `slterm_*`。

## 采纳点

### 补全引擎（pebrel-completions crate → slterm_completions）

1. **照抄：crate 整体骨架与模块划分**（`nebula-completions/src/lib.rs`）。nushell nu-cli 提取的独立引擎，模块契约：`matcher` / `completer` / `file` / `directory` / `static_completion` / `command_context` / `command_search` / `semantic` / `options` / `span` / `suggestion`。两进程模型里它落 Rust 后端侧 crate，前端只消费最终候选，零改动移植。
2. **照抄：`matcher.rs::CandidateMatcher` 三算法匹配引擎**。Prefix/Substring 走 unscored 折叠匹配（`IgnoreCaseExt` 的 unicase 折叠），Fuzzy 走 nucleo-matcher（`prefer_prefix` 配置 + Smart normalization），引号剥离与 grapheme 对齐的 `match_indices`、Smart/Alphabetical 双排序——弹窗高亮与排序质量的全部来源。
3. **照抄：`options.rs::CompletionOptions` / `MatchAlgorithm` / `CompletionSort` 配置族**。照抄。
4. **照抄：`completer.rs::Completer` trait 签名**（`fetch(cwd, prefix, span, offset, options)`）。可插拔来源的合同，照抄。
5. **照抄：`file.rs::complete_item` / `complete_item_with_cancel` / `complete_literal_with_cancel` 递归路径补全**。exact-match 单命中直钻（隐藏兄弟条目）、n-dots 展开/折叠（`expand_ndots` / `collapse_ndots`）、home 展开（`expand_home`）、协作式取消（`cancelled` 闭包在每个目录条目间检查，取消即弃部分结果）、Windows 盘符前缀分量处理、`escape_path` 转义合同。`literal` 变体匹配与引用分离的编辑合同。
6. **照抄：`directory.rs::DirectoryCompletion` + `static_completion.rs::StaticCompletion` 两个内建源**。隐藏文件后置排序等细节照抄。
7. **照抄：`command_context.rs::CommandContext` / `ShellSyntax` 纯字面量 shell 语法解析**。Posix/PowerShell/Cmd/Literal 四方言；只切词、绝不执行展开/子命令/复合语句（`$` / `` ` `` / `%` / `;|&<>()` 等命中即拒解析）；`candidate()` 的 per-shell 引用与转义合同（PowerShell `-Fconfig.conf` 参数分裂整词引用、CMD 尾部反斜杠拒写、单引号 doubling、双引号反引号转义）；行长 4096、词数 64 上限。这是「补全可安全回写」的根。
8. **照抄：`semantic.rs::Source` 枚举 + `Context::parse` 语义源纯描述层**（模块头注释「describes sources, never performs I/O」即纪律）。git/npm/pnpm/yarn/ssh/wsl/common 命令分派；`attached` / `preserved_tail` / `value_prefix` 的值编辑合同（`git push origin main:topic` 的 `:topic` 尾保留、`--track=inherit` 附值偏移）；`candidates()` 的 fuzzy + 前缀提权 + 256 截断。
9. **照抄：`semantic/git.rs` git 参数角色全家**。switch/checkout/merge/rebase 选项表（`OptionSpec` 五属性：names/value/include_busy/terminal/optional_value）、push/pull/fetch 网络族（`git_network`：remote 位置、--multiple、--delete、refspec 尾保留、`PushRefs{destination}` 双侧）、`--track=direct|inherit`、`--guess/--no-guess` 与 `--no-track` 的相互作用、checkout `--` 后的分支/路径二态。纯函数可单测，照抄。
10. **照抄：`semantic/scripts.rs` 包管理器位置解析**。npm/pnpm/yarn 的 `--prefix/--dir/-C/--workspace/--filter/-w/-F`、`--workspaces/--recursive`、`--include-workspace-root`、`--if-present` 全识别，产出 `ProjectSelection` 描述——不调包管理器、不执行项目代码的语义边界照抄。
11. **照抄：`semantic/common.rs` 常用命令参数角色表**。POSIX/PowerShell/Cmd 三套别名（cd/ls/cat/cp/mv/rm/mkdir/grep + PowerShell cmdlet 族 + Windows 下 ls/cat/cp/mv/rm 的 cmdlet 归属特判）、`resolve()` 短选项连写与附值解析。Cmd 的 `/` 选项前缀形态照抄。
12. **照抄：`command_search.rs::CommandQuery` 词级模糊搜索**。空格分词、词序无关、标点字面（shell flag 永不变成搜索算符）、exact/prefix/word-prefix/substring/fuzzy 四级 tier 加权、多字段优先级（`score_fields`）。弹窗历史搜索的质量核心。

### 补全应用层

13. **照抄：`completion.rs::Session` / `Request` 分层**。Session 按 pane 持有 git/scripts 来源 Cache 并提供 `invalidate`；`request_with_syntax` 只拿输入快照与已确认执行环境，界面不决定来源适用条件（注释「来源缓存跟随 pane」即纪律）；`Request::calculate` 在后台算候选，取消/失效代次全链路检查。这是补全不上按键热路径的工程基础。
14. **照抄：`completion.rs::Cancellation`**（`Arc<AtomicBool>` 令牌）。取消语义在文件遍历、历史扫描、git 子进程间的统一形态。
15. **照抄：`nebula_history.rs::NebulaHistory` fish 式作用域历史（本地池）**。jsonl 追加 + BTreeMap 前缀 range 的 `hint`（取最新一条的余量，inline ghost 用）、`search_with_cancel`（popup 用 CommandQuery 评分、8 条上限）、连续重复去重、5000 条池上限、`parse_record` / `record_category_file` 的分池校验纪律。WSL/SSH 两池随远程裁定砍（见不采纳点），本地池语义与文件形态（改名 `slterm_history.jsonl`）照抄。
16. **照抄：`directory_history.rs::DirectoryHistory` frecency 目录智能**。rank × 四档时间衰减（小时/天/周/外）评分、2048 条目上限、总分 1 万封顶、跨进程文件锁 + 原子落盘、只记录本机终端真实启动/上报过的目录（「不引入 shell 特定命令」纪律）。cd 补全与目录幽灵提示的数据源。
17. **照抄：`git_completion.rs` 有界本地 git 发现**。3 秒总预算 + 1MB 输出预算双上限、`for-each-ref` 一次取全 + `config --null --get-regexp` 哨兵模式的两次读取、`Snapshot` 2 秒 TTL + generation 代次失效（提交命令后旧请求不得回填缓存）、busy/worktree/current/commit-ish/symbolic/direct_tracking 引用语义、tracking 上游推断（`git_completion/tracking.rs`）、`RevisionsAndPaths` 的 guesses 文件重名磁盘核查。数据发现与语义描述的分离形态照抄。
18. **照抄：`completion/project_scripts.rs` + `project_scripts/workspace.rs` 静态 scripts 发现**。只读 package.json 的 `scripts` 键（1MB 字节上限、向上找最近项目即停、损坏/缺 scripts 不借父项目、workspace 选择器展开）、绝不调包管理器不执行项目代码、2 秒 TTL + 代次纪律。`read_names` 的「must-not-run」测试断言随迁。
19. **照抄：`display/command_completion.rs` 命令位置 ghost 源**。PATH 可执行探针（PATHEXT 过滤、Windows 大小写不敏感去重、排序）、`nebula_command_hint` / `nebula_command_hints`（长度 + 字典序收敛）、`NEBULA_GHOST_MAX` 幽灵余量上限、`extract_program` 程序身份归一（小写、剥路径与 .exe/.cmd/.bat/.ps1/.com 后缀）——AI assistant 判定表与侧栏图标共用同一份身份。
20. **照抄：`display/suggest_engine.rs` ghost 余量 + 弹窗双形态计算核心**。`calculate` 的 Inline 三级回退（历史 hint → 命令位置 → 路径）、`suggest_collect` 的 Popup 多候选合并（历史 8 条 + 命令 + 路径、256 上限、44 字符左省略 `elide_left`、精确命令补空格、「8 是视口行数不是数据上限」注释承载的教训）；`suggestion_key` 缓存键（cwd + env + line + style + 命令代际）与 Esc 关闭的 `suppressed_line` 语义；`semantic_candidates_at` 把语义 byte span 投射成终端行尾编辑合同（只替换分歧尾部、已闭合引文接受、UTF-8 安全、`replace_chars` / `replace_after_chars`）。
21. **照抄：`CompletionStyle` 三态（Inline/Popup/Hybrid）**。定义在 `nebula_settings::CompletionStyleName`（含 `cycle` / `settings_value`），设置注册表面归分片 06，三态语义照抄。
22. **照抄：`completion/paths.rs` 路径来源复用**。语义上下文与裸行解析共用 `file.rs` 遍历、目录角色判定（`nebula_path_wants_directory`）、shell 转义只由 command_context 负责的边界。
23. **照抄：`display/completion.rs::nebula_commit_line` 的提交纪律**。Enter 提交历史时 screen_line（网格读回，Windows 下唯一能看见 tab 补全的来源）优先于按键重建的 line_buf——`suggest_skip` 注释里「laudeclaude」拼接垃圾的教训；宁可不记也不记脏。
24. **参考：`display/suggest_engine.rs::SuggestEnv` 环境抽象**。WSL/SSH 变体随远程整砍后收敛为 Local 单态，但「补全面对的是哪台机器」的根因注释（走错机器不是补不出来，是补出另一台机器上的路径）是错误模式知识载体，收敛后的代码注释保留其本地形态内核。

### AI assistant

25. **照抄：`ai_assistant.rs` 触发链整体**（`event.rs::maybe_request_ai_fix` 触发点 + 本模块纯函数判定）。OSC 133;D 非零退出（pebrel 自家 shell 集成上报，裸第三方 133;D 为 None）→ 便宜门链（同 pane `COOLDOWN` 5 秒 → `AssistantConfig::load` enabled 检查 → `should_suggest` 规则表）→ 抓网格输出尾部 → 后台线程请求 → `AiFixState`（Pending/Ready，`seq` 防过期响应落位）→ 底部建议条，Ctrl+. 只贴入不执行。任何一门不过都安静返回——这条路径跑在每次命令失败上，不能吵。
26. **照抄：`should_suggest` 防误触规则表**。`USER_ABORT_CODES`（130/137/143 + Windows STATUS_CONTROL_C_EXIT 负值——「用户停了它」不是「它失败了」）、`BARE_TOOLS` 单词调用（裸 git/npm 是帮助屏，修复只会烦人）、`INTERACTIVE` 交互程序表（vim/ssh/claude/codex/lazygit 等非零退出是常态）、`--help/-h/-?` 豁免、用户自定义 `ignored_exit_codes`。
27. **照抄：`grid_output_tail` 网格输出尾部提取**（`event.rs`）。光标处向上 24 行、尾部 2000 字符封顶、跳 WIDE_CHAR_SPACER（CJK 双胞空格污染）、去首尾空行——shell 侧集成看不见渲染上下文，终端网格是权威源。出境前过 `redact_secrets`。
28. **照抄：`redact_secrets` 隐私打码**。40+ 连续 base64/hex 字符（token/私钥体/连接串主体形态）替换 `[redacted]`——隐私闸门三件套（默认关 + 打码 + ≤2000 字符）之一，出厂默认写死成测试。
29. **照抄：`is_dangerous` 本地危险词表 + `parse_fix` 解析合同**。本地词表（rm -rf/git reset --hard/git push --force/remove-item -recurse/del /s/dd if= 等 21 条）与模型 `danger` 标志按位 OR——模型判意图、词表判字面，单方可被骗；`parse_fix` 抗 prose/fence 噪声提取 JSON、空 command 视为刻意沉默、单命令行约束。
30. **照抄：`AssistantConfig` + 独立配置文件**（`pebrel_assistant.txt` → `slterm_assistant.txt`）。默认 enabled=false；独立于设置主文件的理由注释（设置页整存整取会抹掉手工陌生键）照抄为模块知识。
31. **照抄：`ai_providers.rs` provider 元数据/凭据体系整体**。`pebrel_providers.json` 独立存放（凭据只存引用 `api_key_set`/`api_key_hint`，明文永不入 store）、`normalize` 预设补全与 active_id 修复、`credential_target` 经 `store_generic_secret` 进 Windows 凭据管理器、`api_key_hint` 尾四掩码（`••••` + 末四位的注释「user-facing fingerprint」）、`Zeroizing` 全程包裹（含 Bearer 头拼接串）、`apply_metadata_draft` 共享清洗器、`test_provider` 12 秒超时连通性测试 + 语义化结果枚举（`provider_test.rs::ProviderTestOutcome` 十态）、`remove_provider` 先删凭据再删元数据（不留孤儿密钥）。
32. **照抄：`ProviderKind` 十三家枚举全家**。OpenAi/Anthropic/Google/Ollama/OpenRouter/Qwen/DeepSeek/Kimi/Zhipu/Doubao/Mimo/AzureOpenAi/Custom 的 `PRESETS` / `label` / `default_base_url` / `default_model` / `requires_api_key` / `uses_openai_protocol`——slTerminal 面向 27 家 AI CLI 调优，provider 面不裁剪。
33. **照抄：`ai_assistant.rs::send_model_request` 四协议族**。OpenAI chat/completions（Authorization Bearer）、Anthropic /messages（x-api-key + anthropic-version）、Google :generateContent（x-goog-api-key）、Azure OpenAI（api-key + 部署路径）、`full_url` 直通形态；30 秒全局超时；响应按协议族取文本字段。
34. **照抄：`fallback_api_key` 环境变量兜底**。无启用 provider 时的 legacy 兼容路径（`NEBULA_AI_KEY` → `SLTERM_AI_KEY`、`OPENAI_API_KEY`），存在即用的零配置形态。
35. **参考：建议条 UI 与键位接线形态**。pebrel 侧建议条画在 pane 底部、Ctrl+. 贴入，实现落在 display/input 层；slTerminal GPUI 单进程里建议条是 pane chrome 的一部分、键位与「Ctrl+C 保留中断」纪律（产品定位约束）共存，交互语义照抄、落位随 pane 组件设计。

### 数学渲染

36. **照抄：`math/` 引擎整体**（`mod.rs` 头注释「不依赖窗口、OpenGL 或主题类型」的后端无关纪律）。parser→validate→IR→layout→compile→rasterizer/bitmap/cache/font/spacing 十模块一次随迁；pulldown-latex 固定版本（=0.7.1）事件流 + 自建有界 arena 的形态照抄。
37. **照抄：`validate.rs` 线性预算扫描器**。宏展开前的 `FORBIDDEN_COMMANDS` 黑名单（\def/\csname/\input/\directlua/catcode 等动态控制序列与外部资源命令——`\input{private.tex}` 必须报错）、`MathLimits` 八项预算（16KB 源 / 64 深 / 8192 事件 / 4096 节点 / 1024 矩阵格 / 1024 子节点 / 8192 op）与 `MathErrorKind` 错误分类。任何宏展开晚于这一层的顺序纪律照抄。
38. **照抄：`compile.rs` 单点编译入口与传输规范化**。`normalize_formula_source`（HTML 实体有限轮解码、CRLF 归一、行断裂痕修复、未加花括号分式参数规范化）与 `compile_formula_source` 原始路径的分流合同——「original source 不重写、不猜缺失行断、不静默替换箭头」的三条测试钉死；光学补偿只在 `compile_formula` 单点生效（缓存键/fit/draw 全持名义字号）。
39. **照抄：`font.rs` Latin Modern Math 零拷贝访问层**。`include_bytes` 内嵌字体（约 716 KiB）、OnceLock Face 进程级单解析、`MathConstant` 全枚举（轴高/分式/上下标/根号/极限全组）、`StretchPart` 字形组装（1024 上限）——换字体就是换这一个文件。
40. **照抄：`layout.rs` OpenType MATH 盒布局与后端无关绘制指令**。`MathMetrics` / `MathGlyphOp` / `MathRuleOp` / `MathTextOp` / `MathLayout` 五值类型族；`MathTextOp` 是紧凑数学字体缺字的跨平台文本兜底（「\text 里的空格曾吃掉整条流水线公式」教训的修复形态）。
41. **照抄：`rasterizer.rs` + `bitmap.rs` 有界 CPU 栅格化与合成**。glyph 512px 维度上限、coverage gamma 0.75（无 DirectWrite 子像素对比度的灰度笔画补偿）、位图 8192px/24MB 双上限、`required_bytes` 预检与实配一致纪律。
42. **照抄：`cache.rs::MathLayoutCache` 固定 4MB LRU**。`FormulaCacheKey`（公式编号 + 字号 bits + display）、`allocated_bytes` 计费、预算驱逐——防内存膨胀的唯一闸。
43. **照抄：`math/mod.rs` 光学常数族与理由注释**。`OPTICAL_SCALE` 1.21（KaTeX 对 Latin Modern 同族补偿的同源数值，x-height 0.431 em vs 编程字体 0.53–0.56 的实测理由）、`MIN_SCRIPT_SCALE` 0.8 / `MIN_SCRIPT_SCRIPT_SCALE` 0.65（刻意偏离 LaTeX 0.7 的终端理由：20px 下 0.7em 分子只剩 14px，以及 0.85 会把根号顶上字形变体阈值的实测上限注释）、`MIN_READABLE_MATH_PX` 6.0（两管线共用的最小可读底线，低于即回退源码）、`pixels_per_point` 统一公式（scale × 96/72.27，两壳显式长度换算不分裂）。这些注释是决策记录，逐字随迁。
44. **照抄：`display/terminal_math.rs` + `terminal_math/scan.rs` 终端覆盖层整体**。终端仍是真值——不改 cell/滚动/选区/复制，只在 paint pass 替换可见定界符跨度；有界扫描全家：`MAX_VISIBLE_FORMULAS` 64、持久化 2048 条/1MB 预算、裸定界符搜索预算（`BARE_PAREN_SEARCH_ROWS` 8 / `BARE_BRACKET_SEARCH_ROWS` 24 / `BARE_SEARCH_CELL_BUDGET` 4k cell——一屏几千个未闭合 `(` 不得把单帧扫描变成网格平方级）、定界符意图分级（`$$`/`\[`/`\(`/裸括号，意图自证的 O(1) 预过滤）、`LineProjection` 行投射（reflow 重排后覆盖坐标重建、选区单元格原子化——公式跨度是原子，左半/右半选中映射源边界而不是发明 TeX 内部光标）、空白行吸收（`MAX_ABSORBED_BLANK_ROWS` 2 + margin）、ANSI 背景与 TUI reasoning 样式放弃覆盖、行高溢出 12% 容差缩放。
45. **照抄：orphan closer 历史回溯**。视口锚点孤儿闭定界符按源码预算回溯历史行补全（`MAX_HISTORY_FORMULA_ROWS` 512 封顶）——AI TUI 硬折行公式的恢复路径。
46. **照抄：markdown 阅读器数学管线（双管线之一）**。`nebula_app/src/markdown/` 自包含文档模型（flat block 序列 + `FormattedTextFragment::Math` / `MathSource` 数学片段 + parser 的 quoted display math 折叠与 math fence 规范化）+ `display/markdown_view.rs` 的 `measure_math` / `fit_math_run` / `MathRun`（公式参与行宽测量与缩放、复用 `MathLayoutCache`、低于最小可读即回退源码）。引擎全在 app 侧、两条管线共用 `compile_formula` 与字体——「markdown 与终端定位公式的方式不同，但都不允许独立规范化/解析/排版」的边界注释照抄。
47. **照抄：gpui-component fork 的数学薄桥纪律**。fork 固定 rev 配对（`nebula-v1.16.1-math` 分支 + 完整 SHA，版本号与 Git rev 双锁定的注释纪律）+ TextView 数学钩子只作渲染挂点、引擎不下沉组件——slTerminal 侧以同形态重建 fork（改名 slterm 系），薄桥定位不变。
48. **照抄：`scientific_corpus.rs` 回归体系（数学部分）**。维护文档即语料（`docs/math-rendering-test.md` + `scientific-rendering-acceptance-paper.md` 数学 case），GFM `math_flow`/`math_text` 构造开启，case 期望标记注释（`pebrel-test: source-fallback` / `preserve-source`），编译 + 栅格化 + 像素非空断言 + preserve 语义断言（不发明缺失等号、逗号不变成不可见间距）、覆盖数下限断言（防语料缩水）、resize 重复的 LRU 有界测试。分子 case 与化学条目随化学砍（见不采纳点），语料文档的数学部分随迁为测试资产。
49. **参考：`parser.rs` 的 ASCII 箭头规范化与不支持命令替换表**。`normalize_ascii_math_arrows` / `substitute_unsupported_presentation` 机制照抄，具体替换表是实现细节随实现走；「original 路径不重写」的分流合同（见采纳点 38）是裁定的准绳。

### runtime skills

50. **照抄：`ai_hook/local/runtime_skills.rs` 托管 Skill 投放机制**。SHA-256 指纹（SKILL.md + agents/openai.yaml 双文件哈希）+ marker 文件归属三态（Installed/Current/Conflict）、原子写 + 先内容后 marker 顺序（崩溃不产生半套误认）、用户编辑保护（指纹 ≠ marker 即 Conflict，永不覆盖同名用户 Skill）、remove 只删自己文件且目录空才清、双目标投放（codex `~/.agents/skills` + claude 配置目录 `skills/`）、资产经 `include_str!` 内嵌自 `docs/skills/pebrel-runtime/`（资产与代码同源发布）。marker 改名 `.slterm-managed`。
51. **参考：`docs/skills/pebrel-runtime/SKILL.md` 内容资产**。Runtime API 控制面 Skill（`pebrel env` 定向 + pane/agent 命令族 + 环境变量身份契约），内容随 slTerminal runtime API（分片 04）改写产品名与命令面；投放机制（采纳点 50）与内容资产是分离裁定——机制照抄、内容重写过。

### slTerminal 侧现状对照

52. **全新增能力裁定：补全 / AI assistant / 数学渲染在 slTerminal 均无对应物**。slTerminal 现状（`src/panels/` 终端 + 设置中心 + plan_balance HUD）无任何补全、无错误恢复、无数学渲染资产——三个能力面整体新增，无迁移对象、无退役对象；落位随 GPUI 单进程 app 层组织（本分片所有 `nebula_app/src/` 路径平移为 app 层对应模块，见优化方向）。
53. **边界登记：`plan_balance` ≠ AI assistant**。`src-tauri/src/plan_balance/`（deepseek/kimi 套餐余量：读 user 层 settings.json token → 查询代理端点 → 5h/7d 用量快照 → HUD 展示，SEC-18 token 不出后端红线）是「用量查询展示」；AI assistant 是「错误恢复的 LLM 调用」（终端网格上下文出境 + 建议条交互）。两者只共享「AI provider 存在」这个大前提；凭据基建的共享面（`ai_providers` 凭据管理器 vs plan_balance 自读 settings.json）归分片 10 裁定，本分片只取 ai_providers 体系本身。

## 不采纳点

1. **`completion/connections.rs` 整支 + `semantic.rs::Source::SshHosts` + `semantic/common.rs` 的 SSH 选项表 + `Context::parse` 的 ssh 身份文件 home 特判**。理由：已定不采纳 SSH/远程——连接源（ssh config Host 发现、`ssh::hosts::discover`）无对象；语义源里 `SshHosts`/`WslDistributions` 两个变体一并删。
2. **`semantic/common.rs` 的 WSL 选项表 + `SuggestEnv::Wsl` + `nebula_history.rs` 的 WSL 池/`pebrel_history_wsl.jsonl` + `completion.rs` 里执行快照确认 WSL 改写 env 的特判 + `PaneExecContext::wsl_distribution` 在补全侧的接线**。理由（本分片裁定 WSL 作用域）：slTerminal 定位 Windows 原生、不走 WSL；历史只留本地单池，SuggestEnv 收敛 Local 单态。SSH 池与文件（`pebrel_history_ssh.jsonl`）同理删，`HistoryScope` 退化为 Local 单态。
3. **`SuggestEnv::Ssh` / `SuggestEnv::Shell{scope}` 变体 + `remote_dirs` 异步目录拉取通道 + `suggest_engine.rs::POSIX_COMMANDS` 兜底表 + `pending_remote_dir` 需求语义 + `suggestion_key` 的 remote_dirs generation 项**。理由：这组机制服务的就是「远端/WSL pane 的命令位置与目录补全」——环境整砍后无消费者；POSIX 表的存在理由（远端 PATH 未探到前的诚实兜底）随之消失。`Candidates::pending_remote_dir` 字段与 `Request` 的 connections 成员同步删。
4. **`completion/metadata.rs::Execution::Ssh` 变体 + `prepare()` 的 tokio 冻结 + `paths_not_found` 的 python3 远程探针 + `git_completion.rs` 的 `execution.is_host()` guest 分支**。理由：Execution 退化为本地进程单态；远程元数据执行范围无对象，is_host 恒真的分支折叠。
5. **`chemistry.rs` 整支 + `chematic_depict`/`chematic_smiles` 依赖 + `docs/chemistry-rendering-test.md` / `docs/biology-rendering-test.md` 语料 + `scientific_corpus.rs` 的分子 case 与 DOCUMENTS 分子条目**。理由：已定砍化学/生物渲染；上游本身 `STRUCTURE_RENDERING_ENABLED = false`（内存预算调查期停用），是既成事实的停用面而非活跃能力。回归体系只保数学部分。
6. **`nebula-completions` 的 `color` feature（`file.rs` 的 LS_COLORS 样式分支 + `color.rs` + `nu_ansi_term`）**。理由：nushell 遗产——ANSI 样式在 slTerminal GPUI 自绘弹窗（kind → 图标/配色映射走 theme token）无消费端；弹窗表现由 `NebulaCompletionKind`（Dir/File/Command/History）四态承担。
7. **`ai_assistant.rs` / `ai_providers.rs` 的 legacy-shell feature 通道**（`spawn_fix_request` 的 winit EventLoopProxy 形态、`spawn_test`、EventType::AiFixReady/ProviderTestDone 的 winit 事件变体）。理由：slTerminal GPUI 单进程——只保留 GPUI 任务/单进程消息循环投递形态（与分片 03 同判）。
8. **runtime skills 的 legacy 迁移层**（`LEGACY_RUNTIME_SKILL_MARKER` / `.nebula-managed` 回读 / `nebula-runtime` 目录迁移分支及配套测试）。理由：pebrel 双 marker 是为已装历史用户过渡；slTerminal 全新 fork 无迁移对象，单 marker `.slterm-managed` 直落。
9. **历史/目录史的 legacy 迁移面**（`directory_history.rs::import_legacy_history` 从 nebula jsonl 导入 + `unsaved_seed` 种子面）。理由：同上——slTerminal 无 nebula 旧数据，首启即空库直建；`unsaved_seed` 的跨进程迁移竞态处理随迁移面一起删。
10. **引擎与应用层内的 unix/macOS 平台分支**（`matcher.rs`/`file.rs` 的 `#[cfg(unix)]` 小分支、`command_completion.rs` 的 unix 探针分支、路径分隔符自适应中的 POSIX 路径）。理由：Windows-only 定位随多平台一并砍，保留 Windows 形态与通用逻辑；`file.rs` 的 Windows 盘符前缀处理照抄。
11. **Lua 配置接口**。理由：全局决策不采纳 Lua；与本分片资产无交。

## 优化方向

- **模块落位**：`nebula-completions/` 平移为 `slterm_completions` crate（骨架与公开契约零改动）；应用层按域落 app 层：`completion/`（Session/Request + `paths.rs` + `project_scripts/` + `metadata.rs` 收敛版）、`history.rs`（nebula_history 本地池单态化）、`directory_history.rs`、`git_completion/`、`suggest_engine.rs`（display 域下沉为补全域自由函数）、`ai_assistant.rs`、`ai_providers.rs`、`math/` 十模块、`terminal_math/`（终端覆盖层）、`markdown/`（文档模型）+ markdown 阅读器数学适配。改名一次做净：`nebula_*`/`pebrel_*` → `slterm_*`（历史文件 `slterm_history.jsonl`、助手配置 `slterm_assistant.txt`、provider 库 `slterm_providers.json`、凭据 target `Slterm/AI/<id>`、调试日志 `slterm_debug.log`、环境兜底 `SLTERM_AI_KEY`）。
- **远程裁剪后的三处收敛**：`HistoryScope` 单池化（Local 单态 + 单 jsonl 文件）、`SuggestEnv` 单态化（Local 单变体，键与缓存面同步瘦身）、`Execution` 单态化（本地进程单形态）——这是砍 WSL/SSH 的直接后果，收敛后补全链路只剩「本地 pane × 本地文件系统 × 本地 PATH × 本地 git」一个平面；`SuggestSources` 的 Borrowed/Shared 历史双形态在单进程下收敛为单一持有形态。
- **补全与 pane 的挂点关系**：来源缓存按 pane 挂（Session），与「会话元数据单点（终端注册表）」纪律的关系是——PTY 进程映射归注册表、补全来源缓存归 Session，两者以 pane id 关联；`invalidate` 由提交命令事件触发，代次纪律（旧后台请求不回填）写进模块约定。
- **ghost/popup 渲染面**：GPUI 自绘——ghost 为行尾余量文本（theme token 弱化色 + `NEBULA_GHOST_MAX` 上限），popup 为自绘候选列表（kind 四态图标 + 左省略 label + 选中态），CompletionStyle 三态进设置注册表（分片 06 交界）；Ctrl+. / Tab / Esc 交互语义照抄 pebrel 输入接线。
- **数学渲染双管线的单进程优势**：pebrel 为跨壳（winit/GPUI）而后端无关化；slTerminal GPUI 单进程里引擎直接服务两条管线——终端覆盖层在 paint pass 原生绘制（无 IPC），markdown 阅读器（panels/markdown 域）共用 `compile_formula` / `MathLayoutCache` / 字体单例；`MIN_READABLE_MATH_PX` 回退与 `OPTICAL_SCALE` 单点纪律保住两条管线判定一致。字体资产（LatinModernMath.otf）随迁进资源目录；`docs/math-rendering-test.md` 与验收论文数学语料进测试资产。
- **AI assistant 的挂点与出厂形态**：OSC 133;D 处理归终端核心（分片 02 交界，shell 集成语义沿用）；建议条为 pane chrome 组件，Ctrl+. 贴入与「Ctrl+C 保留中断」纪律共存；默认关闭 + 打码 + 2000 字符上限作为出厂默认写死成测试（隐私闸门三件套不可配置掉）。
- **provider 凭据面与分片 10 合流**：`ai_providers` 的 Windows 凭据管理器 + Zeroizing 纪律是凭据基建真源；plan_balance 的 settings.json 自读通道是独立面（SEC-18），两者是否共享存储归分片 10 裁定——本分片只保证 ai_providers 侧引用/掩码/不落明文的红线完整。
- **runtime skills 与安装调度合流**：投放挂进 config_guard 式 heal_all（分片 03 已登记），skill 内容资产（SKILL.md + openai.yaml）随 runtime API（分片 04）改写为 slterm-runtime；投放目标先 claude、codex 随九家一等清单定。
- **测试迁移**：纯函数三族（matcher/command_context/semantic 的 nushell 血统用例）、历史 hint/search 与 frecency 用例、git_completion 真实仓库 fixture（3 秒预算/代次失效/tracking 推断）、project_scripts 不执行项目代码断言、suggest_engine 环境隔离与双形态用例、ai_assistant 判定表/打码/危险词表/解析合同用例、ai_providers 预设/掩码/清洗器用例、scan 有界预算与投射用例、corpus 回归（数学部分）全随迁（Rust 单测）；ghost/popup 绘制与建议条交互归 GPUI 虚拟窗口 UI 测试关键路径用例（测试体系形态归分片 11）。bugfix 防复发纪律（TQ 红线）按新测试体系归位。
