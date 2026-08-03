# 12 L2 快捷键+主题+Store 测试 Review

## 元信息

- **领域**：L2 前端测试——快捷键/命令系统 + 主题/配色/基础 + Store 状态管理
- **测试文件数**：17
- **用例数**：303（快捷键/命令 114、主题/配色/基础 108、Store 81）
- **审查日期**：2026-08-04
- **覆盖率概况**（来自 `extract-uncovered.cjs`）：
  - `src/features/shortcuts/ShortcutRegistry.ts`：行覆盖 100%，但 `forceContext` 平局 tie-breaker 有分支未覆盖（240/241 两路未全走）。
  - `src/lib/claudeStatus.ts`：行覆盖 100%，但 `getStatusIcon` 的 `status === null` 分支未覆盖（24/25 两路未全走）。
  - `src/lib/ErrorBoundary.tsx`：行覆盖 90.9%，`inline` 变体分支未覆盖（不在本次 17 个测试文件内）。
  - `src/stores/fontSize.ts`：行覆盖 92.0%，`cancelPendingSave` 在 timer 存活时的分支未覆盖（84-85）。
  - `src/stores/keybindings.ts`：行覆盖 92.0%，`cancelPendingSave` 在 timer 存活时的分支未覆盖（88-89）。
  - `src/stores/projects.ts`：行覆盖 100%，但 `renamePage` 对不存在 `projectId` 的守卫分支未覆盖（178）。
  - `src/stores/sideBar.ts`：行覆盖 92.8%，`clamp` 非有限数回退分支（59）与 `cancelPendingSave` 活跃 timer 分支（146-147）未覆盖。
  - `src/theme/` 脚本未输出未覆盖行，但人工审查发现 `terminalOptions.vtExtensions.kittyKeyboard` 与部分 token 未在测试中显式断言。

## 覆盖率缺口

按业务风险分级：

### 🔴 核心逻辑零覆盖

- `src/__tests__/colors.test.ts` 大量表驱动用例只断言测试文件内的硬编码字面量，未比对 `colors.ts` 实际导出值（详见 P1）。

### 🟡 边界分支未覆盖

- `src/features/shortcuts/ShortcutRegistry.ts:240-241`：`forceContext` 平局排序的反向分支未覆盖。
- `src/lib/claudeStatus.ts:24-25`：`getStatusIcon(null)` 返回 `""` 的分支未覆盖。
- `src/panels/terminal/theme.ts:43`：`vtExtensions: { kittyKeyboard: true }` 未在 `theme.test.ts` 中显式断言。
- `src/stores/fontSize.ts:84-85`、`src/stores/keybindings.ts:88-89`、`src/stores/sideBar.ts:146-147`：`cancelPendingSave` 在已有待保存 timer 时的清理分支未覆盖。
- `src/stores/projects.ts:178`：`renamePage` 对不存在 `projectId` 的守卫分支未覆盖。
- `src/stores/sideBar.ts:59`：`clamp` 对非有限数回退 `min` 的分支未覆盖。

### 🟢 低风险未覆盖

- `src/lib/ErrorBoundary.tsx:51-139`：`inline` 变体渲染分支未在本次测试文件内覆盖。
- `src/theme/colors.ts`：`EXPLORER_SELECTION_BG`、`HTML_PANEL_LOADING_FG`、`HTML_PANEL_IFRAME_BG` 未在 `colors.test.ts` 中校验。

## 问题列表

### P-1 [🔴] [断言有效性] colors.test.ts 表驱动用例对 GIT_FILE_COLORS / GIT_GUTTER_COLORS / EXPLORER_COLORS / SIDEBAR_COLORS 只断言测试常量自身

- **位置**：
  - `src/__tests__/colors.test.ts:61-67`（GIT_FILE_COLORS）
  - `src/__tests__/colors.test.ts:81-87`（GIT_GUTTER_COLORS）
  - `src/__tests__/colors.test.ts:104-109`（EXPLORER_COLORS）
  - `src/__tests__/colors.test.ts:125-130`（SIDEBAR_COLORS 部分）
  - `src/__tests__/colors.test.ts:218-227`（AGENT_STATUS_USAGE_COLORS 中只有 `expected` 的 regex 断言，缺少实际值比对）
