# L2 前端测试审查报告：终端面板

## 范围与依据

- **审查对象**：`src/panels/terminal/` 领域对应的 15 个 L2 测试文件，合计 218 条用例。
- **被测源码**：`src/panels/terminal/*.ts(x)`。
- **依据**：`.claude/test-inventory.md`、已生成的 `docs/test-review-problem/coverage/frontend/coverage-summary.json`、源码静态对照。
- **方法**：纯静态分析，未运行测试/构建。

## 覆盖率基线（终端面板相关源文件）

| 源文件 | 行覆盖率 | 分支覆盖率 | 函数覆盖率 |
|---|---|---|---|
| `src/panels/terminal/webgl.ts` | 26.41% | 20.00% | 44.44% |
| `src/panels/terminal/TerminalPanel.tsx` | 94.11% | 42.85% | 84.61% |
| `src/panels/terminal/useTerminalInstance.ts` | 90.66% | 80.55% | 76.92% |
| `src/panels/terminal/usePtyOutput.ts` | 93.00% | 78.26% | 88.88% |
| `src/panels/terminal/useXterm.ts` | 91.50% | 91.37% | 66.66% |
| `src/panels/terminal/usePtyResize.ts` | 97.43% | 93.93% | 71.42% |
| `src/panels/terminal/TerminalRegistry.ts` | 90.32% | 100.00% | 75.00% |
| `src/panels/terminal/useClipboardHandler.ts` | 100.00% | 100.00% | 80.00% |
| `src/panels/terminal/useCommandDetection.ts` | 100.00% | 75.00% | 100.00% |
| `src/panels/terminal/keyboard.ts` | 100.00% | 87.50% | 100.00% |

## 问题总览

- **高优先级**：2 项（核心覆盖缺口、重复测试）
- **中优先级**：4 项（mock 污染、分支覆盖缺口、测试设计失真）
- **低优先级**：4 项（调试接口未覆盖、辅助函数与源码脱节等）

## 按文件问题清单

### `use-xterm-lifecycle.test.ts`（81 条用例）

1. **与 `use-xterm-output.test.ts` 存在大量重复用例**
   - `cancelPendingFlush` 测试几乎完整复制了 CPF1/CPF2/CPF5/CPF6/CPF7/CPF8/CPF9/CPF10/CPF11/CPF12/CPF13/CPF14/CPF15/CPF16 等 14 条用例。
   - 重复用例违反 DRY，增加维护成本；一旦合帧/resize 行为变更，需同步修改两处，漏改风险高。

2. **`setBufferType(..., "alternate")` 为虚假测试**
   - 测试名声称验证“交替缓冲中始终调用 fit”，但当前源码 `usePtyResize.ts` / `useXterm.ts` 中**没有任何读取 `terminal.buffer.type` 的逻辑**，`setBufferType` 仅给 mock 对象挂载了一个不会被读取的属性。
   - 结果：测试通过，但并未真实覆盖“交替缓冲”相关分支，存在虚假安全感。

3. **PTY spawn 完成依赖 `await Promise.resolve()` 等待微任务**
   - 多处通过单条 `await Promise.resolve()` 等待 `pty.spawn().then(...)` 中的 `TerminalRegistry.register` 完成。
   - 若未来 `pty.spawn` 内部实现改为多一次微任务或引入 setTimeout，此类断言会批量 flaky。

### `use-xterm-output.test.ts`（37 条用例）

4. **mock 对象存在 copy-paste 污染**
   - 以下 mock 工厂向本不导出 `hooks` 的模块写入了 `hooks:` 字段（明显是从 `../ipc` mock 复制后未清理）：
     - `vi.mock("@xterm/addon-fit", ...)` 第 136 行
     - `vi.mock("../panels/terminal/TerminalRegistry", ...)` 第 162 行
     - `vi.mock("../panels/terminal/TabTitleRegistry", ...)` 第 175 行
   - 这些多余属性当前不影响测试，但会误导维护者认为模块形状包含 `hooks`，也可能在未来做模块导出 shape 校验时暴露。

