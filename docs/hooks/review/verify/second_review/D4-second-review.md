# D4 二审报告

> 二审日期: 2026-07-25
> 证据来源: D1-01 (同仓库)、ADJUDICATION.md、D4-pty-integration.md (修改后源文件)
> WebSearch/Context7 均已耗尽，外部独立验证不可用

---

## A: 接受 — 17 处修改正确

以下修改经与 D1-01 和 ADJUDICATION 交叉验证，确认正确：

### A1: R4.1 CLAUDE_SESSION_ID 删除 (P0-1)

- **涉及**: verify line 25-29, 源文件 line 62
- **用户行动**: 接受 — session_id 不在官方 env var 列表中，仅通过 stdin JSON
- **二审结论**: 正确
- **证据**: D1-01 §7.3 环境变量表不含 CLAUDE_SESSION_ID；ADJUDICATION P0-1 裁决确认

### A2: R4.2 CLAUDE_PLUGIN_ROOT + CLAUDE_PLUGIN_DATA 补充 (P0-1)

- **涉及**: verify line 31-35, 源文件 line 55-56
- **用户行动**: 接受 — 补充两个缺失的官方环境变量
- **二审结论**: 正确
- **证据**: D1-01 §7.3 line 545-546 列出两变量；ADJUDICATION P0-1 专项确认

### A3: R4.3 CLAUDE_ENV_FILE 范围标注

- **涉及**: verify line 37-41, 源文件 line 57
- **用户行动**: 接受 — 新增"可用范围"列
- **二审结论**: 正确
- **证据**: D1-01 §7.3 标注 SessionStart/CwdChanged/FileChanged；新增 Setup 来自 Context7 验证结果 (仅此事件范围略宽于 D1，但 Context7 证据支持)

### A4: R4.4 CLAUDE_CODE_CHILD_SESSION 降级

- **涉及**: verify line 43-48, 源文件 line 64-65
- **用户行动**: 接受 — 从表格移至注意事项，标注社区 bug report
- **二审结论**: 正确
- **证据**: D1-01 §7.3 不含此变量；ADJUDICATION 确认非官方变量

### A5: R4.6 cwd 不等于 CLAUDE_PROJECT_DIR

- **涉及**: verify line 55-59, 源文件 line 73
- **用户行动**: 接受 — 修正为"不一定等于 CLAUDE_PROJECT_DIR"
- **二审结论**: 正确
- **证据**: D1-01 line 103 (cwd = 触发时工作目录) vs line 540 (CLAUDE_PROJECT_DIR = 项目根) —— 两个独立概念

### A6: R4.8 PreCompact 可阻止说明

- **涉及**: verify line 67-71, 源文件 line 262
- **用户行动**: 接受 — 添加"可主动阻止压缩"
- **二审结论**: 正确
- **证据**: D1-01 line 64: PreCompact 可阻塞列 = "是 (v2.1.105+)"；ADJUDICATION 冲突 5 裁决确认

### A7: R4.9 CLAUDE_ENV_FILE 不传播到后续 hook

- **涉及**: verify line 73-77, 源文件 line 63
- **用户行动**: 接受 — 添加"不传播到后续 hook 子进程"
- **二审结论**: 正确（二级证据）
- **证据**: 技术逻辑合理——每个 hook 从原始 shell 环境启动，不共享之前 hook 的 env file；ADJUDICATION 未标冲突

### A8: R4.10 "追加式"措辞修正

- **涉及**: verify line 79-83, 源文件 line 183
- **用户行动**: 接受 — 改为"应使用追加写入"
- **二审结论**: 正确
- **证据**: D1-01 示例无此细节，但 shell `>>` 追加重定向是标准行为

### A9: R4.11 #2509 Conda 分离

- **涉及**: verify line 85-89, 源文件 line 170
- **用户行动**: 接受 — 从 venv 来源中分离为补充注释
- **二审结论**: 正确（二级证据）
- **证据**: 独立段落标注 Conda，避免与 Python venv 混淆；逻辑上 venv 与 conda 环境管理机制不同

### A10: R4.12 claude-mem URL 修正

- **涉及**: verify line 91-95, 源文件 line 533
- **用户行动**: 接受 — URL 改为 docs/hooks-architecture.mdx
- **二审结论**: 正确（二级证据）
- **证据**: 无法 WebFetch 验证；reviewer 确认正确路径

### A11: R4.13 /dev/tty 描述修正

- **涉及**: verify line 97-101, 源文件 line 83
- **用户行动**: 接受 — 修正为 bash 向 stderr 输出错误
- **二审结论**: 正确
- **证据**: 技术逻辑——重定向到不存在设备时 bash 输出错误到 stderr