- **代码片段**：
  ```ts
  it.each(cases)(
    "$key 值为合法 6 位 hex ($expected)",
    ({ expected }: { expected: string }) => {
      expect(expected).toMatch(HEX6_RE);
    },
  );
  ```
- **问题**：`expected` 是测试文件里硬编码的字面量，不是从 `colors.ts` 导入的实际 token 值。这类断言无论源码怎么改都会绿，无法发现 token 漂移、拼写错误或被意外覆盖。
- **改法**：改为读取被测对象的实际值，例如：
  ```ts
  it.each(cases)("$key 值与预期一致", ({ key, expected }) => {
    expect(GIT_FILE_COLORS[key as keyof typeof GIT_FILE_COLORS]).toBe(expected);
  });
  ```
- **变异推演**：把 `src/theme/colors.ts` 中的 `GIT_FILE_COLORS.modified` 改成 `"#000000"`，`colors.test.ts` 中针对 `modified` 的用例仍然通过（因为它在断言字面量 `"#6897BB"` 是否符合 hex），**变红失败**无法发生。

---

### P-2 [🟡] [断言有效性] global-commands.test.ts 用例名与断言不符

- **位置**：`src/__tests__/global-commands.test.ts:166-174`
- **代码片段**：
  ```ts
  it("getDockviewApi 抛异常时 handler 不传播（由 ShortcutRegistry 调用链吞错）", () => {
    const cmds = createGlobalShortcuts(() => {
      throw new Error("unexpected");
    });

    // handler 本身不吞错——如果 getDockviewApi 抛异常，应由 ShortcutRegistry 调用链处理。
    // 此测试仅验证命令对象可正常创建。
    expect(cmds[0]).toBeDefined();
  });
  ```
- **问题**：用例名称声称验证“handler 不传播异常”，但实际只创建了命令对象、从未调用 `cmds[0].handler(event)`。它无法验证 ShortcutRegistry 是否真的吞掉了异常，也无法验证 handler 自身行为。
- **改法**：要么删除该用例，要么真正触发 handler 并断言不抛异常；同时建议改名，例如“factory 在 getter 抛异常时仍能创建命令对象”。
- **变异推演**：若 `createGlobalShortcuts` 的 handler 把 `getDockviewApi()` 调用包在 `try/catch` 里返回 `false`，或让它直接向上抛，当前用例都**不会变红**，因为 handler 从未被执行。

---

### P-3 [🟡] [覆盖度] ShortcutRegistry `forceContext` 平局 tie-breaker 的反向分支未覆盖

- **位置**：`src/features/shortcuts/ShortcutRegistry.ts:236-242`
- **代码片段**：
  ```ts
  matching.sort((a, b) => {
    if (a.priority !== b.priority) return b.priority - a.priority;
    if (forceContext) {
      const aForced = a.context === forceContext ? 1 : 0;
      const bForced = b.context === forceContext ? 1 : 0;
      return bForced - aForced;
    }
    return this.contextStack.lastIndexOf(b.context) - this.contextStack.lastIndexOf(a.context);
  });
  ```
- **问题**：`shortcuts.test.ts` 中 `resolve` 的“forceContext 平手时优先于 global”用例只覆盖了 `a=terminal, b=global` 这一种顺序（240 路 1/0、241 路 0/1）。如果注册顺序反过来（`a=global, b=terminal`），`aForced=0, bForced=1` 分支未跑，覆盖率脚本也显示这两路未全走。
- **改法**：补充一个注册顺序为 `global` 在前、`terminal` 在后、`forceContext="terminal"` 的用例，确保两种排序方向都覆盖。
- **变异推演**：若把 `return bForced - aForced` 误写成 `return aForced - bForced`，当前测试会红（已覆盖的方向）；但若把 `a.context === forceContext` 误写成 `a.context !== forceContext`，则只有在 `global` 在前时才会暴露，当前用例**不会变红**。

---

### P-4 [🟡] [覆盖度] `getStatusIcon` 的 `null` 分支未覆盖

- **位置**：`src/lib/claudeStatus.ts:23-26`
- **代码片段**：
  ```ts
  export function getStatusIcon(status: ClaudeStatus): string {
    if (status === null) return "";
    return STATUS_EMOJI[status] ?? "";
  }
  ```
