# 11 L2 hooks 配置面板测试 Review

## 元信息

- **领域**：hooks 配置面板（`src/panels/hooksConfig/` + `src/features/hooksConfig/schema/` + `src/ipc/hooksConfig.ts`）
- **测试文件数**：10（核心 9 文件 + `open-hooks-config-panel.test.ts`）
- **用例数**：205（inventory 口径：ipc-hooks-config-contract 12 + catalog 19 + matcher 21 + model 17 + panel 20 + jsonmode 17 + gui 25 + sync 8 + handlerform 38 + open-hooks-config-panel 5 + panel-registry 中 hooksConfig 相关 23 条按 1/2 折算后归并）
- **覆盖率概况**：纯函数层（catalog/matcher/model）覆盖完整；组件层（JsonMode/GuiMode/HandlerForm/EventTree/HooksConfigPanel）行覆盖 85-100%，但关键分支（generation 取消、选择态重置、linter 顺序、非法 JSON catch、卸载失败）存在断言缺失；`useHooksConfig` 的竞态取消与 askGuard 分支未覆盖。
- **审查日期**：2026-08-04

## 范围与基线

### 评审范围

| 测试文件 | 用例数 | 对应源码 |
|---|---|---|
| `src/__tests__/ipc-hooks-config-contract.test.ts` | 12 | `src/ipc/hooksConfig.ts` |
| `src/__tests__/hooks-config-catalog.test.ts` | 19 | `src/panels/hooksConfig/eventsCatalog.ts` |
| `src/__tests__/hooks-config-matcher.test.ts` | 21 | `src/panels/hooksConfig/matcherEngine.ts` |
| `src/__tests__/hooks-config-model.test.ts` | 17 | `src/panels/hooksConfig/configModel.ts` |
| `src/__tests__/hooks-config-panel.test.tsx` | 20 | `src/panels/hooksConfig/HooksConfigPanel.tsx`、`useHooksConfig.ts` |
| `src/__tests__/hooks-config-jsonmode.test.tsx` | 17 | `src/panels/hooksConfig/JsonMode.tsx`、`MatcherTester.tsx` |
| `src/__tests__/hooks-config-gui.test.tsx` | 25 | `src/panels/hooksConfig/GuiMode.tsx`、`EventTree.tsx` |
| `src/__tests__/hooks-config-sync.test.tsx` | 8 | `src/panels/hooksConfig/useHooksConfig.ts` |
| `src/__tests__/hooks-config-handlerform.test.tsx` | 38 | `src/panels/hooksConfig/HandlerForm.tsx` |
| `src/__tests__/open-hooks-config-panel.test.ts` | 5 | `src/workspace/pageApis.ts`（C13-7 同页单例） |
| **合计** | **182**（按 inventory 归并后 205） | — |

### 基线文档

- `src/panels/CLAUDE.md` hooksConfig 章节
- `.claude/test-inventory.md`

### 覆盖数据基线

通过 `node docs/test-review-problem/coverage/extract-uncovered.cjs` 获取：

| 源码文件 | 行覆盖 | 未覆盖行 | 未覆盖函数 |
|---|---|---|---|
| `src/panels/hooksConfig/EventTree.tsx` | 85.7% | 50, 148-151, 153 | `(anonymous_10)` |
| `src/panels/hooksConfig/GuiMode.tsx` | 95.7% | 186, 239-240, 250, 289, 427 | `(anonymous_41)` |
| `src/panels/hooksConfig/HandlerForm.tsx` | 98.8% | 291, 342 | `(anonymous_18)` |
| `src/panels/hooksConfig/HooksConfigPanel.tsx` | 95.2% | 105, 160, 176, 203, 247-248 | `(anonymous_6)` |
| `src/panels/hooksConfig/JsonMode.tsx` | 91.8% | 133, 148, 178, 188, 212-213, 216-217 | `(anonymous_10)`、`(anonymous_11)` |
| `src/panels/hooksConfig/MatcherTester.tsx` | 100.0% | — | — |
| `src/panels/hooksConfig/useHooksConfig.ts` | 95.9% | 117, 125, 139-140, 151, 207, 217-218 | `(anonymous_6)` |
| `src/features/hooksConfig/schema/index.ts` | — | 无独立输出（间接覆盖） | — |
| `src/ipc/hooksConfig.ts` | — | 无输出 | — |

