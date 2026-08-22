# L2 前端测试质量审查报告（域 A：工作区/面板核心）

## 执行摘要

- **审查范围**：`src/__tests__/` 下 56 个测试文件（工作区/面板核心域 A），逐文件阅读并对照实现代码。
- **被测代码**：`src/workspace/`、`src/panels/`、`src/stores/`、`src/lib/`、`src/features/` 相关实现。
- **基线结果**：56 个测试文件 / 882 个用例全部通过，0 失败。
- **复跑验证**：对 17 组高可疑用例连续复跑 8 次，发现 1 次偶发失败（`diff-panel.test.tsx`）；单独对 `diff-panel.test.tsx` 连续复跑 10 次全部通过，确认该用例存在并发/组合场景下的脆性。

---

## 问题清单

### [高] H-1：`diff-panel.test.tsx` 脏态弹窗用例在组合运行时偶发失败

- **维度**：异步 / 断言时序
- **证据**：
  - 复跑命令（17 个可疑文件组合）第 1 轮出现失败：
    ```
    FAIL src/__tests__/diff-panel.test.tsx > DiffPanel > 脏态外部 Modify → confirmDialog 弹窗；取消保留本地修改
    TypeError: Cannot read properties of null (reading 'dispatch')
    ❯ src/__tests__/diff-panel.test.tsx:630:15
    ```
  - 失败用例代码：`src/__tests__/diff-panel.test.tsx:620-651`，其中 `:627` 取 `getDiffView(container, "diff-right")`，`:630` 直接 `rightView.dispatch(...)`；`getDiffView` 仅当 `[data-e2e="diff-panel"] .cm-editor` 存在且 `EditorView.findFromDOM` 能反查到实例时才返回非空（`:113-120`）。
  - 同文件单独复跑 10 次全部通过；基线 56 文件全量运行也全部通过。
- **证据类型**：实证
- **问题**：测试仅 `waitFor` 到外层 `[data-e2e="diff-panel"]` DOM 出现，就认为 CodeMirror 编辑器已初始化完成。在组合运行/并发负载下，CM6 编辑器实例的创建可能滞后于面板 DOM，导致 `rightView` 为 `null` 时调用 `dispatch` 抛异常。
- **后果**：该用例在 CI/本地存在约 12.5%（1/8）的偶发失败率，会造成误报并掩盖真正的回归。
- **建议**：在 `dispatch` 前增加 `await waitFor(() => expect(getDiffView(container, "diff-right")).toBeTruthy())`，确保 `rightView` 非空再制造脏态。

---

### [中] M-1：全局 DOM/Canvas stub 存在跨文件污染风险

- **维度**：隔离性 / mock 清理
- **证据**：
  - `src/__tests__/workspace.test.tsx:40`、`src/__tests__/workspace-multi-instance.test.tsx:40`、`src/__tests__/workspace-switch-order.test.tsx:40`、`src/__tests__/workspace-page-dockview.test.tsx:45` 在模块顶层直接赋值 `global.ResizeObserver = class ResizeObserver { ... }`，未在 `afterAll` 中恢复。
  - `src/__tests__/use-xterm-integration.test.ts:162` 在模块顶层 `vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(...)`，未显式恢复（虽然 `setup.ts` 的 `afterAll` 会尝试 `mockRestore`，但模块级 spy 与全局 setup 的执行顺序依赖 Vitest 调度，不可作为清理保证）。
  - `src/__tests__/setup.ts:62-73` 对 `HTMLCanvasElement.prototype.getContext` 的包装按 beforeAll/afterAll 成对恢复，已做得较好；但模块级覆盖仍可能污染同一 worker 内后续文件。
- **证据类型**：静态推断 + 部分实证（H-1 的偶发失败可能与跨文件污染相关）
- **问题**：模块级全局 stub 未在 `afterAll` 中还原，若 Vitest 在同一 worker 内按不同顺序调度文件，可能造成行为差异，增加排错成本。
- **后果**：跨测试文件状态泄漏，容易催生 H-1 这类仅在组合运行时复现的脆性失败。
- **建议**：所有模块级全局覆盖统一改为 `beforeAll`/`afterAll` 成对安装/恢复；若需保持当前写法，至少补充 `afterAll(() => { global.ResizeObserver = originalResizeObserver; })`。