- **问题**：`claude-status.test.ts` 测试了 `eventToStatus` 和 `STATUS_EMOJI` 常量，但没有调用 `getStatusIcon`。`status === null` 时返回空字符串这一展示层关键路径未验证。
- **改法**：增加两个用例：
  ```ts
  expect(getStatusIcon(null)).toBe("");
  expect(getStatusIcon("working")).toBe("⚡");
  ```
- **变异推演**：如果把 `if (status === null) return ""` 改成返回 `"?"` 或直接删除导致 `STATUS_EMOJI[null]` 索引，当前所有用例都**不会变红**。

---

### P-5 [🟡] [覆盖度/断言一致性] theme.test.ts 未断言 `kittyKeyboard`，与测试清单描述不一致

- **位置**：
  - 被测：`src/panels/terminal/theme.ts:43`
  - 测试：`src/__tests__/theme.test.ts`
- **代码片段**：
  ```ts
  vtExtensions: { kittyKeyboard: true },
  ```
- **问题**：`.claude/test-inventory.md` 在主题/配色/基础章节明确列出 `terminalOptions` 覆盖 `kittyKeyboard`，但 `theme.test.ts` 中没有任何断言检查 `vtExtensions.kittyKeyboard`。若该配置被误删或改为 `false`，测试仍全绿。
- **改法**：增加：
  ```ts
  expect(terminalOptions.vtExtensions?.kittyKeyboard).toBe(true);
  ```
- **变异推演**：把 `theme.ts` 中的 `kittyKeyboard: true` 改成 `false` 或直接删除 `vtExtensions` 键，`theme.test.ts` **不会变红**。

---

### P-6 [🟡] [稳定性风险] Store debounce 测试未在 afterEach 清理活跃 timer

- **位置**：
  - `src/__tests__/projects.test.ts:580-668`
  - `src/__tests__/font-size.test.ts:33-35`
  - `src/__tests__/keybindings.test.ts:29-31`
- **代码片段**（以 projects 为例）：
  ```ts
  afterEach(() => {
    clearMocks();
  });
  ```
- **问题**：`markPersistenceReady()` 后触发变更会设置 2s 的 `setTimeout`。测试用 `vi.useFakeTimers()` 并在末尾调用 `vi.useRealTimers()`，但没有调用对应 `cancelPendingSave()`。测试之间若执行变慢，残留的 timer 回调可能在下一个 `beforeEach` 的 `_resetPersistence()` 之前触发，导致不可预期的 `save_projects` 调用或状态写入。
- **改法**：在 `afterEach` 中调用 `cancelPendingSave()`（或 `vi.runOnlyPendingTimers()` + `vi.clearAllTimers()`），与生产代码的关窗清理策略一致。
- **变异推演**：这是一个稳定性问题，非功能变异；在 CI 高负载或单测顺序调整后，残留 timer 可能导致偶发性失败，但当前测试顺序下可能不显现。

---

### P-7 [🟡] [用例设计质量] projects.test.ts 部分守卫用例 codify 了可疑行为

- **位置**：
  - `src/__tests__/projects.test.ts:179-189`（`updatePageLayout` 对不存在 pageId 仍递增 version）
  - `src/__tests__/projects.test.ts:358-371`（`removePage` 对不存在 pageId 仍递增 version）
- **代码片段**（以 updatePageLayout 为例）：
  ```ts
  expect(updated.pages[0].layout).toEqual(page.layout);
  expect(updated.version).toBe(6);
  ```
- **问题**：这些用例把“操作目标不存在时仍递增 version”作为期望行为锁定下来。虽然源码确实如此实现，但这会让后续若想把“无影响操作不 bump version”的优化无法推进，因为测试会红。
- **改法**：若业务上接受当前行为，应在测试注释中说明这是“已知当前行为”；否则应调整源码并同步修改断言。建议至少不要把这类边缘行为作为强契约测试。

---

### P-8 [🟢] [用例设计质量] command-catalog.test.ts 的 `commandFromMeta` 只覆盖了 9 条命令中的 5 条

