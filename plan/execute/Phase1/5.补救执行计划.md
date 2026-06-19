# Phase 1 补救执行计划

> 依据：`plan/execute/Phase 1 验收审计.md` §11 审计决策台账 D1–D5
> 目标：修复 Phase 1 执行计划偏差，实现 DoD 完整达成

---

## 1. 编排总览

### 1.1 依赖分析

```
D1 键盘 handler（keyboard.ts + lib.rs）─┐
D2 E2E Buffer API hook（useXterm.ts）    │
D3 L3 弱测试修复（keyboard.test.ts）     ├── 互不冲突，可全并行
D4 依赖打包修复（package.json）          │
D5 terminalTabConfig（Workspace.tsx）   ─┘
                    │
                    └────── barrier ────────
                              │
                    验证 Phase（parallel）
                    cargo test / npm test / L3 / L4 / clippy / eslint
                              │
                    更新结果验证文档
                              │
                    人工验收
```

### 1.2 编排决策

按 `plan-generation.md` 决策树：**独立文件编辑、互不冲突、无屏障** → 1 个 dynamic workflow 全包。

| 层 | 方式 | 内容 |
|----|------|------|
| `/goal` 主干 | 15 turns 循环 | 锁 D1–D5 全部完成 + 自动化全绿 |
| W1 实现 workflow | parallel 5 agent | D1∥D2∥D3∥D4∥D5 同时启动 |
| W2 验证 workflow | parallel 6 agent | cargo test / npm test / L3 / L4 / clippy / eslint |

### 1.3 三层结构

```
/goal Phase1-Remediate D1–D5 全部修复（15 turns）
  │
  ├─ W1: 实现 (ultracode dynamic workflow)
  │   Phase '修复' [parallel]
  │     agent D1 — 键盘 handler（keyboard.ts + lib.rs）
  │     agent D2 — E2E Buffer API hook（useXterm.ts + test.e2e.ts）
  │     agent D3 — L3 弱测试修复（keyboard.test.ts）
  │     agent D4 — 依赖打包修复（package.json）
  │     agent D5 — terminalTabConfig 使用（Workspace.tsx + E2E test）
  │
  └─ W2: 验证 (parallel 6 agent)
```

---

## 2. 如何使用 /goal

### 2.1 完成条件

```
/goal 修复 Phase 1 D1–D5，下列每项在本会话中均有真实命令输出自证：
(1) D1 键盘 handler：keyboard.ts 存在，Ctrl+Shift+C/V 复制粘贴通过 L3，Ctrl+C 永不拦截（需求基线），lib.rs with_flags 恢复 Ctrl+F+DevTools
(2) D2 E2E Buffer hook：useXterm.ts DEV guard 暴露 window.__e2e_getTerminalText()，npm run wdio 验证 echo e2e_marker 输出
(3) D3 L3 弱测试：shift_tab 验证 serialize 含 \x1b[Z，ctrl_c 用 headless term.input('\x03') + onData 断言
(4) D4 依赖打包：@xterm/addon-serialize/@xterm/headless 移入 devDependencies，@codemirror/basic-setup 移除
(5) D5 面板入口 + renderer：Workspace.tsx 右键菜单可新建终端/编辑器，addPanel 传 renderer: 'always'
(6) L1–L4 仍全绿 + clippy 退出 0 + eslint 退出 0
硬约束：不弱化 lint/断言、不引入 Phase 1 范围外功能、不破坏已有测试
以上未全部满足则继续；或 15 turns 后停止并逐项汇报缺口。
```

### 2.2 运行方式

```
/claude
/goal ...（粘贴上述条件）
```

---

## 3. 如何用 dynamic workflow

### 3.1 W1：实现

```
Phase '修复' [parallel]
  agent D1 — 键盘 handler：keyboard.ts 新建 + lib.rs with_flags 修正（src/panels/terminal/，sonnet）
  agent D2 — E2E Buffer API hook：useXterm.ts 暴露 window.__e2e_getTerminalText() + test.e2e.ts 更新（src/panels/terminal/ + e2e-tests/，sonnet）
  agent D3 — L3 弱测试修复：keyboard.test.ts 修正 shift_tab 和 ctrl_c 断言（test/terminal/，sonnet）
  agent D4 — 依赖打包修复：package.json 移 @xterm/addon-serialize+@xterm/headless→devDependencies + 移除 @codemirror/basic-setup（package.json，sonnet）
  agent D5 — terminalTabConfig 使用：Workspace.tsx 右键菜单 addPanel 传 renderer: 'always' + E2E 同步（src/workspace/ + e2e-tests/，sonnet）
```