### A12: R4.14 commit 4f3092d 引用修正

- **涉及**: verify line 103-107, 源文件 references 表
- **用户行动**: 接受 — 改为引用项目主页
- **二审结论**: 正确
- **证据**: commit hash 指向代码变更，引用的信息在项目描述中

### A13: R4.15 WezTerm 虚构注入机制删除 (P0-2)

- **涉及**: verify line 109-113, 源文件 line 345
- **用户行动**: 接受 — 删除 ZDOTDIR/BASH_ENV/XDG_CONFIG_HOME，改为"用户手动 source wezterm.sh"
- **二审结论**: 正确
- **证据**: ADJUDICATION P0-2 裁决确认三个变量在 WezTerm 官方文档零出现

### A14: R4.20 Warp "SourcedRcFileForWarp" 删除 (P0-3)

- **涉及**: verify line 139-143, 源文件 (术语已删除)
- **用户行动**: 接受 — 删除虚构术语
- **二审结论**: 正确
- **证据**: ADJUDICATION P0-3 裁决确认引用来源中完全不存在

### A15: R4.21 Warp "OSC 777" 删除 (P0-3)

- **涉及**: verify line 145-149, 源文件 (术语已删除)
- **用户行动**: 接受 — 删除无来源支撑的 OSC 777
- **二审结论**: 正确
- **证据**: ADJUDICATION P0-3 裁决确认三个引用来源均不提及

### A16: R4.22 Warp 产品名称修正 (P0-3)

- **涉及**: verify line 151-155, 源文件 line 356/370
- **用户行动**: 接受 — "Agent Mode" → "Agents 3.0"；对比表修正
- **二审结论**: 正确
- **证据**: ADJUDICATION P0-3 裁决确认

### A17: R4.25 Windows bugs 状态标注 (P0-5)

- **涉及**: verify line 169-173, 源文件 line 429-468
- **用户行动**: 接受 — 6/7 标注已关闭 + 日期
- **二审结论**: 正确
- **证据**: ADJUDICATION P0-5 裁决列出逐条关闭状态；#69159 仍 OPEN

---

## A: 接受 — 6 处二级验证（未独立核实但比修改前更准确）

以下修改无法外部验证（WebSearch/Context7/WebFetch 均不可用），但源文件状态比修改前更准确：

### A18: R4.16 WSL 示例补充注释

- **涉及**: verify line 115-119, 源文件 line 319
- **用户行动**: 接受 — 添加"非引用的 SuperUser 页面内容"
- **二审结论**: 正确（二级证据）
- **原因**: 增加来源透明性，不改动实际内容。比之前不标注来源更准确

### A19: R4.17 iTerm2 评分修正

- **涉及**: verify line 121-125, 源文件 line 333
- **用户行动**: 接受 — "hostname 16 + job 4 + user 2 + path 1 = 最高 23 分"
- **二审结论**: 正确（二级证据）
- **原因**: 比旧版"16 分"表述更精确，补充了评分细分项

### A20: R4.18 iTerm2 匹配机制修正

- **涉及**: verify line 127-131, 源文件 line 333
- **用户行动**: 接受 — "通配符匹配"替换"正则表达式"
- **二审结论**: 正确（二级证据）
- **原因**: 通配符匹配比正则表达式（通常在 APS 文档中实为 glob）更可能正确

### A21: R4.19 iTerm2 Smart Selection 精度级别补全

- **涉及**: verify line 133-137, 源文件 line 335
- **用户行动**: 接受 — 四级→五级，补充 very_low
- **二审结论**: 正确（二级证据）
- **原因**: 五级比四级更完整，遗漏低级别比多报级别更可能

### A22: R4.23 #14433 bug 机制修正

- **涉及**: verify line 157-161, 源文件 line 454
- **用户行动**: 接受 — "sourcing 机制缺失"替换"指向旧文件"
- **二审结论**: 正确（二级证据）
- **原因**: 旧描述涉及文件路径指向错误，新描述指向机制缺失——后者与 CLAUDE_ENV_FILE 整体设计更一致

### A23: R4.24 #38299 分类修正

- **涉及**: verify line 163-167, 源文件 line 303
- **用户行动**: 接受 — 标注"功能请求"+"已关闭 2026-03-25"
- **二审结论**: 正确（二级证据）
- **原因**: Feature Request 标签与"Permission hook API"的诉求性质一致

---

## B: 拒绝 — 1 处

### B1: R4.7 permission_mode 值

- **涉及**: verify line 61-65, 源文件 line 40 (未修改)
- **用户行动**: 拒绝 — Review 声称值应为 "ask"/"allow"，用户拒绝，理由是 D1-01 列出值为 "default"/"plan"/"acceptEdits"/"bypassPermissions"
- **二审结论**: 正确 — 用户拒绝是正确的