- **位置**：`src/__tests__/command-catalog.test.ts:76-135`
- **问题**：`commandFromMeta` 的测试只验证了 `editor.save`、`explorer.delete/open/rename`、`editor.toggleWordWrap`，未覆盖 `terminal.copy/paste/newline`、`global.closeTab`。虽然元数据表测试已覆盖 defaultKey，但 `commandFromMeta` 的合并路径并未全部走通。
- **改法**：将 `commandFromMeta` 测试改为参数化循环，遍历 `EXPECTED_IDS` 中全部命令，统一断言 `id/context/defaultKey/handler`。

---

### P-9 [🟢] [覆盖度] colors.test.ts 缺少 `EXPLORER_SELECTION_BG` 等 token 校验

- **位置**：
  - 导出：`src/theme/index.ts:23-28`
  - 测试：`src/__tests__/colors.test.ts`
- **问题**：`EXPLORER_SELECTION_BG`、`HTML_PANEL_LOADING_FG`、`HTML_PANEL_IFRAME_BG` 均从 `theme/index.ts` 导出，但 `colors.test.ts` 的“通用 UI 色”表中没有它们。若这些 token 被误改，测试无法发现。
- **改法**：将上述 token 加入 `uiTokenCases` 或新增独立断言。

---

### P-10 [🟢] [覆盖度] `cancelPendingSave` 的活跃 timer 分支未覆盖

- **位置**：
  - `src/stores/fontSize.ts:82-85`
  - `src/stores/keybindings.ts:85-89`
  - `src/stores/sideBar.ts:143-149`
- **问题**：现有 Store 测试只在 timer 为空时调用 `cancelPendingSave`，未验证它真的能取消一个已排队的保存。覆盖率脚本也显示这些行未被覆盖。
- **改法**：在 `loaded=true` 状态下触发一次变更，不推进到 2s，调用 `cancelPendingSave()`，再推进 2s，断言 `saveSettings` / `save_projects` 未被调用。

---

### P-11 [🟢] [覆盖度] projects.ts `renamePage` 对不存在 projectId 的守卫未覆盖

- **位置**：`src/stores/projects.ts:175-179`
- **代码片段**：
  ```ts
  renamePage: (projectId, pageId, newName) =>
    set((state) => {
      const project = state.projects[projectId];
      if (!project) return state;
      ...
    }),
  ```
- **问题**：`projects.test.ts` 测试了“不存在的 pageId”，但没有测试“不存在的 projectId”。该早期返回分支未被覆盖。
- **改法**：增加一个用例：对不存在的 projectId 调用 `renamePage`，断言 `projects` 状态不变、version 不变。

---

### P-12 [🟢] [结构与可维护性] 多个测试用例名称与断言不一致

- **位置**：
  - `src/__tests__/global-commands.test.ts:166-174`（同 P-2）
  - `src/__tests__/projects.test.ts:339-342`：`markPersistenceReady 应允许后续 save 操作` 只断言 `not.toThrow`，没有验证后续 save 是否被允许。
- **问题**：用例名称承诺的行为未在断言中落实，后续维护者会误以为已有覆盖。
- **改法**：改名以匹配实际断言，或补充断言。

---

### P-13 [🟢] [稳定性风险] inject-script.test.ts 用真实耗时做性能断言

- **位置**：`src/__tests__/inject-script.test.ts:222-228`
- **代码片段**：
  ```ts
  const start = Date.now();
  const result = injectScript(big, SCRIPT, MARKER);
  const elapsed = Date.now() - start;
  expect(result).toContain(SCRIPT);
  expect(result).toContain("x".repeat(500_000));
  expect(elapsed).toBeLessThan(500);
  ```
- **问题**：`elapsed < 500ms` 依赖 CI 机器负载，可能在资源紧张时抖动失败；且不是核心功能断言。
- **改法**：删除时间断言，保留“不抛异常、结果包含脚本与原始内容”的断言即可；如需性能回归，应放专门基准测试。

---

### P-14 [🟢] [用例设计质量] projects.test.ts 对“状态不变”的断言使用同引用快照