### [中] M-2：CodeMirror 几何测量 stderr 噪音被吞，测试仍通过

- **维度**：断言有效性 / 环境适配
- **证据**：
  - 运行 `diff-panel.test.tsx` 与 `diff-panel-stale-banner.test.tsx` 时 stderr 反复出现：
    ```
    TypeError: textRange(...).getClientRects is not a function
    ```
  - 该错误来自 CM6 在 jsdom 下测量 DOM 几何尺寸失败，但测试未因此失败。
- **证据类型**：实证
- **问题**：jsdom 缺少 `Range.getClientRects` 等几何 API，CM6 的 decoration/measurement 路径实际上处于异常回退状态；依赖这些路径的断言（如 gutter 占位、diff marker 数量）可能是“在错误环境下恰好通过”的假阳性。
- **后果**：stderr 噪音会掩盖真正的错误日志；基于几何测量的回归防护不可靠。
- **建议**：
  - 在 `setup.ts` 或相关测试中为 `Range.prototype.getClientRects` / `Element.prototype.getBoundingClientRect` 提供可返回固定值的 stub，消除异常路径；
  - 或评估 CM6 组件是否更适合放在 L3/L4 进行真实浏览器渲染验证。

### [中] M-3：Watermark 回归测试使用手写组件，未覆盖生产 `createWatermark`

- **维度**：回归覆盖 / mock 合理性
- **证据**：`src/__tests__/workspace-header-actions.test.tsx:356-442` 的 "Watermark 回归" describe 自行实现了一个内联 `Watermark` 组件，注释称“Watermark 等价于：createWatermark(nextId, pageId, cwd)”，但实际并未导入或渲染 `src/workspace/PageDockviewHost.tsx` 中的 `createWatermark`。
- **证据类型**：静态推断
- **问题**：回归测试验证的是一个“等价”手写组件的 `addPanel` 调用参数，而非生产 Watermark 的真实 JSX、样式、事件处理及与 `containerApi` 的交互。
- **后果**：如果生产 `createWatermark` 的渲染逻辑、按钮文案、`renderer` 默认值或 `params` 结构发生回归，该测试无法发现。
- **建议**：补充直接以生产 `createWatermark` 为被测对象的用例；或至少将手写 Watermark 的断言迁移到 `workspace-page-dockview.test.tsx` 中，通过真实 `PageDockviewHost` 触发空布局按钮。

### [中] M-4：部分 barrel `../lib` mock 仅导出少量成员，未来易碎

- **维度**：mock 合理性 / 维护性
- **证据**：
  - `src/__tests__/use-xterm-error-toast.test.ts:188-191`：
    ```ts
    vi.mock("../lib", () => ({
      toast: { show: mockToastShow },
      getErrorMessage: mockGetErrorMessage,
    }));
    ```
  - `src/__tests__/diff-panel.test.tsx:61-65`、`src/__tests__/diff-panel-stale-banner.test.tsx:58-62`：仅导出 `confirmDialog`/`toast`/`getErrorMessage`。
- **证据类型**：静态推断
- **问题**：上述 mock 覆盖了 `../lib` 的整个 barrel，但只提供被测文件当前引用的导出。若将来 `useXterm` 或 `DiffPanel` 改为从 `../lib` 引入新的工具（如 `E2E_ENABLED`、`useFontSizeWheel`、`StatusDot` 等），这些测试会静默把新依赖置为 `undefined`，导致行为偏离生产，甚至通过测试但产品崩溃。
- **后果**：barrel 新增导出即破坏，且失败点离真实原因较远。
- **建议**：优先使用 `importOriginal` 保留未 mock 的真实导出，再覆盖需要 stub 的成员；参考 `editor-confirm.test.ts:107-114` 的做法。

---

### [低] L-1：测试文件硬编码与实现同义的魔数阈值，未共享真值源

- **维度**：维护性 / 真值源
- **证据**：
  - 实现：`src/panels/terminal/usePtyOutput.ts:21-30` 定义 `DIRECT_WRITE_THRESHOLD=256`、`IDLE_FLUSH_MS=2`、`MAX_FLUSH_MS=16`、`MAX_PENDING_BYTES=65536`、`E2E_BUFFER_MAX_LINES=1000`（均未导出）。
  - 测试：`src/__tests__/use-xterm-output.test.ts:207` 写 `>=256`、`:323` 写 "直写阈值 256"、`:497` 写 "max timer 强制 flush" 依赖 16ms、`:1080` 硬编码 `MAX_PENDING_BYTES = 65536`、`:1166` 硬编码 `1000` 行截断。