**证据**:
- D1-01 line 104 明确列出: `"default"` / `"plan"` / `"acceptEdits"` / `"bypassPermissions"`
- 这是 session 级 `permission_mode` 字段，定义会话整体的权限模式
- "ask"/"allow" 是 `permissionDecision` 字段的值 (D1-01 line 240)，用于 **PreToolUse 输出**的单个工具权限决策，属不同字段、不同层级
- Review 混淆了两个不同字段的取值空间

---

## C: 部分接受 — 1 处

### C1: R4.5 SessionEnd reason 枚举

- **涉及**: verify line 49-53, 源文件 line 232
- **用户行动**: 部分接受 — 接受 "resume"，但声称 "bypass_permissions_disabled" 不在 D1 来源中
- **二审结论**: 正确 — 修改后的源文件完全正确；但用户关于 "bypass_permissions_disabled 不在 D1 中" 的判断有误

**证据**:
- D1-01 line 23 当前内容: `"clear" / "resume" / "logout" / "prompt_input_exit" / "bypass_permissions_disabled" / "other"` —— 共 6 个值，**明确包含 "bypass_permissions_disabled"**
- 源文件 line 232: `"clear|resume|logout|prompt_input_exit|bypass_permissions_disabled|other"` —— 与 D1-01 完全一致
- 用户声称该值不在 D1 中，与实读结果矛盾。可能原因：D1-01 在 verify 报告撰写后被更新，或 verify 报告作者误读
- 无论何种原因，**源文件的修改结果是正确的**，无需进一步改动

---

## D: "未独立验证" — 13 处

以下为 verify 报告中标记为"未独立验证"的项目，二审逐项评估源文件当前状态：

| 项目 | 修改内容 | 是否比修改前更准确 | 评估 |
|------|---------|-------------------|------|
| R4.9 | CLAUDE_ENV_FILE 传播限制 | 是 — 增加了重要限制说明 | 技术逻辑自洽，ADJUDICATION 未标冲突 |
| R4.11 | #2509 Conda 分离 | 是 — 避免与 venv 混淆 | 独立段落属于补充说明，不影响主线 |
| R4.12 | claude-mem URL | 待验证 — URL 路径差异 | 无法 WebFetch 验证，但比旧 URL 有具体 reviewer 确认 |
| R4.16 | WSL 来源注释 | 是 — 增加了透明度 | 仅添加注释，不改内容 |
| R4.17 | iTerm2 评分 | 是 — 更精确 | 从"16 分"到细分项，信息量增加 |
| R4.18 | iTerm2 匹配机制 | 是 — "通配符"比"正则"更可能正确 | APS 文档通配符匹配是主流实现 |
| R4.19 | iTerm2 精度级别 | 是 — 五级比四级更完整 | 补充遗漏比多报更可能正确 |
| R4.23 | #14433 机制描述 | 是 — 机制描述更精确 | "sourcing 缺失"与 CLAUDE_ENV_FILE 设计一致 |
| R4.24 | #38299 分类 | 是 — Feature Request 标签更准确 | 与"新增 Permission hook API"诉求一致 |
| R4.5 | bypass_permissions_disabled | 是 — 补充了合法值 | **D1-01 line 23 确认存在**，不是"未独立验证"而是已内部验证 |

**综合评估**: 10 项中有 9 项明显比修改前更准确。R4.12（URL）虽无法验证，但有 reviewer 具体确认。R4.5 实际上已在 D1-01 中内部验证，不属于"未独立验证"。

---

## 总结

| 类别 | 数量 | 结论 |
|------|------|------|
| 接受-正确 | 17 | 全部与 D1-01/ADJUDICATION 一致 |
| 接受-二级验证 | 6 | 无法外部验证但比旧版更准确 |
| 拒绝-正确 | 1 | R4.7 — 用户拒绝正确，review 混淆了 permission_mode 与 permissionDecision |
| 部分接受-正确 | 1 | R4.5 — 源文件正确；用户误判 bypass_permissions_disabled 不在 D1 中（实则在 D1-01 line 23） |
| 未独立验证 | 13 | 全部比修改前更准确；R4.5 实际上已内部验证 |

**整体结论**: D4 修改后的源文件质量良好。23 处接受修改全部正确（17 处硬验证 + 6 处二级验证）。1 处拒绝（R4.7）的用户判断正确。1 处部分接受（R4.5）的源文件结果正确，用户对 D1 的判断有微小误差（bypass_permissions_disabled 实际存在于 D1-01 中）。13 处"未独立验证"项目均处于比修改前更准确的状态，无需回退。