- **位置**：`src/__tests__/projects.test.ts:173-177`、`290-300`
- **代码片段**：
  ```ts
  const stateBefore = useProjects.getState().projects;
  useProjects.getState().updatePageLayout("nonexistent-proj", page.pageId, { x: 1 });
  expect(useProjects.getState().projects).toEqual(stateBefore);
  ```
- **问题**：`stateBefore` 是操作前对象引用。若实现意外返回了新的但内容相同的对象，`toEqual` 会通过；若实现mutate了原对象，`toEqual` 也会通过（因为比较的是值，且 `stateBefore` 也被 mutate）。严格来说没有真正验证不可变性。
- **改法**：使用 `structuredClone(stateBefore)` 或深度拷贝，再与操作后状态 `toEqual` 比对。

## 已做变异推演的用例清单

下表列出本次审查中对核心行为用例做的“改错会不会变红”推演：

| 被测文件 | 测试文件/用例 | 假设的篡改 | 当前用例是否变红 | 说明 |
|---|---|---|---|---|
| ShortcutRegistry.ts | `shortcuts.test.ts` 同按键不同 priority | `return a.priority - b.priority` | 是 | low 会优先执行，与期望 `["high"]` 冲突 |
| ShortcutRegistry.ts | `shortcuts.test.ts` 同 priority 上下文栈顶优先 | `lastIndexOf` 改为升序 | 是 | terminal 会赢，与期望 `editor` 冲突 |
| ShortcutRegistry.ts | `shortcuts.test.ts` forceContext 平手 | 删除 tie-breaker 或反向 | 是 | global 会赢，当前用例可发现 |
| ShortcutRegistry.ts | `shortcuts.test.ts` setOverrides 重绑 | `effectiveKeystroke` 不重建索引 | 是 | 旧默认键仍命中，与期望冲突 |
| ShortcutRegistry.ts | `shortcuts.test.ts` 保留键覆盖 | `isReserved` 恒返回 false | 是 | Ctrl+C 会被绑定并消费 |
| ShortcutRegistry.ts | `shortcuts.test.ts` IME 透传 | 移除 `isComposing` 守卫 | 是 | 默认命令会消费事件 |
| claudeStatus.ts | `claude-status.test.ts` SessionStart → attention | 改为 `working` | 是 | 明确断言 `attention` |
| claudeStatus.ts | `claude-status.test.ts` Notification attention 子类型 | 从 attention 集合移除 `permission_prompt` | 是 | 该用例会失败 |
| claudeStatus.ts | `claude-status.test.ts` 未识别事件 → null | default 分支返回 `"unknown"` | 是 | 多个用例断言 `null` |
| keystroke.ts | `keystroke.test.ts` format∘parse 恒等 | format 输出修饰键倒序 | 是 | 往返结果不匹配 |
| path.ts | `path.test.ts` 同一前缀但不是子目录 | 去掉末尾斜杠处理 | 是 | `D:/project-extra` 会误判为子目录 |
| injectScript.ts | `inject-script.test.ts` 宿主 `</script>` 转义 | 删除 `escapeScriptClose` | 是 | hostPart 仍含 `</script>` |
| projects.ts | `projects.test.ts` M2.2 debounce | 移除 `initialized` 守卫 | 是 | markPersistenceReady 前的变更也会触发保存 |
| fontSize.ts | `font-size.test.ts` loadFromDisk 超限 clamp | 移除 clamp | 是 | 保存值 100 会直接进入 state |
| keybindings.ts | `keybindings.test.ts` sanitize 脏值 | 移除 sanitize | 是 | 对象/数字会保留在 overrides 中 |
| globalCommands.ts | `global-commands.test.ts` 活跃面板关闭 | handler 不返回 true | 是 | 断言 `result` 为 true 会失败 |

未通过变异推演、存在“改错也抓不到”风险的核心用例：
- `colors.test.ts` 的 GIT_FILE_COLORS / GIT_GUTTER_COLORS / EXPLORER_COLORS / SIDEBAR_COLORS 表驱动用例（断言测试字面量，不读源码）。
- `global-commands.test.ts` “getDockviewApi 抛异常时 handler 不传播”用例（未调用 handler）。
- `theme.test.ts` 未覆盖 `kittyKeyboard` 的隐含断言。
- `claude-status.test.ts` 未覆盖 `getStatusIcon(null)` 分支。
