# Stage 16 逐项验证断言（唯一真值源）

> stage-16 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 注意：helpers.ts 不在根 tsconfig include——本 Stage 门禁靠 `npx tauri build --debug --no-bundle` 构建级验证兜底。

<!--
生成纪律：
1. 逐 ID 对照 checklist 原文写断言，禁止凭记忆——断言内容与 checklist 条目一一对应
2. 每条断言必须可机械检验（grep 模式 / Read 确认 / 测试命令），不写"检查是否合理"
3. "禁止存在 X"类断言写语义式，防"改名迎合"；正向意图断言同样写语义式，防"字面通过"
4. 本文件由 stage 脚本与 fix-loop 共用——断言变更只改本文件，两处自动生效
5. 门禁命令按 Stage 触碰文件选择——触及 tsc/eslint 覆盖外文件时补 npx vite build 构建级兜底
6. 断言与该 Stage 完成后的真实中间态一致——计数/枚举类按中间态推导，不照抄终态（test.e2e.ts 拆分后计数按新 spec 推导）
7. 断言证据在本 Stage 门禁命令产出内——不可得则收窄取数口径（静态 grep 计数）或补门禁命令
-->

## 断言清单

- **E2E-04**：视觉回归用例存在（真实 WebView2：全屏 TUI 输出后 resize、切页签往返、WebGL→DOM 回退不白屏）——**M2 人工验证点**（截图基线人工确认，stages.md M2）；L3 定位声明由 DOC-02 收尾（本 Stage 可留注释）
- **E2E-05**：run-wdio.cjs 备份范围含 `~/.claude/settings.json`（存在时备份）+ exit 还原 + 清理 `~/.slterminal/hooks/` 与 `hooks-events/`（Read 确认三启动路径：node22 直跑/便携下载/fallback 均覆盖）
- **E2E-06**：真实 reporter 用例存在（`node ~/.slterminal/hooks/slterm-hook-reporter.js` + stdin 写 JSON + `SLTERM_PANEL_ID` env → 断言信号文件产生且被消费（页签 emoji 变化）；非法 JSON 输入脚本 exit 0——C10 守卫，Read 确认）
- **E2E-09**：test.e2e.ts 拆分完成（原文件 ≤800 行；新 spec 文件按领域落位：terminal/sidebar/agent/history/hooks 等且被 wdio.conf specs 通配覆盖，Glob 确认）；`withProjectAndTerminal`（或等价共享 setup）提取到 helpers.ts（Read 确认）
- **E2E-10**：`browser.pause(` 在 spec 中零残留（grep 零命中；`browser.waitUntil` 轮询具体状态替代）
- **E2E-11**：①"拖拽跨区"标题改"侧栏视图跨区移动状态机（R6/R7）"（实际走 store helper）；②恢复编排用例注释标注"部分端到端（断言到 pty.write 命令注入，不含真实进入会话）"（Read 确认两处）
- **E2E-12**：Job Object 杀父进程用例存在（spawn 持久子进程 → 强杀 slterminal.exe → 断言子进程无残留；KILL_ON_JOB_CLOSE 真实验证）
- **E2E-13**：①Node 22 便携版预置 `.temp/node22` 或 CI 固定 Node 22 跳过外网下载（Read 确认）；②还原前存在 `rmSync(settingsPath, {force:true})` 或等价（grep 命中）；③`e2e-tests/fixtures/claude-projects/README.md` 存在（Glob 命中）说明编码目录名/UUID 与排除规则同步关系
- **E2E-15**：wdio.conf 含重试配置（mocha retries 或 specFileRetries，grep 命中）
- **禁区**：L4 全程未触碰真实 `~/.claude/projects/`（`SLTERM_CLAUDE_PROJECTS_DIR` env 隔离 fixture）——**M3 人工确认**；`~/.claude/settings.json` 已还原、`~/.slterminal/hooks*/` 已清理（M3）
- **门禁**：`npx tauri build --debug --no-bundle` + `npm run wdio` 全绿（测试类断言据此判定）

## 全量测试（全部通过为门禁）

1. `npx tauri build --debug --no-bundle`
2. `npm run wdio`