## 覆盖率缺口

### 核心逻辑零覆盖/断言缺失

- `src/panels/hooksConfig/JsonMode.tsx:156`：两个 `linter()` 调用顺序未断言，仅验证了各自的 `delay`/`needsRefresh` 选项。若 `jsonParseLinter` 与 `jsonSchemaLinter` 被交换包装，语法错误可能进入 schema 校验路径，测试仍绿。
- `src/panels/hooksConfig/useHooksConfig.ts:117,125`：`load()` 的 generation 取消检查（正常路径与 catch 路径）零覆盖。快速切层/切项目时过期结果会覆盖当前层数据。
- `src/panels/hooksConfig/useHooksConfig.ts:207`：`reload()` 的 `askGuardRef` 抑制分支未覆盖。弹窗关闭后的回归触发可能再次弹窗。
- `src/panels/hooksConfig/HooksConfigPanel.tsx:160`：`handleJsonChange` 的 `JSON.parse` catch 分支未覆盖。非法 JSON 从 `JsonMode.onChange` 进入时组件可能崩溃。
- `src/panels/hooksConfig/HooksConfigPanel.tsx:247-248`：`handleUninstall` 的异常 catch 未覆盖。卸载失败时无错误提示回归。
- `src/panels/hooksConfig/HandlerForm.tsx:340`：record/stringArray 字段清空（`t.trim() === ""`）后应删除键的分支未覆盖。清空 args/headers 后残留旧值无回归。

### 边界分支未覆盖

- `src/panels/hooksConfig/EventTree.tsx:148-153`：未知事件分组（`UNKNOWN_EVENT_GROUP`）追加逻辑未测试。
- `src/panels/hooksConfig/EventTree.tsx:50`：`formatHandlerSummary` 的 `agent` 分支 0 命中。
- `src/panels/hooksConfig/GuiMode.tsx:239-240,289`：删除 matcher 组 / handler 时的选中态重置分支未触发（测试未先选中再删除）。
- `src/panels/hooksConfig/GuiMode.tsx:186,250`：`addEvent`/`addHandler` 的空候选/不支持类型防御分支未覆盖。
- `src/panels/hooksConfig/useHooksConfig.ts:151`：`setLayer` 同层 early return 未覆盖。
- `src/panels/hooksConfig/useHooksConfig.ts:217-218`：`rootPath` 变空后非 user 层回退 user 层的分支未覆盖。
- `src/features/hooksConfig/schema/index.ts`：`validateHooksJson` 缺乏直接单元测试（仅通过 JsonMode/sync 间接覆盖），缺失 required 字段、非法 handler type、数组根等边界用例。

### 低风险/可接受缺口

- `src/panels/hooksConfig/JsonMode.tsx:212-217`：事件按钮 `onMouseEnter`/`onMouseLeave` 视觉 hover 效果，纯视觉分支。
- `src/panels/hooksConfig/HooksConfigPanel.tsx:105`：初始注入状态 `"--"` 未断言。
- `src/panels/hooksConfig/HooksConfigPanel.tsx:203`：`refreshInjectionStatus` 查询失败 catch，降级逻辑简单。

## 问题清单

### 红色（严重）

#### R1. JsonMode 测试未锁定两个 `linter()` 的包装顺序

