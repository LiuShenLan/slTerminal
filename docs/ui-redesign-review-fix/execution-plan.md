# Review 修复执行编排参数（ui-redesign-review-fix）

> 通用执行规则见 `/systematic-changes-execute`（单一真值源），本文件只写任务特定参数。
> 计划文档族：`docs/ui-redesign-review-fix/checklist.md`（48 项）+ `stages.md`（9 Stage）+ 本文件 + `workflows/`。

## Stage 表

| Stage | 名称 | 脚本 | verify | commit message |
|---|---|---|---|---|
| 01 | 浮层收尾 | workflows/stage-01-overlay-cleanup.js | workflows/verify/stage-01.md | `fix(overlay): 原生 alert/confirm 清零——统一 confirmDialog/toast + 会话反查上提 pageApis` |
| 02 | 配色单点收敛 | workflows/stage-02-theme-tokens.js | workflows/verify/stage-02.md | `fix(theme): 硬编码色收敛——titlebarCloseHover token + ROOT_CSS_VARS 扩 4 键 + TitleBar 订阅拆分` |
| 03 | 右键菜单与树行 | workflows/stage-03-context-menu.js | workflows/verify/stage-03.md | `fix(menu): panelId 延迟到 action 执行 + 右键菜单 hover 改 React state + 圆角档修正` |
| 04 | 对话框与通知形态 | workflows/stage-04-dialogs.js | workflows/verify/stage-04.md | `fix(dialog): 弹窗圆角档收敛 + ConfirmDialog 焦点陷阱 + ToastHost aria-live` |
| 05 | 侧栏与活动栏 | workflows/stage-05-sidebar-state.js | workflows/verify/stage-05.md | `fix(sidebar): splitRatio 双开保留 + reconcileZones 纯函数化 + ActivityBar 动效/指示线修正` |
| 06 | 杂项收敛 | workflows/stage-06-misc-frontend.js | workflows/verify/stage-06.md | `fix(misc): 加载页/错误边界字体栈统一 + GitShow 告警图标化 + 关窗 unlisten 兜底 + 焦点环接管` |
| 07 | 退役组件删除 | workflows/stage-07-retire-history-list.js | workflows/verify/stage-07.md | `refactor(agentHistory): 删除退役 HistorySessionList/Row（生产零消费）+ 测试迁移 NavHistoryRow` |
| 08 | 测试质量强化 | workflows/stage-08-test-hardening.js | workflows/verify/stage-08.md | `test(e2e+l2): waitForPanelTabStatus 更名 + 假守卫断言强化 + 测试数据口径对齐` |
| 09 | 规范修订与文档同步 | workflows/stage-09-spec-docs.js | workflows/verify/stage-09.md | `docs(spec): 字号 14px/文件树 24px 规范登记 + 文档失实修正 + 配色例外指向收敛` |

## git add 路径枚举（Stage commit 限定）

`src/`、`e2e-tests/`、`.claude/CLAUDE.md`、`.claude/test-inventory.md`、`docs/`（来源：config.json workflow.gitAddPaths 裁剪——本任务不触碰 src-tauri/ 与 test/）

## fix-loop args 规范

- 脚本：`docs/ui-redesign-review-fix/workflows/fix-loop.js`
- `args: { stage, failedItems, fixContext, verifyFile, constraints }`
- `verifyFile` = 对应 `docs/ui-redesign-review-fix/workflows/verify/stage-NN.md`（与 Stage 脚本同一真值源）
- `constraints`：各 Stage 特殊纪律——取值一律以对应 Stage 脚本头部注释的「fix-loop 调用本 Stage 时 args.constraints 传」为准（单点定义，本文不复制）；无该行则传空串。当前仅 Stage 08（只改测试/e2e 辅助）与 Stage 09（只改文档与注释）有特殊纪律

## 契约单点索引（跨 agent 共享，写死于各脚本头）

1. **confirmDialog/toast API**（既有，Stage 01）：`confirmDialog(opts): Promise<boolean>`（确认 true / 取消·Esc·遮罩 false）、`toast.show("success"|"warning"|"error", message)`；危险确认 `danger: true`
2. **pageApis 新导出签名**（Stage 01）：`findPanelForSession(cliId: string, sessionId: string): string | undefined`、`findPageIdForPanelId(panelId: string): string | null`
3. **配色新 token**（Stage 02）：ui 段标量 `titlebarCloseHover: "#c04747"` → facade 导出 `TITLEBAR_CLOSE_HOVER_BG`；`ROOT_CSS_VARS` 扩 4 键 `--sl-focus-border`（#6e9ff2）/ `--sl-scrollbar-slider` / `--sl-scrollbar-slider-hover` / `--sl-scrollbar-slider-active`（rgba(255,255,255,0.10/0.20/0.28)）——值与现硬编码逐字相同
4. **ConfirmDialog e2e 红线**（Stage 04）：`data-e2e="confirm-ok"/"confirm-cancel"/"confirm-dialog-mask"` 选择器零变更（history.e2e.ts:602 依赖）
5. **e2e 更名**（Stage 08）：`waitForPanelTabIcon` → `waitForPanelTabStatus`（签名不变）
6. **规范修订口径**（Stage 09）：字号「内容区默认 14px 登记例外」、行高「导航树 28/会话行 30/文件树 24 分档」——修订措辞照 checklist SPEC-01/02 原文

## 进度跟踪表（执行期逐 Stage 更新）

| Stage | 状态 | commit | 备注 |
|---|---|---|---|
| 01 | ✅ 已完成 | 3bbc8b2 | fix-loop 1 轮（diff-panel 测试隔离缺陷 mockRejectedValue→Once） |
| 02 | ✅ 已完成 | ee39acd | fix-loop 1 轮（theme/index.ts 注释计数 36→35） |
| 03 | ✅ 已完成 | 5c50c60 | 一次通过，无 fix-loop |
| 04 | ✅ 已完成 | fbd89ad | 一次通过，无 fix-loop |
| 05 | 未开始 | — | |
| 06 | 未开始 | — | |
| 07 | 未开始 | — | |
| 08 | 未开始 | — | 门禁含 `npm run e2e` |
| 09 | 未开始 | — | |

## 收尾（全部 Stage 完成后）

1. 人工验证点 11 项实测（stages.md 末节汇总：本次修复新增 5 项 + 继承 review 未实机 6 项）
2. 进度跟踪表全绿后向用户汇报交付