5. **与 `use-xterm-lifecycle.test.ts` 重复覆盖同一行为**
   - 除 `cancelPendingFlush` 外，ResizeObserver 相关合帧测试（如列变化 debounce、仅行变化立即 resize 等）与 lifecycle 文件重叠。

6. **未覆盖 `usePtyOutput.ts` 的 64KB 缓冲上限淘汰逻辑**
   - 源码第 209–217 行在 `pendingBufSizeRef.current + rawBytes.length > MAX_PENDING_BYTES` 时会丢弃最旧数据块。
   - 全部 37 条用例未发送超过 65536 字节的数据，该分支当前为 0 覆盖。

### `e2e-gating-terminal.test.ts`（5 条用例）

7. **mock 对象同样混入 `hooks:` 字段**
   - `vi.mock("@xterm/addon-fit", ...)` 第 113 行
   - `vi.mock("@xterm/addon-webgl", ...)` 第 121 行
   - `vi.mock("../panels/terminal/TerminalRegistry", ...)` 第 134 行
   - 与 `use-xterm-output.test.ts` 第 4 项问题同源。

### `terminal.test.tsx`（4 条用例）

8. **分支覆盖严重不足（42.85%）**
   - 现有用例仅覆盖挂载时 `loading=true`、`getWindowsBuildNumber` 调用、`pty.spawn` 调用、`Terminal.open` 调用四条路径。
   - 未覆盖：
     - 1.5s 超时后 `setLoading(false)` 隐藏遮罩；
     - `handleTabStateChange` 的 `active=false` 分支（恢复原标题、清除 tabIcon）；
     - `windowsBuildNumber` 获取后更新 `term.options.windowsPty`。

### `webgl.ts`（源文件，覆盖率 26.41%）

9. **核心渲染重试逻辑几乎未测**
   - `detectWebgl()` 在 `detect-webgl.test.ts` 中有 3 条用例覆盖，但 `setupWebglWithRetry()` 的以下路径全部缺失：
     - context loss 触发后的指数退避重建；
     - 重试次数耗尽后调用 `onFail()` 回退 DOM 渲染器；
     - 加载过程抛异常后的重试；
     - `cancel()` 清理定时器并 dispose addon。
   - 这是终端 GPU 渲染稳定性的关键路径，却依赖 E2E/L4 兜底，L2 缺失回归防护。

### `useTerminalInstance.ts`（源文件，分支覆盖率 80.55%）

10. **多条分支未覆盖**
    - `document.fonts.ready.then(...)` 的 `.catch` 或 `document.fonts` 不存在路径（源码未做防御，测试中也没触发）。
    - `fontSize === undefined` 时提前返回的分支。
    - `prevFontSizeRef.current === fontSize` 相同值跳过写入的分支。
    - `tryLoadWebgl()` 中 `webglAddonRef.current` 已存在时直接返回的分支。

### `TerminalRegistry.ts`（源文件，函数覆盖率 75.00%）

11. **`getAll` / `_size` / `_dump` 三个接口未被调用**
    - 这三个接口在源码中标注“仅用于调试/测试”，测试中也未使用，导致函数覆盖率从 100% 降至 75%。
    - 虽不直接影响业务，但若这些接口在 Agent Status / 历史区被依赖，则属于未覆盖的真实路径。

### `usePtyOutput.ts`（源文件，分支覆盖率 78.26%）

12. **退出码分支未完整覆盖**
    - `EXIT-1` 覆盖退出码 `1`，`EXIT-2` 覆盖 `null`；但未覆盖 `0` 或其他非空数字。
    - `isCommandRunningRef.current` 为 false 时退出不会触发 `onTabStateChange` 的分支虽已覆盖，但未显式断言。

13. **E2E 缓冲行数上限分支未覆盖**
    - 源码第 191–196 行在 `e2eTextBufferRef.current.length > E2E_BUFFER_MAX_LINES` 时截断缓冲，当前无对应测试。