- **风险等级**：🔴 高
- **位置**：`src/__tests__/hooks-config-jsonmode.test.tsx:158-181`
- **源码**：`src/panels/hooksConfig/JsonMode.tsx:155-156`
- **问题描述**：`schema 扩展注册` 用例只断言了 `mockLinter` 两次调用的 `[1]` 选项分别是 `{delay:300}` 和 `{needsRefresh:mockHandleRefresh}`，但从未断言 `[0]` 实际传入的是 `mockJsonParseLinter()` 还是 `mockJsonSchemaLinter()`。生产代码顺序是 `linter(jsonParseLinter(), {delay:300})` 在前、`linter(jsonSchemaLinter(), {needsRefresh:handleRefresh})` 在后。若有人把两者交换，语法错误会被送进 schema linter，而 schema linter 的 `needsRefresh` 被挂到 parse linter 上，导致 schema 校验随 parse 变化刷新，行为严重异常但当前测试仍绿。
- **引用代码**：
  ```ts
  const linterCalls = mockLinter.mock.calls as unknown as [
    [unknown, unknown],
    [unknown, unknown],
  ];
  expect(linterCalls[0][1]).toEqual({ delay: 300 });
  expect(linterCalls[1][1]).toEqual({ needsRefresh: mockHandleRefresh });
  // linterCalls[0][0] / linterCalls[1][0] 未断言
  ```
- **改进建议**：追加 `expect(linterCalls[0][0]).toBe(mockJsonParseLinter())` 与 `expect(linterCalls[1][0]).toBe(mockJsonSchemaLinter())`（或断言标识）。
- **变异推演**：交换 `jsonParseLinter()` 与 `jsonSchemaLinter()` 在 extensions 数组中的位置，当前 JSON-mode 全部 17 条用例仍绿。

#### R2. `useHooksConfig.load()` 的 generation 竞态取消无测试守卫

- **风险等级**：🔴 高
- **位置**：`src/panels/hooksConfig/useHooksConfig.ts:110-129`
- **源码**：`load()` 函数第 117 行 `if (gen !== genRef.current) return;` 与第 125 行 catch 内的同名检查。
- **问题描述**：切层、切项目、重读均为异步 `readHooksConfig`。若某个旧请求在目标层已切换后才 resolve，必须通过 generation 检查丢弃，否则旧层配置会覆盖当前层。覆盖率报告显示这两处分支 0 命中，也没有任何用例模拟快速连续切换。
- **改进建议**：在 `hooks-config-sync.test.tsx` 或 `hooks-config-panel.test.tsx` 中新增用例：先切到 project 层并挂起其 read，再切回 user 层并 resolve user 读，最后 resolve project 读，断言最终 `configJson` 仍为 user 层数据。
- **变异推演**：删除 `if (gen !== genRef.current) return;`，当前全部 205 条用例仍绿；真实使用中快速切层会出现配置错层。

#### R3. `HooksConfigPanel.handleJsonChange` 对非法 JSON 的 catch 无回归

- **风险等级**：🔴 高
- **位置**：`src/panels/hooksConfig/HooksConfigPanel.tsx:146-155`
- **源码**：`handleJsonChange` 用 `try { JSON.parse(text) } catch { ... }`。
- **问题描述**：`JsonMode` 每次 doc 变化都会调用 `onChange`，包括非法 JSON 状态。`handleJsonChange` 的 catch 分支负责保留最后合法快照、避免组件崩溃。当前所有 panel/sync/jsonmode 测试均只向 `onChange` 传入合法 JSON，非法 JSON 进入此路径的回归缺失。
- **改进建议**：在 `hooks-config-panel.test.tsx` 中新增用例：通过 `mockJsonMode` 的 `onChange` 传入 `"{ "PreToolUse": "` 等非法文本，断言 `configJson` 保持原合法快照、组件不抛错、保存按钮因 `jsonValid=false` 仍禁用。
- **变异推演**：把 `handleJsonChange` 改为直接 `updateConfigJson(JSON.parse(text))` 不捕获，非法 JSON 输入时组件崩溃，但当前测试仍绿。

### 黄色（中等）

#### Y1. HandlerForm 清空 record/stringArray 字段后应删除键的逻辑未覆盖

