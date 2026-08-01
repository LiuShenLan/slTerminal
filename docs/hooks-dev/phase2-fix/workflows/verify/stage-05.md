# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量回归任一命令失败则相关项判 not_fixed。
> 文档类断言须对照当前真实代码逐段核实，防文档撒谎。

## 断言清单

- **V1（PF2-DOC-02）**：四 CLAUDE.md 描述与最终代码一致（语义式：逐段 Read 对照代码核实不撒谎）——
  ① `src/features/sideViews/CLAUDE.md`：useAgentStatus 行语义 = 运行中 claude 会话（双通道建行/三通道删行/初始扫描只建活会话+携 transcriptPath 拉 usage/reconcile 对账）；AgentStatusRow 四字段口径；
  ② `src/panels/CLAUDE.md`：TerminalRegistry 含 claudeSession/setClaudeSession/sessionChange 描述；TabTitleRegistry match 为首 token 语义；
  ③ `src/ipc/CLAUDE.md`：notification 行为 `sendToastNotification(title, {body})`（Tauri 原生通道，无 onClick）+ AUMID 平台限制结论；
  ④ `e2e-tests/CLAUDE.md`：L4 用例表 agent-status 段为新语义（静态行反转 + 动态四态首信号建行 + R2/R3/R4 变体 3 条）。
- **V2（PF2-DOC-02）**：`src/panels/CLAUDE.md` 无 `TabTitleRegistry.test.ts` 驼峰残留（grep 零命中）；`tab-title-registry.test.ts` kebab-case 存在（grep 命中）。
- **V3（PF2-DOC-03）**：`.claude/test-inventory.md` 用例数与 `npm test` 实际输出一致——重写的 9 个文件（agent-status-hook / agent-status-view / notifications / tab-rules / tab-title-registry / terminal-registry / terminal-registry-subscribe / use-xterm-lifecycle / ipc-hooks-contract）逐一核对计数；L4 段（原 :257-259）agent-status 用例描述为新语义且条数与 `test.e2e.ts` 静态 it 计数一致。
- **V4（PF2-DOC-04）**：`src-tauri/src/hooks/CLAUDE.md` 含 hook 脚本性能实测结论（36-44ms/次）+ 接受现状决策（hooks 总贡献 ~0.1s 量级，非启动慢主因）——Read 确认两点齐全。
- **V5（PF2-DOC-01 核对 + 门禁）**：`docs/hooks-dev/contract.md` C12 段含四字段定义与口径说明（Stage 03 回填核对，drift 判 partial）；全量回归六命令全绿（见下）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npm run test:l3`
5. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
6. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