**触发**：
```
ultracode: 按以下编排执行 slTerminal Phase 1 补救 D1–D5，只做计划内修改、不做无关改动，每阶段完成后贴命令输出回主会话
```

> **无同文件冲突**——D1 改 keyboard.ts+lib.rs、D2 改 useXterm.ts+test.e2e.ts、D3 改 keyboard.test.ts、D4 改 package.json、D5 改 Workspace.tsx+test.e2e.ts。注意 D2 和 D5 都涉及 `e2e-tests/test.e2e.ts`——合并到一个 agent（D2），D5 只改 Workspace.tsx。

### 3.2 W2：验证

```
Phase '验证' [parallel]
  agent T1 — cargo test（L1 全量，sonnet）
  agent T2 — npm test（L2 全量，sonnet）
  agent T3 — npm run test:l3（L3，sonnet）
  agent T4 — npm run wdio（L4 E2E，sonnet）
  agent T5 — cargo clippy -- -D warnings（sonnet）
  agent T6 — npx eslint src/（sonnet）
```

---

## 4. 各 agent 任务卡

##### Agent D1 — 键盘 handler（Phase '修复'，sonnet）

| 项目 | 内容 |
|------|------|
| **任务** | 实现 keyboard.ts：Ctrl+C 智能分流 + Ctrl+Shift+C/V 复制粘贴；修正 lib.rs 的 prevent-default 配置 |
| **操作** | ① 新建 `src/panels/terminal/keyboard.ts` ② `window.addEventListener('keydown', handler, { capture: true })` ③ Ctrl+Shift+C → `navigator.clipboard.writeText(term.getSelection())` → preventDefault ④ Ctrl+Shift+V → `navigator.clipboard.readText()` → `term.paste(text)` → preventDefault ⑤ Ctrl+C → **永不拦截**，让 xterm.js 自然发 `\x03`（需求基线："Ctrl+C 保留为中断，复制用 Ctrl+Shift+C"）。xterm.js 源码确认 `_keyDown` 不检查 `hasSelection()`，浏览器 `copy` 事件与 `keydown` 独立触发——有选区时剪贴板被自动填充 AND `\x03` 同时发送，无需 handler 干预 ⑥ IME 组合中（`e.isComposing`）→ 全透传 ⑦ `lib.rs:29`：将 `init()` 改为 `Builder::default().with_flags(Flags::all().difference(Flags::FIND \| Flags::DEV_TOOLS)).build()` |
| **产出** | `src/panels/terminal/keyboard.ts`（新建）、`src-tauri/src/lib.rs`（修改 L29） |
| **验收** | `keyboard.ts` 文件存在；`npm run test:l3` key_encoding 测试通过；`cargo check` 通过 |

> **注意**：① `tauri-plugin-prevent-default` 不依赖 Tauri 权限系统——无需 `capabilities` 权限 ② Ctrl+W/T/N 已被 WebView2 硬编码关闭 ③ IME 防御推后 ④ **Ctrl+C 无需 hasSelection 判断**——xterm.js `copy` 事件处理（`copyHandler`）在有选区时自动填剪贴板，与 `\x03` 并行发生

##### Agent D2 — E2E Buffer API hook（Phase '修复'，sonnet）

| 项目 | 内容 |
|------|------|
| **任务** | useXterm.ts 暴露 `window.__e2e_getTerminalText()` 通过 serialize addon 返回终端文本；E2E 测试改用此 API |
| **操作** | ① `useXterm.ts` 中，终端 init 后用 `import.meta.env.DEV` 条件加载 `SerializeAddon`（Vite 生产构建时整个分支被 tree-shake 移除，不引用 `@xterm/addon-serialize`）② 暴露 `window.__e2e_getTerminalText = () => serializeAddon.serialize()` ③ `e2e-tests/test.e2e.ts` 中：在 echo `e2e_marker` 后 waitUntil → 调用 `window.__e2e_getTerminalText()` → 断言含 `e2e_marker` ④ E2E 的 `addPanel` 调用传入 `renderer: 'always'`（避免 onlyWhenVisible 下 xterm.js DOM 脱离导致 0×0 渲染） |