- **风险等级**：🟡 中
- **位置**：`src/panels/hooksConfig/HandlerForm.tsx:337-342`
- **源码**：`args`/`headers`/`allowedEnvVars`/`input` 等 textarea 的 `onChange` 在 `t.trim() === ""` 时调用 `handleFieldChange(key, undefined)` 删除该键。
- **问题描述**：当前测试覆盖了清空 `timeout`（number 类型）后删除键，但没有覆盖清空 `args`/`headers` 等 record/stringArray 字段。若此分支被误删，用户清空参数/请求头后保存仍会携带旧值，造成数据错误。
- **改进建议**：在 `hooks-config-handlerform.test.tsx` 中新增用例：渲染 command 型 handler 并填入 `args`，再清空 textarea，断言 `onChange` 收到的新对象中不存在 `args` 键。
- **变异推演**：删除 `if (t.trim() === "") { handleFieldChange(key, undefined); }` 分支，清空 args 后 `onChange` 不再触发，当前测试仍绿。

#### Y2. GuiMode 删除选中 matcher 组 / handler 时未验证选中态重置

- **风险等级**：🟡 中
- **位置**：`src/panels/hooksConfig/GuiMode.tsx:229-244`（删除 matcher 组）、267-293（删除 handler）
- **源码**：删除当前选中的 matcher 组或 handler 时会重置 `selectedMatcherIndex`/`selectedHandlerIndex`。
- **问题描述**：现有测试删除的是未选中的组/handler，因此 `selectedMatcherIndex === index` 等条件分支未触发。若重置逻辑被删除，选中态会指向已不存在的索引；虽然派生守卫会把它归 null，但 UI 可能出现一闪而过的错误高亮或表单残留。
- **改进建议**：新增两个用例：1）先点击 matcher 组头选中，再删除该组，断言详情区回退空态；2）先选中 handler 使其展开 HandlerForm，再删除该 handler，断言 HandlerForm 消失。
- **变异推演**：删除 `deleteMatcherGroup`/`deleteHandler` 中的选中态重置逻辑，当前测试仍绿。

#### Y3. EventTree 未知事件分组逻辑未测试

- **风险等级**：🟡 中
- **位置**：`src/panels/hooksConfig/EventTree.tsx:143-154`
- **源码**：事件若不属于 `EVENT_GROUPS` 中任何已知分组，则追加 `UNKNOWN_EVENT_GROUP` 分组。
- **问题描述**：`jsonToGui` 明确保留未知事件并归入未知组。若 EventTree 的未知组追加逻辑被误改（如直接丢弃），未知事件在 GUI 模式下会消失，但当前测试全部使用已知事件，无法发现。
- **改进建议**：新增用例：传入一个 `group` 为 `"UnknownGroup"` 的事件，断言树中渲染出 `"未知事件"` 分组标题且包含该事件行。
- **变异推演**：把未知组事件也塞到 `EVENT_GROUPS[0]`，当前测试仍绿。

#### Y4. `HooksConfigPanel.handleUninstall` 失败路径未覆盖

- **风险等级**：🟡 中
- **位置**：`src/panels/hooksConfig/HooksConfigPanel.tsx:239-252`
- **源码**：`handleUninstall` catch 分支设置 `injectionError`。
- **问题描述**：注入失败路径已测试，但卸载失败路径没有对应用例。若卸载 catch 被删除或错误提示文案丢失，用户点击卸载失败后将看不到任何反馈。
- **改进建议**：新增用例：mock `uninstall.mockRejectedValue(...)`，点击卸载按钮后断言 `hooks-injection-error` 出现“卸载失败”文案，且未触发重读。
- **变异推演**：删除 catch 块，卸载失败时组件抛错，当前测试仍绿。

#### Y5. `validateHooksJson` 缺乏直接的 schema 边界用例

- **风险等级**：🟡 中
- **位置**：`src/features/hooksConfig/schema/index.ts:61-79`
- **源码**：`validateHooksJson` 使用 `json-schema-library` 的 `Draft07` 校验 hooks 子 schema。
- **问题描述**：当前仅通过 JsonMode 的非法 JSON/未知事件用例间接覆盖。缺少对以下关键 schema 约束的直接测试：handler `type` 不在枚举中、command handler 缺少必填 `command`、http handler 缺少必填 `url`、顶层为数组而非对象、空对象合法通过。这些边界是保存拒绝逻辑的基础。
- **改进建议**：新增 `hooks-config-schema.test.ts`，直接调用 `validateHooksJson` 断言上述边界。
- **变异推演**：若 hooks 子 schema 提取错误导致 `additionalProperties` 被放宽或 required 字段丢失，当前间接测试可能无法精确定位。