- **证据类型**：静态推断
- **问题**：测试与实现各自维护同一组阈值。若性能调优需要修改 256/16/65536/1000 等数字，必须同时手动同步多处测试，否则会出现“实现正确但测试失败”或“测试通过但逻辑已变”的错位。
- **后果**：阈值调整成本上升，容易遗漏同步。
- **建议**：将 `usePtyOutput.ts` 的阈值常量导出为 package-private（`export const ...` 或 `export { ... } for testing`），测试统一导入引用。

### [低] L-2：`workspace-sideviews.test.tsx` 顶部注释与常量不一致

- **维度**：文档准确性
- **证据**：`src/__tests__/workspace-sideviews.test.tsx:4` 注释写“pane1 活动栏 40px 固定”，但代码 `:18-22` 从真实 sideBarState 导入 `ACTIVITY_BAR_SIZE`，实际值为 46px。
- **证据类型**：静态推断
- **问题**：注释未及时随三栏改造（46px）更新，会误导后续维护者。
- **后果**：低；仅文档漂移。
- **建议**：将注释改为“46px（以 ACTIVITY_BAR_SIZE 为准）”。

### [低] L-3：部分用例断言偏弱，仅验证工厂/中间状态存在

- **维度**：断言有效性
- **证据**：
  - `src/__tests__/use-code-mirror.test.ts` 早期用例仅断言 `capturedStateExtensions` 被定义或 `capturedStateExtensions.some(...)`，未深入验证 EditorView 实际文档、主题或事件处理。
  - `src/__tests__/editor-confirm.test.ts` 的脏态保存分支主要验证 `mockConfirmDialog` 被调用及 `mockGitDiff` 调用次数，对磁盘写入结果仅做间接断言。
- **证据类型**：静态推断
- **问题**：这些用例覆盖了分支触发，但对最终用户可见行为（文档内容、错误提示、保存后的状态）的断言较薄。
- **后果**：真实回归可能在“工厂被调用”层面仍通过，却已破坏功能。
- **建议**：在关键路径补充端到端断言（如保存后 `readFile` 内容、编辑器显示文本、toast 文案）。

---

## 审查覆盖声明

### 已审阅文件（56 个）

- `src/__tests__/active-editor.test.ts`
- `src/__tests__/active-terminal.test.ts`
- `src/__tests__/app-error.test.ts`
- `src/__tests__/app.test.tsx`
- `src/__tests__/bootstrap.test.ts`
- `src/__tests__/can-fit.test.ts`
- `src/__tests__/close-handler.test.ts`
- `src/__tests__/default-layout-format.test.ts`
- `src/__tests__/detect-webgl.test.ts`
- `src/__tests__/diff-alignment.test.ts`
- `src/__tests__/diff-panel-stale-banner.test.tsx`
- `src/__tests__/diff-panel.test.tsx`
- `src/__tests__/editor-confirm.test.ts`
- `src/__tests__/editor-font.test.ts`
- `src/__tests__/editor-keyboard.test.ts`
- `src/__tests__/editor.test.tsx`
- `src/__tests__/git-gutter.test.ts`
- `src/__tests__/gitshow-panel.test.tsx`
- `src/__tests__/html-panel.test.tsx`
- `src/__tests__/layout-serde.test.ts`
- `src/__tests__/layout-switch.test.ts`
- `src/__tests__/layout.test.ts`
- `src/__tests__/main-bootstrap.test.tsx`
- `src/__tests__/pageapis.test.ts`
- `src/__tests__/panel-registry.test.ts`
- `src/__tests__/panelId.test.ts`
- `src/__tests__/reserved.test.ts`
- `src/__tests__/startup-restore.test.ts`
- `src/__tests__/startup-store-fail-warn.test.tsx`
- `src/__tests__/terminal-instance.test.ts`
- `src/__tests__/terminal-lifecycle.test.ts`
- `src/__tests__/terminal-registry-subscribe.test.ts`
- `src/__tests__/terminal-registry.test.ts`
- `src/__tests__/terminal-rename-apply.test.ts`
- `src/__tests__/terminal-rename-dialog.test.tsx`
- `src/__tests__/terminal-strictmode.test.ts`
- `src/__tests__/terminal.test.tsx`
- `src/__tests__/title-bar.test.tsx`
- `src/__tests__/title-manager.test.ts`
- `src/__tests__/use-code-mirror-reload-error.test.ts`
- `src/__tests__/use-code-mirror.test.ts`
- `src/__tests__/use-xterm-error-toast.test.ts`
- `src/__tests__/use-xterm-integration.test.ts`
- `src/__tests__/use-xterm-lifecycle.test.ts`
- `src/__tests__/use-xterm-output.test.ts`
- `src/__tests__/webgl-setup.test.ts`
- `src/__tests__/workspace-callback-cache.test.tsx`
- `src/__tests__/workspace-defaulttab.test.tsx`
- `src/__tests__/workspace-e2e-ready.test.tsx`
- `src/__tests__/workspace-file-panel-types.test.ts`
- `src/__tests__/workspace-header-actions.test.tsx`
- `src/__tests__/workspace-multi-instance.test.tsx`
- `src/__tests__/workspace-page-dockview.test.tsx`
- `src/__tests__/workspace-sideviews.test.tsx`
- `src/__tests__/workspace-switch-order.test.tsx`
- `src/__tests__/workspace.test.tsx`