```typescript
// 仅 DEV/E2E 模式暴露 serialize hook
if (import.meta.env.DEV) {
  const serializeAddon = new SerializeAddon();
  term.loadAddon(serializeAddon);
  window.__e2e_getTerminalText = () => serializeAddon.serialize();
}
```
| **产出** | `src/panels/terminal/useXterm.ts`（修改）、`e2e-tests/test.e2e.ts`（修改） |
| **验收** | `npm run wdio` PASS，日志/断言含 `e2e_marker` 文本 |

> **注意**：`SerializeAddon` 读取终端内部 Buffer（纯数据层），与 WebGL/DOM 渲染器无关——无论加载哪个渲染器，`serialize()` 结果一致。来源：canopyide/canopy #1307

##### Agent D3 — L3 弱测试修复（Phase '修复'，sonnet）

| 项目 | 内容 |
|------|------|
| **任务** | 修复 keyboard.test.ts 中两个弱测试的无效断言 |
| **操作** | ① `key_encoding_shift_tab`：将 `expect(result.length).toBeGreaterThanOrEqual(0)` 改为 `expect(result).toContain('\x1b[Z')` ② `key_encoding_ctrl_c`：`\x03`（ETX）是 C0 控制字符，被 xterm.js InputHandler 消费不进 buffer，`serialize()` 无法保留。改用 headless `term.input('\x03')` + `onData` 断言收到 `\x03` 字节 ③ 确保两个测试能正确失败（非永真） |

```typescript
// key_encoding_ctrl_c 修正方案
it('key_encoding_ctrl_c', async () => {
    const { term } = createTerminal();
    const received: string[] = [];
    term.onData(data => received.push(data));
    term.input('\x03');  // 模拟 Ctrl+C keydown
    expect(received).toContain('\x03');
});
```
| **产出** | `test/terminal/keyboard.test.ts`（修改） |
| **验收** | `npm run test:l3` PASS，key_encoding 断言为真验证（非永真） |

##### Agent D4 — 依赖打包修复（Phase '修复'，sonnet）

| 项目 | 内容 |
|------|------|
| **任务** | 修正 package.json 依赖分类，移除冗余包 |
| **操作** | ① `@xterm/addon-serialize` 从 dependencies → devDependencies ② `@xterm/headless` 从 dependencies → devDependencies ③ 移除 `@codemirror/basic-setup ^0.20.0`（`codemirror` v6 的旧名包，已被 `codemirror` v6 完全替代）④ `npm install` 更新 lockfile |
| **产出** | `package.json`（修改）、`package-lock.json`（自动更新） |
| **验收** | `npm install` 退出 0；`npm test` + `npm run test:l3` 仍 PASS；`npm run tauri build -- --debug --no-bundle` 仍成功 |

##### Agent D5 — 面板创建入口 + renderer: 'always'（Phase '修复'，sonnet）

| 项目 | 内容 |
|------|------|
| **任务** | Workspace.tsx 创建面板入口（右键菜单 + 工具栏按钮），addPanel 传入 `renderer: 'always'` |
| **操作** | ① 提供 `getTabContextMenuItems` 回调：自定义"新建终端"/"新建编辑器"菜单项（`{ label, action: () => api.addPanel({...}) }`）+ 内置 close/closeOthers/closeAll ② 添加工具栏按钮（可选，"+"按钮创建终端）③ 在两个 `addPanel()` 调用中传入 `renderer: 'always'`（使用已定义的 `terminalTabConfig`）④ 删除未使用的 `terminalTabConfig` 导出（如果最终未引用）<br><br>**注意**：`onlyWhenVisible` 下 PTY 不会被 kill（Dockview 源码确认 `doSetActivePanel` 不调 `dispose()`），`renderer: 'always'` 的价值是防止 xterm.js DOM 脱离文档树导致 0×0 渲染/闪烁 |
| **产出** | `src/workspace/Workspace.tsx`（修改） |
| **验收** | `npm test` 仍 PASS；右键菜单可见"新建终端""新建编辑器"；全仓 grep `terminalTabConfig` 确认已被引用或已删除 |

---

## 5. 验收闭环

### 5.1 自动化验收