#### Y6. `open-hooks-config-panel` 未覆盖 `getPanel` 返回对象但无 `focus` 的降级

- **风险等级**：🟡 低中
- **位置**：`src/__tests__/open-hooks-config-panel.test.ts:57-68`
- **源码**：`workspace/pageApis.ts` 中 `openHooksConfigPanel` 调用 `getPanel(id)?.focus()`。
- **问题描述**：测试断言 `panel.focus` 被调用，但 stub 的 `getPanel` 返回的对象恒有 `focus`。若 `getPanel` 返回的对象缺少 `focus`（Dockview 的某些边界），`?.focus()` 会静默跳过，但测试无法发现这种退化。
- **改进建议**：新增边界用例：`getPanel` 返回 `{ }`（无 focus），断言函数不抛错、addPanel 不被再次调用。
- **变异推演**：把 `?.focus()` 改为 `.focus()`，无 focus 的 panel 会抛错，但当前测试仍绿。

### 绿色（提示/低优先级）

#### G1. JsonMode 事件导航的 hover 效果未测试

- **风险等级**：🟢 低
- **位置**：`src/panels/hooksConfig/JsonMode.tsx:211-218`
- **问题描述**：`onMouseEnter`/`onMouseLeave` 仅涉及背景色视觉反馈，无业务逻辑风险。
- **建议**：可接受不覆盖，或由 E2E 兜底。

#### G2. HooksConfigPanel 初始注入状态 `"--"` 未断言

- **风险等级**：🟢 低
- **位置**：`src/panels/hooksConfig/HooksConfigPanel.tsx:104-114`
- **问题描述**：挂载后、状态查询完成前，状态条显示 `"--"`。当前测试直接等待 `"未注入"`，未断言中间态。
- **建议**：非关键路径，可不补。

#### G3. MatcherTester 未测试 `matcherTarget` 占位文案切换

- **风险等级**：🟢 低
- **位置**：`src/panels/hooksConfig/MatcherTester.tsx:89`
- **问题描述**：`toolName` placeholder 随事件变化显示 `matcherTarget`，现有测试只验证了默认 PreToolUse 的全匹配。
- **建议**：低优先级，可后续补充一条用例。

## 变异测试分析

对核心代码做“关键行删除/交换”思想实验，判断当前测试能否发现：

| 变异目标 | 当前测试能否发现 | 说明 |
|---|---|---|
| `JsonMode.tsx`：交换 `jsonParseLinter` 与 `jsonSchemaLinter` 的 linter() 包装 | ❌ 不能 | 仅断言 options，未断言被包装的 linter 身份。 |
| `useHooksConfig.ts`：删除 `load()` 的 generation 检查 | ❌ 不能 | 无快速连续切换/过期结果丢弃用例。 |
| `useHooksConfig.ts`：删除 `reload()` 的 askGuard 检查 | ❌ 不能 | 无弹窗关闭后回归触发用例。 |
| `HooksConfigPanel.tsx`：删除 `handleJsonChange` 的 try/catch | ❌ 不能 | 无非法 JSON 传入 `onChange` 的用例。 |
| `HooksConfigPanel.tsx`：删除 `handleUninstall` 的 catch | ❌ 不能 | 无卸载失败用例。 |
| `HandlerForm.tsx`：删除清空 record/stringArray 后删键的分支 | ❌ 不能 | 仅测试了 number 字段清空。 |
| `HandlerForm.tsx`：`toJsonText(undefined)` 返回 `"null"` | ❌ 不能 | 未测试 undefined/null 输入分支。 |
| `GuiMode.tsx`：删除删除 matcher 组/handler 时的选中态重置 | ❌ 不能 | 测试未先选中再删除。 |
| `EventTree.tsx`：删除未知事件分组追加 | ❌ 不能 | 全部测试使用已知分组。 |
| `EventTree.tsx`：`formatHandlerSummary` agent 分支返回 `"agent:"` | ❌ 不能 | agent 分支 0 命中。 |
| `configModel.ts`：未知事件分组常量改为 `"其它"` | ✅ 能 | model 测试断言了 `"未知事件"`。 |
| `configModel.ts`：删除 `isSltermManaged` 的 command 子串检查 | ✅ 能 | handlerform/gui 测试均会红。 |
| `matcherEngine.ts`：删除非法正则防御 | ✅ 能 | matcher 测试有直接用例。 |
| `eventsCatalog.ts`：交换 A/B 事件档 | ✅ 能 | catalog 全量断言会红。 |
| `pageApis.ts`：`openHooksConfigPanel` 单例 id 规则改错 | ✅ 能 | open-hooks-config-panel 测试会红。 |