### `use-xterm-integration.test.ts`（12 条用例）

14. **`term.input(...)` 触发 `onData` 属于间接验证**
    - INT-2.x 用例通过 `term.input()` 模拟键盘输入，依赖 xterm.js 内部把 input 转成 onData。虽然该路径有效，但对“用户按键 → PTY write”的验证不如直接触发 DOM keydown 真实。
    - 且 `term.input` 在 headless/integration 测试中与真实键盘事件差异较大，L4 仍需兜底。

### `src/__tests__/helpers/xterm-test-utils.ts`

15. **`setBufferType` 辅助函数与源码脱节**
    - 该辅助函数给 mock Terminal 挂载 `buffer.active.type`，但当前终端源码未读取该属性。
    - 导致依赖它的测试用例（如 AB/CPF11–CPF16）看似在测“交替缓冲”，实则未命中交替缓冲相关代码。

## 按六维度汇总

### 1. 断言有效性

- 未发现断言与源码实现明显冲突（如文本匹配错误等）。
- 但大量重复断言（lifecycle/output 的 CPF 系列）只增加了用例数，未增加验证价值。

### 2. 覆盖率

- **严重不足**：`webgl.ts`（26.41% 行覆盖）核心重试逻辑缺失。
- **分支缺口**：`TerminalPanel.tsx`（42.85%）、`usePtyOutput.ts`（78.26%）、`useTerminalInstance.ts`（80.55%）。
- **未覆盖关键路径**：64KB 缓冲上限淘汰、E2E 缓冲截断、调试接口 `getAll/_size/_dump`。

### 3. 测试设计质量

- `setBufferType` 与 alternate buffer 测试失真。
- `use-xterm-lifecycle` 与 `use-xterm-output` 职责边界模糊，大量重叠。
- 过度依赖 `await Promise.resolve()` 等待异步注册，时序脆弱。

### 4. Mock 使用

- `@xterm/addon-fit`、`@xterm/addon-webgl`、`TerminalRegistry`、`TabTitleRegistry` 的 mock 中出现不属于这些模块的 `hooks:` 字段，属于 copy-paste 残留。
- 污染分布：`use-xterm-output.test.ts` 3 处、`e2e-gating-terminal.test.ts` 3 处。

### 5. 稳定性风险

- `vi.useFakeTimers()` / `vi.useRealTimers()` 在多个 describe 中切换，若某个 `afterEach` 漏恢复，会影响后续测试。
- `await Promise.resolve()` 等待 spawn 注册在 Vitest 异步调度变化时可能 flaky。
- `mockTabTitleMatch`、`mockResolve` 等 hoisted mock 的默认值在测试间共享，若某条用例修改后未 reset，会污染后续用例。

### 6. 结构/可维护性

- 14 条 cancelPendingFlush/ResizeObserver 用例重复，违反 DRY。
- `xterm-test-utils.ts` 中的 `setBufferType` 已成为与源码脱节的死辅助函数。
- `TerminalPanel.tsx` 测试文件仅 4 条用例，且集中在 happy path，未覆盖组件状态机。

## 前三名问题

1. **`webgl.ts` 核心渲染重试路径覆盖严重不足（26.41%）**：`setupWebglWithRetry` 的 context loss、指数退避、耗尽回退、`cancel()` 清理等关键逻辑在 L2 完全缺失，终端 GPU 渲染稳定性缺乏回归防护。
2. **`use-xterm-lifecycle.test.ts` 与 `use-xterm-output.test.ts` 存在约 14 条重复用例**：`cancelPendingFlush` 与 ResizeObserver 合帧测试在两边几乎逐字复制，既浪费执行时间，也增加同步维护成本。
3. **多处 mock 对象混入不属于目标模块的 `hooks:` 字段**：`@xterm/addon-fit`、`@xterm/addon-webgl`、`TerminalRegistry`、`TabTitleRegistry` 的 mock 被复制了 `../ipc` 的 `hooks` 形状，属于 copy-paste 污染，长期会误导维护并可能在 shape 校验时暴露。