| 项 | 命令 | 通过标准 |
|----|------|---------|
| L1 | `cargo test` | 9 passed（含 Phase 0 回归） |
| L2 | `npm test` | 6 files 10 tests PASS |
| L3 | `npm run test:l3` | key_encoding_shift_tab 验证 `\x1b[Z]`；key_encoding_ctrl_c 验证 `\x03` |
| L4 | `npm run wdio` | 1 spec PASSED，验证 `e2e_marker` 文本 |
| Clippy | `cargo clippy -- -D warnings` | 退出 0 |
| ESLint | `npx eslint src/` | 退出 0 |
| Build | `npm run tauri build -- --debug --no-bundle` | exe 产出 |

### 5.2 人工验收

在所有自动化通过后：

- [ ] `slterminal.exe` 启动 → Ctrl+Shift+C 成功复制终端选中文本
- [ ] Ctrl+Shift+V 成功粘贴到终端
- [ ] `ping -t localhost` → Ctrl+C 停止（无选区时发 SIGINT）
- [ ] 选中终端文本后 → Ctrl+C 复制（不发 SIGINT）
- [ ] 终端中运行 `claude` → Shift+Tab 切换 Plan Mode
- [ ] 右键菜单新建终端 → 切换到另一个面板再切回 → PTY 未断开（进程仍存活）

### 5.3 DoD（完成定义）

- D1–D5 全部实现
- L1–L4 全绿
- 人工清单全部勾选
- Phase 1 结果验证文档更新反映修复后状态
- Phase 1 收口

---

## 6. 执行顺序速查

```
1. W1 实现（parallel 5 agent：D1∥D2∥D3∥D4∥D5）
2. W2 验证（parallel 6 agent）
3. 人工验收
4. 更新 Phase 1 结果验证.md
5. Phase 1 收口
```

---

## 7. 注意事项

- **同文件冲突**：D2 和 D5 都涉及 `e2e-tests/test.e2e.ts`——E2E 的 `renderer: 'always'` 合并到 D2（useXterm.ts + test.e2e.ts），D5 只改 Workspace.tsx
- **D1 与 D5**：D1 新建的 keyboard.ts 与 D5 不冲突（不同文件）
- **D4 后需用便携 Node 22 `npm install`**：lockfile 更新后用项目根目录 `.temp/node22/node.exe` 运行 `npm install` 保证 engine 兼容性
- **D2 的 `import.meta.env.DEV` guard**：Vite 生产构建时 `import.meta.env.DEV` 被静态替换为 `false`，整个分支 tree-shake 移除——不引用 `@xterm/addon-serialize`，与 D4 的 devDependencies 位置兼容
- **Ctrl+C 永不拦截**：`tauri-plugin-prevent-default` 不干扰 Ctrl+C（文本编辑键不在 Flags 范围内），xterm.js `copy` 事件在有选区时自动填剪贴板，`\x03` 同时发送到 PTY——无需 handler 干预

---

## 8. 决策台账

| # | 议题 | 决策 | 落点 |
|---|------|------|------|
| D1 | 键盘 handler 缺失 | 实现 keyboard.ts（Ctrl+Shift+C/V 复制粘贴，Ctrl+C 永不拦截）+ lib.rs with_flags。IME 防御推后 | `src/panels/terminal/keyboard.ts`、`lib.rs` |
| D2 | L4 E2E 终端输出验证 | useXterm.ts `import.meta.env.DEV` 条件暴露 `window.__e2e_getTerminalText()` 通过 serialize addon；E2E test 也传 renderer:'always' | `useXterm.ts`、`test.e2e.ts` |
| D3 | L3 弱测试 | shift_tab 验证 serialize 含 `\x1b[Z`；ctrl_c 用 headless `term.input('\x03')` + `onData` 断言（控制字符不进 buffer） | `test/terminal/keyboard.test.ts` |
| D4 | 依赖打包 | @xterm/addon-serialize + @xterm/headless → devDependencies；移除 @codemirror/basic-setup（npm deprecated，codemirror v6 替代） | `package.json` |
| D5 | 面板创建入口 + renderer | Workspace.tsx 创建 getTabContextMenuItems + 工具栏按钮，addPanel 传 renderer:'always'（onlyWhenVisible 下 PTY 不会被 kill，renderer always 防 0×0 渲染闪烁） | `Workspace.tsx` |