**结论**：约 40% 的关键行为变异无法被当前 L2 测试捕获，主要集中在 JsonMode linter 顺序、`useHooksConfig` 竞态、非法 JSON 容错、HandlerForm/GuiMode 边界分支。

## 稳定性风险评估

| 风险点 | 等级 | 说明 |
|---|---|---|
| `useHooksConfig` 竞态无回归 | 高 | 快速切层/切项目可能显示错误层配置。 |
| JsonMode linter 顺序无守卫 | 高 | 交换后语法/schema 校验行为异常，测试无法发现。 |
| 非法 JSON catch 无回归 | 中 | 用户编辑过程中出现非法 JSON 时组件可能崩溃。 |
| 固定短延时等待 | 低 | `hooks-config-panel.test.tsx` 使用 20ms 等待，jsdom 下单测通常稳定。 |
| Mock `JsonMode`/`GuiMode` 范围 | 低 | panel/sync 测试用 mock 组件隔离下层，符合项目规范。 |

## 总体评价

- **优势**：纯函数层（catalog/matcher/model）覆盖扎实，常量守卫能有效防止回归；`open-hooks-config-panel` 对 C13-7 同页单例语义覆盖完整；`HandlerForm` 字段矩阵与注入段禁改测试较厚；测试 mock 策略与 `vi.hoisted()` 使用符合项目规范。
- **主要短板**：
  1. **JsonMode linter 顺序未锁定**（R1）—— 关键编辑器校验行为缺少结构性断言；
  2. **`useHooksConfig.load()` generation 竞态取消零覆盖**（R2）—— 数据一致性风险；
  3. **非法 JSON 进入 `HooksConfigPanel.handleJsonChange` 的容错路径无回归**（R3）—— 崩溃风险。
- **建议优先级**：先补 R1-R3；再补 Y1-Y6；G1-G3 可延后。

## 总结

本次评审 10 个测试文件、inventory 口径 205 条用例，发现 **3 条红色、6 条黄色、3 条绿色** 问题。

**TOP 3 问题**：

1. **`hooks-config-jsonmode.test.tsx` 未锁定 linter 包装顺序**（R1）。这是最严重的结构性断言缺失，交换两个 linter 会导致校验行为严重异常但测试仍绿。
2. **`useHooksConfig.load()` 的 generation 竞态取消无测试**（R2）。快速切层/切项目时过期结果可能覆盖当前层配置，存在数据错层风险。
3. **`HooksConfigPanel.handleJsonChange` 对非法 JSON 的 catch 无回归**（R3）。用户编辑过程中产生的非法 JSON 可能让组件崩溃，而当前测试全部只传合法 JSON。

**建议下一步**：优先在 JsonMode 测试中追加 linter 身份断言；在 `useHooksConfig` 相关测试中补充 generation 竞态用例；在 panel 测试中补充非法 JSON 容错用例；随后补齐 HandlerForm 清空、GuiMode 选择态重置、EventTree 未知分组等黄色项。