### 基线执行命令与结果

```bash
cd D:/data/learn/code/slTerminal
cut -f2 C:/Users/liush/AppData/Local/Temp/claude/domain-a-files.txt \
  | grep -E '\.test\.(ts|tsx)$' \
  | xargs npx vitest run
```

| 指标 | 结果 |
|------|------|
| 测试文件 | 56 passed |
| 测试用例 | 882 passed |
| 失败 | 0 |
| 耗时 | ~11.7s |

### 复跑验证记录

**可疑用例组合（17 文件，316 用例）复跑 8 次：**

```bash
npx vitest run \
  src/__tests__/diff-panel.test.tsx \
  src/__tests__/diff-panel-stale-banner.test.tsx \
  src/__tests__/use-xterm-output.test.ts \
  src/__tests__/use-xterm-lifecycle.test.ts \
  src/__tests__/workspace-multi-instance.test.tsx \
  src/__tests__/workspace-page-dockview.test.tsx \
  src/__tests__/workspace-switch-order.test.tsx \
  src/__tests__/workspace-header-actions.test.tsx \
  src/__tests__/use-xterm-integration.test.ts \
  src/__tests__/webgl-setup.test.ts \
  src/__tests__/terminal-strictmode.test.ts \
  src/__tests__/close-handler.test.ts \
  src/__tests__/panel-registry.test.ts \
  src/__tests__/workspace-callback-cache.test.tsx \
  src/__tests__/workspace.test.tsx \
  src/__tests__/terminal-lifecycle.test.ts \
  src/__tests__/editor-confirm.test.ts
```

| 轮次 | 结果 |
|------|------|
| 1 | **1 failed**（`diff-panel.test.tsx` 脏态弹窗用例 `TypeError: Cannot read properties of null (reading 'dispatch')`） |
| 2 | 17 passed / 316 passed |
| 3 | 17 passed / 316 passed |
| 4 | 17 passed / 316 passed |
| 5 | 17 passed / 316 passed |
| 6 | 17 passed / 316 passed |
| 7 | 17 passed / 316 passed |
| 8 | 17 passed / 316 passed |

**`diff-panel.test.tsx` 单独复跑 10 次：**

```bash
for i in $(seq 1 10); do
  npx vitest run src/__tests__/diff-panel.test.tsx
done
```

| 轮次 | 结果 |
|------|------|
| 1-10 | 全部 1 passed / 40 passed |

---

## 结论

- 域 A 56 个 L2 测试文件整体基线稳定，882 用例全绿。
- 发现 **1 个高优先级脆性用例**（`diff-panel.test.tsx` 脏态弹窗用例），在组合运行时约 12.5% 概率因未等待 CodeMirror 初始化而失败，需修复等待条件。
- 另有 4 个中优先级、3 个低优先级问题，集中在全局 stub 清理、CM6 jsdom 适配、barrel mock 覆盖、真值源共享、注释/断言质量等方面，建议按优先级逐步整改。
- 未发现被测生产代码缺陷。
