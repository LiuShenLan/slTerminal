# 章四「前端架构」修复清单（CP-016/017/018/019/020/021/022/031/036/037/039/042）

起草日期：2026-09-07。所有「位置/现状」均经实读代码原文核对；与 `docs/compromises.md` 章四登记有出入处以现状为准并在「起草附注」留痕。

---

## CP-016 · 侧栏视图换区重建丢状态——视图状态上移注册表状态槽 [Stage 07]

1. **位置**：
   - `src/features/sideViews/SideBarArea.tsx:97-141`（上下 pane 条件渲染视图，`:111-114`/`:134-137` 为 `<def.component>` 两处实例）
   - `src/features/sideViews/sideViewRegistry.ts:22-31`（`SideViewDef` / `SideViewComponentProps`）、`:34-56`（注册表类）
   - `src/features/explorer/useFileTree.ts:36`（`rootNodes` 组件内 state，展开态随卸载丢失）
   - `src/features/explorer/ExplorerPanel.tsx:43,63`（`React.FC` 无 props，`:63` 调 `useFileTree({ rootPath })`）

2. **现状**：
   - SideBarArea.tsx:5-6 头注释：「切换即卸载旧视图组件——状态丢失语义 ADR-0001 已接受……换区重建亦为已知行为」；`:103-116` 上 pane `topDefs.filter((def) => def.id === open.top)` 条件渲染。
   - sideViewRegistry.ts:34-56 注册表仅 `defs` 一张 Map，`_reset()` 只清 defs。
   - useFileTree.ts:36 `const [rootNodes, setRootNodes] = useState<TreeNode[]>([])`；`:109-158` `toggleExpand` 展开态只写进 rootNodes。
   - SideViewComponentProps（sideViewRegistry.ts:14-19）只有 `switchToPage` / `onDeletePage` 两字段。

3. **修复步骤**：
   1. `sideViewRegistry.ts`——`SideViewComponentProps` 追加两个可选槽位：
      ```ts
      /** 侧栏视图组件的 props——与 SidebarTree props 精确匹配 */
      export interface SideViewComponentProps {
        /** 切换到指定操作页面（async——切换完成后再开面板） */
        switchToPage: (projectId: string, pageId: string) => Promise<void>;
        /** 删除指定操作页面 */
        onDeletePage: (projectId: string, pageId: string) => void;
        /** 视图恢复状态（CP-016：槽位切换/换区重建后由注册表状态槽回填；无历史状态则 undefined） */
        viewState?: unknown;
        /** 视图状态上呼（组件内部状态变化时持久化；模块级存活，跨挂载不丢） */
        onViewStateChange?: (state: unknown) => void;
      }
      ```
   2. `sideViewRegistry.ts`——`SideViewRegistry` 类新增状态槽（`_reset` 同步清理）：
      ```ts
      /** 侧栏视图注册表——模块级单例 */
      export class SideViewRegistry {
        private defs: Map<string, SideViewDef> = new Map();
        /** 视图状态槽（CP-016）：以视图 id 为键持有跨挂载状态——与 defs 同生命周期（模块级） */
        private viewStates: Map<string, unknown> = new Map();

        // register / getAll / get 不变……

        /** 读取视图恢复状态（无条目返回 undefined） */
        getViewState<T>(id: string): T | undefined {
          return this.viewStates.get(id) as T | undefined;
        }

        /** 写入视图状态（组件经 onViewStateChange 上呼；同 id 覆盖） */
        setViewState(id: string, state: unknown): void {
          this.viewStates.set(id, state);
        }

        /** 清空所有定义 + 视图状态（仅测试用） */
        _reset(): void {
          this.defs.clear();
          this.viewStates.clear();
        }
      }
      ```
   3. `SideBarArea.tsx`——两处 `<def.component …>` 统一改为受控消费（`:111-114` 与 `:134-137` 同改）：
      ```tsx
                  <def.component
                    switchToPage={switchToPage}
                    onDeletePage={onDeletePage}
                    viewState={sideViewRegistry.getViewState(def.id)}
                    onViewStateChange={(state) =>
                      sideViewRegistry.setViewState(def.id, state)
                    }
                  />
      ```
      同时改写文件头注释 :4-6：删除「状态丢失语义 ADR-0001 已接受（导航树滚动位置等轻状态不保活）；换区重建亦为已知行为」两句，替换为：
      ```
      // 每半区一槽位，视图经条件渲染切换（FE-21）。视图跨挂载状态（展开集等）上移
      // sideViewRegistry 状态槽（CP-016）——以视图 id 为键，组件经 viewState/onViewStateChange
      // 受控消费；换区/槽位切换重建后由回填恢复，不再依赖组件内部 state。
      ```
   4. `useFileTree.ts`——展开态真值源外移（snapshot 以 rootPath 为域键，项目间不复用）。`UseFileTreeOptions` 改：
      ```ts
      /** 侧栏视图持久状态（CP-016——expandedPaths 真值源上移 sideViewRegistry 状态槽） */
      export interface FileTreeViewState {
        /** 快照所属根路径（与当前 rootPath 不一致则整份作废） */
        rootPath: string | null;
        /** 已展开目录路径集合（提交时自 rootNodes 树遍历派生） */
        expandedPaths: string[];
      }

      interface UseFileTreeOptions {
        rootPath: string | null;
        /** 注册表回填的恢复状态（ExplorerPanel 自 SideViewComponentProps.viewState 透传；无则 undefined） */
        viewState?: unknown;
        /** 状态变化上呼（透传 SideViewComponentProps.onViewStateChange） */
        onViewStateChange?: (state: unknown) => void;
      }
      ```
      hook 内新增（放在 `genRef` 声明之后）：
      ```ts
      // CP-016：恢复状态只在挂载后首次加载完成时消费一次（ref 快照，不入 deps——
      // 与 initialDocRef 同模式，避免回填 doc 类值变化触发无谓重建）
      const viewStateRef = useRef<FileTreeViewState | undefined>(
        (viewState as FileTreeViewState | undefined) ?? undefined,
      );
      const onViewStateChangeRef = useRef(onViewStateChange);
      onViewStateChangeRef.current = onViewStateChange;
      /** 恢复展开期间置位——抑制逐层提交（恢复完成一次性提交） */
      const restoringRef = useRef(false);
      ```
      新增提交与恢复两个闭包（放在 `refreshSubtreeAt` 之后、rootPath effect 之前）：
      ```ts
      /** 自当前 rootNodes 树遍历派生展开集并上呼（restoring 期间不提交） */
      const commitViewState = useCallback(() => {
        if (restoringRef.current) return;
        const expanded: string[] = [];
        const walk = (nodes: TreeNode[]) => {
          for (const n of nodes) {
            if (n.expanded && n.entry.isDir) {
              expanded.push(n.entry.path);
              walk(n.children);
            }
          }
        };
        walk(rootNodesRef.current);
        onViewStateChangeRef.current?.({
          rootPath: rootPathRef.current,
          expandedPaths: expanded,
        });
      }, []);

      /** 树中是否含指定路径节点（恢复前存在性守卫——磁盘已删目录不发起无谓 readDir） */
      const hasPath = useCallback((path: string): boolean => {
        const walk = (nodes: TreeNode[]): boolean =>
          nodes.some((n) => n.entry.path === path || walk(n.children));
        return walk(rootNodesRef.current);
      }, []);

      /** 恢复展开态（CP-016）：快照 rootPath 与当前一致才恢复；浅→深逐层 toggleExpand，
       *  复用其「展开+异步加载子目录」逻辑；全程 restoringRef 抑制提交 */
      const restoreExpanded = useCallback(async () => {
        const stored = viewStateRef.current;
        if (!stored) return;
        if (stored.rootPath !== rootPathRef.current) return;
        const paths = [...stored.expandedPaths].sort(
          (a, b) => a.length - b.length,
        );
        if (paths.length === 0) return;
        restoringRef.current = true;
        try {
          for (const p of paths) {
            if (hasPath(p)) await toggleExpand(p);
          }
        } finally {
          restoringRef.current = false;
        }
        commitViewState();
      }, [toggleExpand, hasPath, commitViewState]);
      ```
      rootPath 变更 effect（useFileTree.ts:331-359）的 `loadRoot(gen);` 之后追加恢复调用：
      ```ts
        loadRoot(gen).then(() => {
          if (gen !== genRef.current) return; // rootPath 已变化，丢弃过期恢复
          void restoreExpanded();
        });
      ```
      `toggleExpand`（:109-158）末尾追加提交（restoring 期间 commitViewState 自身短路，无需条件）：
      ```ts
        // toggleExpand 函数体末尾（第二个 setRootNodes 之后）追加：
        commitViewState();
      ```
      `reloadPreservingExpanded`（:162-190）`setRootNodes(next);` 之后追加 `commitViewState();`（磁盘删除已展开目录后展开集同步收缩）。
   5. `ExplorerPanel.tsx`——接 props 并透传（`:43` 与 `:63`）：
      ```tsx
      export const ExplorerPanel: React.FC<SideViewComponentProps> = ({
        viewState,
        onViewStateChange,
      }) => {
        // ……
        const { rootNodes, gitStatusMap, rootError, toggleExpand, refresh } = useFileTree({
          rootPath,
          viewState,
          onViewStateChange,
        });
      ```
      （`switchToPage` / `onDeletePage` ExplorerPanel 现状未消费，保持不析取。）

4. **测试同步**：
   - 改 `src/__tests__/sideViewRegistry.test.ts`：新增用例组「CP-016 视图状态槽」——`setViewState/getViewState 同 id 覆盖`、`getViewState 无条目返回 undefined`、`_reset 清空状态槽`。
   - 改 `src/__tests__/use-file-tree.test.ts`：新增「CP-016 恢复」用例组——① 传入 `viewState={rootPath 匹配, expandedPaths:[...]}` → 初始加载后对应目录自动展开且子节点已加载；② `viewState.rootPath` 与当前不符 → 不恢复；③ `toggleExpand` 后 `onViewStateChange` 被调且 `expandedPaths` 含新展开路径、折叠后移除；④ 磁盘删除已展开目录经 `triggerFsEvent` 刷新后提交集中该路径消失。既有用例适配：`renderHook(useFileTree)` 全部调用点补传 `viewState: undefined, onViewStateChange: undefined`（可选 prop，默认 undefined，现有断言不动）。
   - 加 `src/__tests__/sidebar-area-viewstate.test.tsx`（新）：SideBarArea 渲染注册表 mock 视图，断言 `viewState`/`onViewStateChange` 两 prop 透传到视图组件；`onViewStateChange` 回调后注册表 `getViewState(id)` 可读回（即换区重建后可回填）。
   - SideBarArea 既有视图槽条件渲染用例（若断言「换区后旧区卸载」）保持通过——渲染形态不变，状态由回填恢复。

5. **文档同步**：
   - `src/features/sideViews/CLAUDE.md`「关闭语义——按需卸载（FE-21）+ 换区重建」节：删「ADR-0001 已确认接受」的丢状态口径，改为「槽位切换/换区仍按需卸载重建（FE-21 不变），但视图状态经 sideViewRegistry 状态槽（getViewState/setViewState，`_reset` 同清）以视图 id 为键持久——组件经 SideViewComponentProps.viewState/onViewStateChange 受控消费」；「外部坑/红线」中「换区重建丢失状态」「FE-21 隐藏视图卸载」两条同步修订。
   - `src/features/explorer/CLAUDE.md`「宿主变更（ADR-0001）」节「已知行为：换区重建丢失展开状态」整段删除，替换为展开态槽位契约（FileTreeViewState 结构、rootPath 域键、提交/恢复时机）；「测试模式」补 CP-016 恢复用例组说明。

6. **验证**：
   - `npx eslint src/` 退出码 0；`npx tsc --noEmit` 退出码 0。
   - `npx vitest run sideViewRegistry use-file-tree sidebar-area` 全绿。
   - `grep -n "getViewState" src/features/sideViews/SideBarArea.tsx` 有 2 处命中（上下 pane 各一）。
   - `grep -c "commitViewState" src/features/explorer/useFileTree.ts` ≥ 4（定义 + toggleExpand + reloadPreservingExpanded + restoreExpanded）。

---

## CP-017 · settings dirty 真值源脱离壳生命周期 + settings 纳入 always-render [Stage 07]

> 与 CP-036 同 agent（共碰 `src/workspace/tabClose.ts`、`src/features/settingsCenter/dirtyRegistry.ts`）；本条先做真值源/保活，CP-036 做批量收口。

1. **位置**：
   - `src/panels/settings/SettingsPanel.tsx:331-338`（壳挂载注册 false / 卸载 clear——真值源生命周期绑死壳）
   - `src/workspace/pageApis.ts:150-155`（`openSettingsPanel` 的 addPanel 无 `renderer` 参数）
   - `src/panelRegistry.ts:104-117`（`isAlwaysRenderPanel` 白名单不含 settings，SC-FE-06 决策写死处）
   - `src/features/settingsCenter/dirtyRegistry.ts:9-25`（真值源本体，注释明说「dirty 只在壳实例存活期间有意义」）

2. **现状**：
   - SettingsPanel.tsx:333-337：`useEffect(() => { … setSettingsDirty(panelId, false); return () => clearSettingsDirty(panelId); }, [params?.panelId]);`
   - pageApis.ts:150-155：`api.addPanel({ id: panelId, component: "settings", title: "设置", params: {…} });`——无 `renderer`。
   - panelRegistry.ts:111-117：白名单 = terminal + htmlviewer + markdownviewer；:109 注释「editor / gitshow / diff 故意排除」。
   - dirtyRegistry.ts:5-6 头注释：「壳挂载注册（false）、卸载 clear；dirty 只在壳实例存活期间有意义——面板关闭/卸载即清除」。

3. **修复步骤**：
   1. `panelRegistry.ts`——白名单追加 settings（契约单点；SC-FE-06 口径翻案）：
      ```ts
      /**
       * 检查面板是否应使用 renderer="always" 模式。
       * 显式白名单：terminal（保持 PTY 存活）+ htmlviewer/markdownviewer（避免 iframe
       * browsing context 销毁重建导致白屏 + CM 编辑实例切走切回不重建——草稿与缩放
       * 状态保活，决策 #17）+ settings（CP-017：dirty 真值源脱离壳生命周期——壳不随
       * 页签切换卸载，dirtyMap/dirtyRegistry 条目跨切签存活）。
       * editor / gitshow / diff 故意排除——CM6 重建无视觉闪屏，且大文件编辑器若始终挂载会显著增加内存开销。
       */
      export function isAlwaysRenderPanel(type: string): boolean {
        return (
          type === PANEL_TERMINAL ||
          type === PANEL_HTML_VIEWER ||
          type === PANEL_MARKDOWN_VIEWER ||
          type === "settings"
        );
      }
      ```
   2. `pageApis.ts`——`openSettingsPanel` 的 addPanel 补 `renderer`（与 CP-042 重写后的版本合并落地，此处只加一行）：
      ```ts
        api.addPanel({
          id: panelId,
          component: "settings",
          title: "设置",
          renderer: "always", // CP-017：settings 纳入 always-render（isAlwaysRenderPanel 白名单同步）
          params: { panelId, ...(settingsPageId ? { selectedPage: settingsPageId } : {}) },
        });
      ```
   3. `SettingsPanel.tsx`——删除壳生命周期绑定 effect（:331-338 整段删除），替换为说明注释：
      ```ts
      // CP-017：dirty 真值源（dirtyRegistry）脱离壳生命周期——不再随壳挂载注册/卸载清除。
      // 条目生命周期收口到「确认丢弃关闭」动作点（tabClose.ts closeTabGuarded /
      // closeTabsGuarded 与壳内 SC-FE-08 项目切换守卫），壳只负责读写，不拥有条目。
      ```
   4. `tabClose.ts`——`closeTabGuarded` 确认分支补清除（单面板关闭入口，×/Ctrl+W/中键/右键四路共用）：
      ```ts
      export async function closeTabGuarded(
        api: { close(): void },
        panelId: string | undefined,
      ): Promise<void> {
        if (panelId?.startsWith("settings-") && isSettingsDirty(panelId)) {
          const ok = await confirmDialog({
            title: "未保存的修改",
            message: "当前配置页有未保存的修改，关闭将丢弃这些修改。",
            kind: "warning",
          });
          if (!ok) return;
          // CP-017：确认丢弃 = 真值源条目唯一清除点之一（壳卸载钩子已移除）
          clearSettingsDirty(panelId);
        }
        api.close();
      }
      ```
      （`import { isSettingsDirty, clearSettingsDirty } from "../features/settingsCenter/dirtyRegistry";`）
   5. `SettingsPanel.tsx`——SC-FE-08 项目切换守卫的确认分支（:383-400 一带，`ok` resolve 之后、`api.close()` 之前）同样补 `clearSettingsDirty(panelId);`；「初始评估静默关」分支不补（该分支只在刚挂载时触发，条目必不存在）。
   6. `dirtyRegistry.ts`——头注释改为：
      ```
      // 壳（SettingsPanel）与关闭守卫共享同一 dirty 真值源。CP-017 后条目生命周期
      // 脱离壳：不写挂载注册、不做卸载 clear；条目在「确认丢弃关闭」动作点清除
      // （tabClose.ts / SC-FE-08 守卫）。无条目 = 非 dirty。
      ```

4. **测试同步**：
   - 改 `src/__tests__/settings-panel-dirty.test.tsx`：删除/改写「壳卸载清除 dirtyRegistry 条目」类断言（该契约已删）；新增——壳卸载后（renderer always 场景=不切页不触发，模拟直接 unmount）`isSettingsDirty` 仍返回 true（真值源脱离壳生命周期）。
   - 改 `src/__tests__/tab-close.test.ts`：新增「确认关闭 dirty settings 面板后 isSettingsDirty(panelId) 为 false」；「取消关闭后条目仍在」。
   - 改 `src/__tests__/workspace-file-panel-types.test.ts`：`isAlwaysRenderPanel("settings")` 期望 `true`（:67-87 用例组追加一条）。
   - 改 `src/__tests__/open-settings-panel.test.ts`：addPanel 参数断言补 `renderer: "always"`。
   - `settings-panel-autoclose.test.tsx`：SC-FE-08 确认分支现有断言保持；若其 mock 了 unmount-clear 语义则按新契约适配。

5. **文档同步**：
   - `src/workspace/CLAUDE.md`「面板注册表已提取到 `src/panelRegistry.ts`」节：`isAlwaysRenderPanel` **不含 settings**（决策写死，SC-FE-06）段整段改写——「settings 已纳入 renderer="always"（CP-017）：dirty 真值源脱离壳生命周期，壳随页签切换保挂载」。
   - `src/features/settingsCenter/CLAUDE.md`「dirtyRegistry 真值源（SC-FE-07）」节：「壳挂载注册 false、卸载 clear」改为「条目生命周期收口到确认丢弃关闭动作点；壳不拥有条目」。
   - `src/panels/CLAUDE.md` settings 节「isAlwaysRenderPanel 不加入 settings（决策写死，SC-FE-06）」句删除并替换为 CP-017 口径。

6. **验证**：
   - `grep -n "renderer" src/workspace/pageApis.ts` 命中 addPanel 一处 `"always"`。
   - `grep -n "clearSettingsDirty" src/workspace/tabClose.ts` ≥ 1。
   - `grep -c "setSettingsDirty(panelId, false)" src/panels/settings/SettingsPanel.tsx` = 0（挂载注册已删）。
   - `npx vitest run settings-panel-dirty tab-close open-settings-panel workspace-file-panel-types` 全绿。

---

## CP-018 · WebGL 检测区分 SwiftShader 并一次性 toast 降级提示 [Stage 08]

1. **位置**：`src/panels/terminal/webgl.ts:34-44`（`detectWebgl`）、`:81-111`（`tryLoad` 成功路径）；`src/panels/terminal/useXterm.ts` 经 `setupWebglWithRetry` 消费（webgl.ts:64-142）。

2. **现状**：
   - webgl.ts:34-44：`detectWebgl()` 单参 `canvas.getContext("webgl2")`，模块级 `webglCache`；:28-33 注释登记 FE-26 理由（blocklist 场景拒软件渲染 → DOM 掉帧；SwiftShader 远快于 DOM）。
   - 全文件无任何降级信号/通知；检测契约被 `src/__tests__/detect-webgl.test.ts:49`（「4. 检测不带 failIfMajorPerformanceCaveat」）与 `src/__tests__/webgl-setup.test.ts` 锁死。

3. **修复步骤**：
   1. `webgl.ts` 顶部补 import：
      ```ts
      import { toast } from "../../lib";
      ```
   2. `detectWebgl()` 保持原样不动（检测契约本身不改，FE-26 注释保留）。
   3. `webgl.ts` 在 `detectWebgl` 之后新增 SwiftShader 判定（独立缓存，一次性通知旗标）：
      ```ts
      /** SwiftShader（软件渲染）判定缓存——模块级，一次检测全生命周期复用 */
      let swiftShaderCache: boolean | null = null;
      /** 软件渲染 toast 是否已提示（CP-018：全生命周期一次性） */
      let swiftShaderNotified = false;

      /**
       * 判定当前 WebGL2 渲染器是否软件渲染（SwiftShader）。
       * 经 WEBGL_debug_renderer_info 扩展读 UNMASKED_RENDERER_WEBGL；扩展缺失或
       * 读取出错时保守返回 false（不提示——避免误报）。不改 detectWebgl 检测契约。
       */
      export function isSwiftShaderRenderer(): boolean {
        if (swiftShaderCache !== null) return swiftShaderCache;
        try {
          const canvas = document.createElement("canvas");
          const gl = canvas.getContext("webgl2");
          if (!gl) {
            swiftShaderCache = false;
          } else {
            const ext = gl.getExtension("WEBGL_debug_renderer_info");
            const renderer = ext
              ? String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL))
              : "";
            swiftShaderCache = /swiftshader/i.test(renderer);
          }
        } catch {
          swiftShaderCache = false;
        }
        return swiftShaderCache;
      }

      /** 重置 SwiftShader 判定缓存与通知旗标（仅测试使用） */
      export function resetSwiftShaderCache(): void {
        swiftShaderCache = null;
        swiftShaderNotified = false;
      }
      ```
   4. `setupWebglWithRetry` 的 `tryLoad` 成功路径（webgl.ts:88 `onSuccess(webglAddon);` 之后）追加：
      ```ts
        // CP-018：软件渲染一次性降级提示——GPU blocklist 机器落入 SwiftShader 时
        // 用户无任何感知（FE-26 接受软件渲染的前提是「远快于 DOM」，但仍慢于硬件 GPU）
        if (isSwiftShaderRenderer() && !swiftShaderNotified) {
          swiftShaderNotified = true;
          toast.show("info", "当前终端使用软件渲染（SwiftShader），滚动性能可能下降");
        }
      ```

4. **测试同步**：
   - 改 `src/__tests__/detect-webgl.test.ts`：保留现有 4 例（detectWebgl 契约不动）；新增 `isSwiftShaderRenderer` 用例组——① mock `getExtension` 返回 `{UNMASKED_RENDERER_WEBGL: 0x9246}` 且 `getParameter` 返回 `"ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device ...))"` → true；② 参数返回 `"ANGLE (Intel ...)"` → false；③ `getExtension` 返回 null → false；④ 每例前调 `resetSwiftShaderCache()`（用例隔离）。
   - 改 `src/__tests__/webgl-setup.test.ts`：新增「context loss 重试路径上 SwiftShader 仅提示一次」——连续两次触发成功回调，断言 `toast.show` 仅被调一次（mock `../../lib` 的 `toast`；每例前 `resetSwiftShaderCache()`）。

5. **文档同步**：
   - `src/panels/CLAUDE.md`「WebGL 优先 + DOM 兜底」节：FE-26 条目末尾追加——「CP-018：SwiftShader 场景首次加载成功后经 `isSwiftShaderRenderer()`（WEBGL_debug_renderer_info / UNMASKED_RENDERER_WEBGL）判定，一次性 toast 提示软件渲染降级；检测契约（不带 failIfMajorPerformanceCaveat）不变」。
   - `docs/compromises.md` 修复销项时登记本口径。

6. **验证**：
   - `grep -n "isSwiftShaderRenderer" src/panels/terminal/webgl.ts` ≥ 2（定义 + 调用）。
   - `grep -n "failIfMajorPerformanceCaveat" src/panels/terminal/webgl.ts` 仍仅注释命中（检测契约零变更）。
   - `npx vitest run detect-webgl webgl-setup` 全绿。

---

## CP-019 · PTY spawn 改事件驱动（ResizeObserver 首帧信号） [Stage 07]

1. **位置**：`src/panels/terminal/useXterm.ts:307-384`（PTY spawn 等待块）；清理点在 `:517-543`（`:521-523` 取消 rAF）。

2. **现状**：
   - :309-314：`const MAX_FRAMES = 30; const FIT_TIMEOUT = 500; const fitStartTime = performance.now();`
   - :355-384：`pollFitAndSpawn` rAF 自轮询——`container.offsetWidth > 0 && offsetHeight > 0` 才 fit+spawn；30 帧或 500ms 超时回退 `DEFAULT_COLS/DEFAULT_ROWS`（80×24）。
   - 测试锁死：`src/__tests__/use-xterm-lifecycle.test.ts` T1（:408 offsetWidth=0 不立即 spawn）、T2（:424 轮询后获得尺寸 spawn）、T3（:464 30 帧超时回退）、T4（:485 500ms 超时回退）。

3. **修复步骤**：
   1. `useXterm.ts` 删除 :310-314（`fitRafId/fitFrames/MAX_FRAMES/FIT_TIMEOUT/fitStartTime` 声明）与 :355-384（`pollFitAndSpawn` 定义 + 启动 rAF），`doSpawn`（:316-350）与 `doSpawnRef.current = doSpawn;`（:353）原样保留。
   2. 原 :384 `fitRafId = requestAnimationFrame(pollFitAndSpawn);` 处替换为事件驱动 spawn：
      ```ts
      // ── PTY spawn（CP-019：事件驱动）──
      // ResizeObserver 首帧回调确认容器尺寸就绪 → fit → proposeDimensions →
      // pty.spawn(真实尺寸)；500ms 超时仅作防御底线（回退 80×24，原 FIT_TIMEOUT 语义）
      let spawned = false;
      const spawnWithFit = () => {
        if (spawned) return;
        spawned = true;
        spawnObserver.disconnect();
        window.clearTimeout(spawnTimeoutId);
        if (canFit(term, fitAddon, container, isDisposedRef)) {
          try {
            fitAddon.fit();
            const dims = fitAddon.proposeDimensions();
            if (dims && Number.isFinite(dims.cols) && Number.isFinite(dims.rows)) {
              doSpawn(dims.cols, dims.rows);
              return;
            }
          } catch {
            // fit 失败 → 回退
          }
        }
        doSpawn(DEFAULT_COLS, DEFAULT_ROWS);
      };
      const spawnObserver = new ResizeObserver(() => {
        if (container.offsetWidth > 0 && container.offsetHeight > 0) spawnWithFit();
      });
      spawnObserver.observe(container);
      const spawnTimeoutId = window.setTimeout(spawnWithFit, 500);
      ```
   3. 清理段（:517-543）`:521-523` 的 rAF 取消替换为：
      ```ts
      spawnObserver.disconnect();
      window.clearTimeout(spawnTimeoutId);
      ```
      （`spawnObserver`/`spawnTimeoutId` 与清理闭包同处一个 effect 作用域，直接可达。）
   4. 头注释「PTY spawn 等待布局就绪」相关行（若文件头职责注释提及 30 帧/500ms 轮询）同步改为 ResizeObserver 事件驱动口径。

4. **测试同步**：
   - 改写 `src/__tests__/use-xterm-lifecycle.test.ts` T1-T4 四例为 ResizeObserver 驱动：
     - `setup.ts` 或该文件顶部补 `globalThis.ResizeObserver` mock（jsdom 无实现）——记录回调、`observe` 触发时手动调回调：
       ```ts
       class MockResizeObserver {
         static instances: MockResizeObserver[] = [];
         cb: ResizeObserverCallback;
         constructor(cb: ResizeObserverCallback) { this.cb = cb; MockResizeObserver.instances.push(this); }
         observe() { this.cb([], this as never); }
         disconnect() {}
       }
       ```
     - T1 改写：容器 `offsetWidth=0` → 手动触发 observer 回调 → `pty.spawn` 仍未调（事件驱动同样等尺寸）；T2 改写：置非零尺寸 → 触发回调 → 断言 `pty.spawn` 以 `proposeDimensions` 真实尺寸调用；T3/T4 合并改写为「500ms 超时兜底」——尺寸恒 0、不触发回调、推进 fake timers 500ms → 断言回退 `DEFAULT_COLS×DEFAULT_ROWS`。
   - 文件头注释（:3「rAF 轮询」）同步改为「ResizeObserver 事件驱动 + 超时防御」。

5. **文档同步**：
   - `src/panels/CLAUDE.md`「PTY spawn 等待布局就绪」节整节改写：
     ```
     `useXterm` 挂载后不立即 spawn PTY：ResizeObserver 首帧回调确认容器尺寸就绪（CP-019
     事件驱动）→ fit + proposeDimensions 取真实字符尺寸 → pty.spawn(真实 cols×rows)。
     500ms 超时仅作防御底线（回退 80×24）——正常路径不再有时序猜测轮询。
     ```

6. **验证**：
   - `grep -n "requestAnimationFrame" src/panels/terminal/useXterm.ts` = 0 命中。
   - `grep -n "ResizeObserver" src/panels/terminal/useXterm.ts` ≥ 2。
   - `npx vitest run use-xterm-lifecycle` 全绿（T1-T4 改写后）。

---

## CP-020 · Ctrl+C 本地中断事件源——working 显式置 attention [Stage 08]

> 起草裁定：**不新增 interrupted 态**——F3 四态（working 绿/attention 黄/done 灰/error 红）是 `StatusDot` 单点渲染契约（`src/lib/agentStatus.ts:17` + `src/lib/CLAUDE.md` IC-03），新增第五态会波及页签/导航树/历史行三处消费方；「attention」语义（需要关注/等待输入）恰好覆盖中断后等待用户输入的场景，且 60s `idle_prompt` 本就会转 attention——本地中断只是把该转换提前显式化。60s 兜底语义保留不变。

1. **位置**：
   - `src/features/shortcuts/commandCatalog.ts:26-99`（COMMAND_CATALOG，无 terminal.interrupt）
   - `src/panels/terminal/keyboard.ts:18-51`（`createTerminalShortcuts`，:7/:49 明示「Ctrl+C 不注册命令」）
   - `src/panels/terminal/activeTerminal.ts:10-14`（`TerminalActions` 三字段）
   - `src/panels/terminal/useXterm.ts:220-227`（`terminalActions` 构造）、`:161-172`（`useXterm` props 面）
   - `src/panels/terminal/TerminalPanel.tsx:84-102`（`handleTabStateChange`，tabStatus 写点先例 :89）
   - `src/features/shortcuts/reserved.ts:14-20`（TERMINAL_RESERVED 含 `Ctrl+KeyC`——只拦用户覆盖，不拦代码默认键，:8 注释）

2. **现状**：
   - keyboard.ts:49 注释：「Ctrl+C 不注册命令 → 自然透传，xterm.js 发送 \x03 到 PTY」。
   - TerminalPanel.tsx:89：`api.updateParameters({ ...latestParamsRef.current, tabStatus: state.status });`
   - `src/lib/agentStatus.ts:12-14` 登记已知行为：「Ctrl+C 用户主动中断不发射任何 hook 事件，working 无中断出边为预期行为，依赖下一事件覆盖或空闲提示(~60s) 衰减转 attention」。
   - command-catalog.test.ts:44-48 守卫：每条 defaultKey 对自身 context 非保留。

3. **修复步骤**：
   1. `commandCatalog.ts` 在 `terminal.newline` 条目后追加：
      ```ts
      {
        // CP-020：Ctrl+C 本地中断事件源——派发本地 interrupt 后置 attention，再透传 \x03。
        // 保留键语义不变：isReserved 仍拦用户覆盖（用户无法改绑/解绑此键）；
        // 代码默认键绑保留键为 CP-020 显式豁免（command-catalog.test 同步）。
        id: "terminal.interrupt",
        title: "中断（本地状态提示）",
        category: "terminal",
        context: "terminal",
        defaultKey: key("KeyC", { ctrl: true }),
        priority: 100,
      },
      ```
   2. `activeTerminal.ts`——`TerminalActions` 追加可选字段：
      ```ts
      /** 聚焦终端对外暴露的动作（供快捷键命令 handler 调用） */
      export interface TerminalActions {
        getSelection: () => string | undefined;
        paste: (text: string) => void;
        writeToPty: (data: Uint8Array) => void;
        /** CP-020：本地中断提示——Ctrl+C 时当前页签 working → attention；实现须幂等 */
        interrupt?: () => void;
      }
      ```
   3. `TerminalPanel.tsx`——新增 `handleInterrupt` 并传入 `useXterm`：
      ```ts
      // CP-020：本地中断事件源——claude 上游中断不发 hook 事件，状态机无中断出边，
      // 前端自建：仅当页签当前 working 时置 attention（幂等；window capture 与 xterm
      // attachCustomKeyEventHandler 委托双路径各调一次，第二次为 no-op）
      const handleInterrupt = useCallback(() => {
        if (latestParamsRef.current.tabStatus !== "working") return;
        api.updateParameters({ ...latestParamsRef.current, tabStatus: "attention" });
      }, [api]);
      ```
      `useXterm({...})` 调用（:161-172）追加一行：
      ```ts
        onInterrupt: handleInterrupt,
      ```
   4. `useXterm.ts`——props 接口追加 `onInterrupt?: () => void;`（带 JSDoc「CP-020」）；`terminalActions`（:220-227）改为：
      ```ts
      const onInterruptRef = useRef(onInterrupt);
      onInterruptRef.current = onInterrupt;
      const terminalActions = useMemo<TerminalActions>(
        () => ({
          getSelection: () => terminalRef.current?.getSelection(),
          paste: (text: string) => terminalRef.current?.paste(text),
          writeToPty,
          interrupt: () => onInterruptRef.current?.(),
        }),
        [writeToPty],
      );
      ```
   5. `keyboard.ts`——在 `terminal.newline` 命令后、`createTerminalShortcuts` 返回数组末尾追加：
      ```ts
      // CP-020：Ctrl+C 本地中断提示——派发 interrupt 后置 attention，再返回 false 透传，
      // xterm.js 仍发送 \x03 到 PTY（SIGINT 语义不变）
      commandFromMeta("terminal.interrupt", () => {
        const t = getActiveTerminal();
        if (!t?.interrupt) return false; // 无聚焦终端/旧实例 → 透传
        t.interrupt();
        return false; // 关键：透传——中断字节仍由 xterm 自然发送
      }),
      ```
      并改写文件头 :7 注释为：
      ```
      // Ctrl+C 注册为 terminal.interrupt 命令（CP-020）：handler 派发本地中断提示后
      // 返回 false 透传，xterm.js 仍自然发送 \x03 到 PTY（SIGINT 语义不变）。
      ```
   6. `src/lib/agentStatus.ts:12-14` 已知行为注释改为：
      ```
      // - Ctrl+C 用户主动中断不发 hook 事件——CP-020 起由前端本地中断命令
      //   （terminal.interrupt）显式将 working 置 attention；60s idle_prompt 兜底语义保留
      ```

4. **测试同步**：
   - 改 `src/__tests__/command-catalog.test.ts`：`EXPECTED_IDS` 追加 `"terminal.interrupt"`；:44-48 保留键守卫改为显式豁免形态——
     ```ts
     it("每条 defaultKey 对自身 context 非保留（terminal.interrupt 为 CP-020 显式豁免）", () => {
       for (const m of COMMAND_CATALOG) {
         if (m.id === "terminal.interrupt") {
           // 保留键语义不变：用户覆盖仍被 isReserved 拦截，仅代码默认键绑保留键
           expect(isReserved(m.defaultKey!, m.context)).toBe(true);
           continue;
         }
         expect(isReserved(m.defaultKey!, m.context)).toBe(false);
       }
     });
     ```
   - 改 `src/__tests__/terminal-shortcuts.test.ts`：新增用例组「CP-020 terminal.interrupt」——① 有 active 且 `interrupt` 存在 → handler 返回 false 且 `interrupt` 被调一次（防 window capture + xterm 委托双路径重复生效的幂等由 TerminalPanel 层保证，本层只断言派发与透传）；② 无 active → 返回 false；③ active 无 `interrupt` 字段（旧形态兼容）→ 返回 false 不抛。
   - 新增 `src/__tests__/terminal-interrupt-status.test.tsx`（TerminalPanel 层集成或 hook 级）：mock `api.updateParameters`，params.tabStatus="working" → 触发 `handleInterrupt` 等价路径 → 断言以 `{...params, tabStatus: "attention"}` 调用一次；params.tabStatus="done"/null → 不调用（幂等/条件断言，防复发用例——对照修复前「中断滞留 working」）。
   - `use-xterm-lifecycle.test.ts`：若断言 `terminalActions` 三字段形态，补 `interrupt` 存在性断言。

5. **文档同步**：
   - `src/features/shortcuts/CLAUDE.md`「外部坑/红线」——「Ctrl+C 保留为中断」整条改写：
     ```
     - **Ctrl+C 保留为中断（CP-020 修订）**：终端 Ctrl+C 注册为 `terminal.interrupt` 命令
       （defaultKey Ctrl+KeyC，保留键——用户覆盖仍被 isReserved 拦截，无法改绑/解绑）。
       handler 派发本地中断提示（working→attention）后**必须返回 false 透传**，xterm.js
       仍自然发送 \x03；任何新增 terminal context 命令不得拦截 Ctrl+C 的透传语义。
     ```
   - `src/panels/CLAUDE.md`「Ctrl+C 保留为中断」节与「中断场景已知行为（Ctrl+C）」节：前者同步上述口径；后者整段改写——「CP-020 起 Ctrl+C 经 terminal.interrupt 本地置 attention，滞留 working 消除；60s idle_prompt 兜底保留」。
   - `src/features/agentStatus/CLAUDE.md` 若登记「60s ticker」相关中断语义无需动；`src/panels/terminal/CLAUDE.md` 不存在则跳过。

6. **验证**：
   - `grep -n "terminal.interrupt" src/features/shortcuts/commandCatalog.ts src/panels/terminal/keyboard.ts` 各 ≥ 1 命中。
   - `grep -n "commandFromMeta(\"terminal.interrupt\"" src/panels/terminal/keyboard.ts` 命中且紧邻 `return false;`。
   - `npx vitest run command-catalog terminal-shortcuts terminal-interrupt` 全绿。
   - L4 可观测：真实 WebView2 中 claude 运行中按 Ctrl+C → 页签圆点绿转黄，且 PTY 确实收到 \x03（claude 取消行为不回归——手工验收，登记 test-exemptions 若需）。

---

## CP-021 · 历史区相对时间 60s ticker——占位对齐（实现归章六 CP-026） [Stage 01]

> 本条目为**契约占位**，不重复起草实现。已锁定决策：useAgentStatus 迁入 navTree 时把 60s ticker 显式移交 navTree 宿主，渲染层与数据层节奏解耦；实现与章六 CP-026 条目（S01，同 agent）合并落地。

1. **位置**：
   - `src/features/navTree/NavHistoryRow.tsx:38`（`const timeStr = formatRelativeTime(session.mtimeMs, Date.now());`——渲染时取 `Date.now()`，无 ticker）
   - `src/features/navTree/NavTree.tsx:510-516`（NavHistoryRow 唯一渲染点，宿主）
   - 先例：`src/features/agentStatus/useAgentStatus.ts:92-97`（60s `setInterval` ticker 驱动 `now`）
   - 登记点：`src/features/agentHistory/CLAUDE.md:89`（MC-318「视为可接受，不修」——修复后须改写）

2. **现状**：NavHistoryRow:38 相对时间每次渲染现算；重渲染由数据层快照广播（`backgroundTaskScheduler` sessionRefresh，默认 3s 可配 2-300s 可禁用）间接触发——禁用 sessionRefresh 或慢档时相对时间冻结（compromises.md CP-021 核查修正口径）。

3. **修复步骤（仅锁契约，实现归 CP-026 agent）**：
   1. **ticker 落点文件**：`src/features/navTree/NavTree.tsx`（navTree 宿主）——宿主级单一 60s ticker，替代各数据 hook 自建 ticker 的分散形态。
   2. **触发方式**：宿主 `useEffect(() => { const t = setInterval(() => setNow(Date.now()), 60_000); return () => clearInterval(t); }, [])`；`now: number` 经 React state 持有。
   3. **接口约定**：
      - `NavHistoryRowProps` 新增必填 `now: number`；`NavHistoryRow.tsx:38` 改为 `formatRelativeTime(session.mtimeMs, now)`——渲染层与数据层节奏解耦（sessionRefresh 禁用/慢档不影响相对时间刷新）。
      - NavTree 渲染 NavHistoryRow 处（:511-516）传 `now={now}`。
      - useAgentStatus 现有 60s ticker（useAgentStatus.ts:94-97）是否一并移交宿主（NavSessionRow 相对时间同源注入）由 CP-026 条目统一定稿；本条目不起草其代码。
   4. 本节仅作为章四与章六的对接锚点，执行 agent 以章六 CP-026 条目为准合并实现。

4. **测试同步**：归 CP-026 条目（预期：nav-tree 测试 fake timers 跨 60s 断言相对时间文本变化；NavHistoryRow 单测改传 `now` prop）。

5. **文档同步**：
   - `src/features/agentHistory/CLAUDE.md:89` MC-318 已知限制改写：「历史区相对时间由 navTree 宿主 60s ticker 驱动重算（CP-021，与 sessionRefresh 数据层节奏解耦）」。
   - `src/features/navTree/CLAUDE.md`「层级与数据源」节补一句宿主 ticker 契约。

6. **验证**：
   - `grep -n "Date.now()" src/features/navTree/NavHistoryRow.tsx` = 0 命中（实现落地后）。
   - `grep -n "60_000\|60000" src/features/navTree/NavTree.tsx` ≥ 1 命中。
   - 机械断言以 CP-026 条目的验证节为准。

---

## CP-022 · 大文件只读分片浏览路径（10MB 可编辑上限不变） [Stage 09]

1. **位置**：
   - `src/panels/editor/useCodeMirror.ts:46,48`（`MAX_FILE_SIZE_BYTES = 10_000_000` / `LARGE_FILE_WARN_BYTES = 1_000_000`）、`:305-309`（超限拒绝分支）
   - `src/panels/gitshow/GitShowPanel.tsx:177-185`（超限拒绝 + 大文件警告 header）
   - `src/panels/diff/DiffPanel.tsx:245-258`（head/workdir 双路超限检查）
   - 消费测试：`src/__tests__/use-code-mirror.test.ts:1029,1047`、`gitshow-panel.test.tsx:345,362`、`diff-panel.test.tsx:729,741`
   - 既有通道先例：后端 `fs_read_file` 256KB Channel 分块（BE-03，editor/CLAUDE.md:28-29）

2. **现状**：
   - useCodeMirror.ts:305-308：`sizeHint > MAX_FILE_SIZE_BYTES` → doc 替换为拒绝文案 + `filePathRef.current = undefined`。
   - GitShowPanel.tsx:178：拒绝文案「文件过大……已拒绝打开以保护内存」；diff 双栏直接拒绝。
   - CM6 不支持部分文档模型（FE-31 登记属实），但生态可只读分片——本条补只读路径，不改 10MB 可编辑上限语义。

3. **修复步骤（契约/模块级设计，代码给接口签名骨架）**：
   1. **后端 range 读块 IPC（新命令）**：
      - `src-tauri/src/fs/` 模块新增 `fs_read_file_range(path, offset_bytes, length_bytes) -> Result<String, AppError>`——经 `spawn_blocking` 读文件指定字节区间（UTF-8 边界安全：读取后裁到首个完整字符边界，返回实际字符串）；按硬约束 #3 注册进 `lib.rs` 的 `generate_handler!`、`capabilities/` 显式放行（不追加 `*`）。
      - DTO 双边对应（硬约束 #4）：`src/types/fs.ts` ↔ Rust DTO 同步定义，camelCase/snake_case 对应。
      - `src/ipc/fs.ts` 增 wrapper：
        ```ts
        /** 分块读文件指定字节区间（CP-022 大文件只读浏览）——返回区间内完整 UTF-8 文本 */
        export function readFileRange(filePath: string, offsetBytes: number, lengthBytes: number): Promise<string> {
          return invoke("fs_read_file_range", { filePath, offsetBytes, lengthBytes });
        }
        ```
      - L1 测试：临时文件全分支（空区间/越界 clamp/多字节字符边界不截断/不存在路径报 AppError）。
   2. **前端新模块 `src/panels/editor/largeFileViewer/`**（面板封闭 #5 下为 editor 面板子组件，不经 panelRegistry 新类型——同面板内形态切换）：
      ```
      largeFileViewer/
        LargeFileViewer.tsx      — 宿主组件（接管渲染替代 CM）
        useLineIndex.ts          — 行起始偏移索引 hook
        blockCache.ts            — LRU 读块缓存
      ```
      关键接口签名骨架：
      ```ts
      // blockCache.ts
      /** 读块大小（字节）——对齐后端 256KB 分块先例（BE-03） */
      export const READ_BLOCK_BYTES = 256 * 1024;
      /** 缓存块数上限（LRU） */
      export const BLOCK_CACHE_LIMIT = 32;
      /** 按需读块（命中 LRU 直接返回；未命中经 ipc/fs.readFileRange 拉取） */
      export async function readBlock(filePath: string, blockIndex: number): Promise<string>;

      // useLineIndex.ts
      /** 行起始偏移索引——首块扫 \n 建初始索引，滚动至未索引区时按需向后扩展 */
      export function useLineIndex(filePath: string, fileSizeBytes: number): {
        /** 总行数（索引未覆盖到 EOF 时为下界估计值） */
        lineCount: number;
        /** 取行文本（虚拟化窗口调用；行所在块未载入则同步触发读块后重渲染） */
        getLine(lineIndex: number): string | undefined;
        /** 索引是否已覆盖到 EOF（行数从估计转精确） */
        fullyIndexed: boolean;
      };

      // LargeFileViewer.tsx
      interface LargeFileViewerProps {
        filePath: string;
        fileSizeBytes: number;
        /** 来源面板展示用（editor/gitshow/diff） */
        sourceLabel: string;
      }
      /** 只读大文件浏览：固定行高虚拟化行窗口（窗口 = 可见行数 + 上下 overscan 各 20 行），
       *  行内经 useLineIndex 按需读块；顶部信息条提示「只读浏览（文件大小），可编辑上限 10MB」 */
      export const LargeFileViewer: React.FC<LargeFileViewerProps>;
      ```
      虚拟化窗口实现要点（写死）：行高取与编辑器一致的字号线高（复用 `createEditorFontExtension` 同款字号，默认 14px，行高 1.4 ≈ 20px 常量 `LARGE_FILE_LINE_HEIGHT = 20`）；容器 `overflow: auto` 自身为滚动容器（编辑器红线「外层不抢滚轮」不适用——本组件无 CM）；滚动事件换算 `startLine = floor(scrollTop / LINE_HEIGHT)`，仅渲染窗口内行（参照 `FileTree` 手实现虚拟化先例，explorer/CLAUDE.md「虚拟化行高 24px」）。
   3. **超限拒绝语义改引导**：
      - `useCodeMirror.ts:305-309` 拒绝分支不再置错误 doc，改为向上报告：hook 返回值增 `largeFile: { filePath: string; sizeBytes: number } | null`（超限分支填充；正常路径 null）。EditorPanel 检测 `largeFile` → 渲染 `<LargeFileViewer>` 替代 CM 编辑区（同面板形态切换，不经新面板类型）。`filePathRef.current = undefined` 保留（防误保存覆盖原文件——只读路径同样经此守卫）。
      - `GitShowPanel.tsx:177-185`：`sizeHint > MAX_FILE_SIZE_BYTES` 分支改渲染 LargeFileViewer（`sourceLabel="git show"`），1MB-10MB 警告 header 语义不变。
      - `DiffPanel.tsx:245-258`：任一侧超限 → 该侧以 LargeFileViewer 展示（对齐/滚动同步对只读浏览侧降级为单文档浏览，占位对齐装饰跳过——diff 分栏对齐仅保障 ≤10MB 可编辑域）；两侧均超限时分栏各自 LargeFileViewer。
   4. 10MB 可编辑上限与 1MB 警告阈值常量不动（`MAX_FILE_SIZE_BYTES`/`LARGE_FILE_WARN_BYTES` 仍单点导出）。

4. **测试同步**：
   - L1：新 `fs_read_file_range` 集成测试（`<cmd>_tests.rs` 命名，如 `fs_read_file_range_tests.rs`）。
   - L2 新增 `src/__tests__/large-file-viewer.test.tsx`：mock `ipc/fs.readFileRange`——行索引扩展、窗口渲染行数与滚动位置、LRU 驱逐；`useLineIndex` 单测：首块索引行数、跨块行拼接、多字节字符边界。
   - 改 `src/__tests__/use-code-mirror.test.ts:1029` 附近：「超过 MAX_FILE_SIZE_BYTES → 拒绝」改写为「→ 返回 largeFile 信号且 view 不创建」；`:1047` 警告用例不变。
   - 改 `src/__tests__/gitshow-panel.test.tsx:345`：拒绝文案断言改为 LargeFileViewer 渲染断言（`data-e2e` 锚）；`:362` 警告 header 用例不变。
   - 改 `src/__tests__/diff-panel.test.tsx:729,741`：超限用例改 LargeFileViewer 分栏断言。
   - 防复发用例：修复前「超限 = 静态拒绝文案」行为锁进上述改写用例的 before 形态断言（拒绝文案不再出现）。

5. **文档同步**：
   - `src/panels/editor/CLAUDE.md`「大文件不虚拟化（FE-31 登记，D3 关闭）」节改写：CM6 部分文档模型不支持仍属实——**可编辑域**维持 10MB 上限 + 1MB 警告 + BE-03 分块三层防线；新增第四层「>10MB 只读分片浏览（CP-022：LargeFileViewer 虚拟化行窗口 + fs_read_file_range 按需读块）」，editor/gitshow/diff 超限拒绝语义改引导。
   - `src/panels/CLAUDE.md` docViewer 家族/gitshow/diff 节补一句超限引导口径。
   - `src/ipc/CLAUDE.md`（若存在）登记新 `fs_read_file_range` wrapper。

6. **验证**：
   - `grep -n "fs_read_file_range" src-tauri/src/lib.rs src/ipc/fs.ts` 各 ≥ 1 命中。
   - `npx vitest run large-file-viewer use-code-mirror gitshow-panel diff-panel` 全绿。
   - `cargo test --test lib_tests fs_read_file_range -- --test-threads=1` 全绿（定向红线：走 lib_tests target）。
   - 手工/L4：>10MB 文本经 editor 打开 → 只读浏览可滚动至 EOF 且行内容正确（抽样断言）。

---

## CP-031 · escapeScriptClose 宿主 script 破坏——随 S10 预览根治消亡判定 [Stage 10]

> 本条**不起草独立修复代码**。已锁定决策：S10 预览迁独立 webview 后 srcdoc + injectScript 注入机制整体重写，本条随之消亡；起草内容 = 消亡判定条件 + skip 用例改写方向。

1. **位置**：
   - `src/lib/injectScript.ts:12-14`（`escapeScriptClose` 无差别转义）、`:40`（调用点）
   - `e2e-tests/html.e2e.ts:87`（`it.skip("内联 <script> 与内联事件属性在预览中执行")`）

2. **现状**：
   - injectScript.ts:12-14：`return html.replace(/<\/script>/gi, "<\\/script>");`——宿主 HTML 内所有 `</script>` 被转义 → 宿主 script 吞到 EOF → SyntaxError 永不执行。
   - **漂移留痕（实读核对）**：html.e2e.ts:87 的 skip 用例是**空壳**（函数体仅一行注释保留结构），其 skip 注释（:85-86）登记的原因是「依赖 CSP 'unsafe-inline' 放行内联脚本，修复需改动 src-tauri/tauri.conf.json，Stage 6 仅允许修改 e2e-tests/」——与 compromises.md 所述「escapeScriptClose 导致 skip」**不是同一通道**；escapeScriptClose 的 e2e 触发通道是 fixture 走 `<body onload>` 内联事件属性（html.e2e.ts:97-99 缩放用例注释实证）。两处登记在 compromises 里被合并叙述，实读证实为两个独立缺陷表现。

3. **修复步骤（判定条件 + 用例方向，无独立代码）**：
   1. **消亡判定条件（S10 执行收尾时逐项断言，全中才销项）**：
      - 判定一：`grep -rn "escapeScriptClose" src/` 零命中（注入机制重写后该函数不存在，或整文件 `src/lib/injectScript.ts` 已删除）；
      - 判定二：新预览注入机制（S10 产出）中宿主 HTML 的 `<script>` 段**不经任何字符串级转义**进入渲染文档——以新架构源码为准核对（如经独立 webview 直载或分段注入，转义点不存在）；
      - 判定三：预览 HTML 自带 `<script>`（非注入脚本）在新架构下真实可执行——由改写后的 e2e 用例（见下）在真实 WebView2 验证。
   2. **html.e2e.ts:87 skip 用例改写方向**：
      - 用例恢复为真实断言：fixture HTML 内嵌宿主 `<script>`（脚本内设置文档标记或 postMessage 上行）→ 新预览架构渲染后父侧/桩侧断言标记出现 → `it` 取消 skip；
      - 若 S10 新架构宿主 script 仍受 CSP 约束（`unsafe-inline` 决策未变），则该用例改写为**事件属性通道**（`<body onload>`，同缩放用例先例）并在用例注释登记原因，skip 解除与否以 S10 架构实际能力为准；
      - 原空壳注释「CSP 修复后取消 skip 即可恢复」删除——判定三（宿主 script 可执行）与 CSP 决策是 S10 的两项独立输入，用例形态按两者较严者定。
   3. 本条目不产出独立修复 PR；若 S10 架构评审结论为「仍同态」，则 CP-031 翻案回独立修复（转义收窄为「仅注入点之前宿主部分」——先定位插入点（`</head>`/`<body`/策略 3/4），前段转义后拼接未转义后段 + 注入脚本），届时须重新起草。

4. **测试同步**：归 S10 条目；本条仅要求 S10 收尾时执行判定一至三的机械断言。

5. **文档同步**：
   - `src/panels/CLAUDE.md`「宿主内联 `<script>` 不执行（escapeScriptClose 转义存量缺陷登记）」节——S10 销项时整段删除，替换为新架构注入机制说明；同态维持期间该节保留并追加「CP-031 已判消亡/翻案」状态行。
   - `src/panels/markdown/CLAUDE.md` 外部坑「host 内联 `<script>` 不执行」同步同上。

6. **验证**：
   - `grep -rn "escapeScriptClose" src/` 退出码 1（零命中）。
   - `grep -n "it.skip" e2e-tests/html.e2e.ts` 相对 S10 前基线 -1（该 skip 已解除或改写）。
   - 改写用例在 `npm run e2e`（html spec）真实通过。

---

## CP-036 · 批量关闭族（关闭其他/关闭全部）接入 dirty 守卫 [Stage 07]

> 与 CP-017 同 agent。依赖 CP-017 第 4 步（`closeTabGuarded` 确认分支补 `clearSettingsDirty`）先行落地，本条在其上扩展批量入口。

1. **位置**：
   - `src/workspace/PageDockviewHost.tsx:287-304`（「关闭其他」`:287-296`、`关闭全部」`:297-304` 直关）
   - `src/workspace/tabClose.ts:19-32`（`closeTabGuarded` 单面板入口——批量判定逻辑的事实真值源）
   - `src/features/settingsCenter/dirtyRegistry.ts:18`（`isSettingsDirty`）

2. **现状**：
   - PageDockviewHost.tsx:289-295：`group.panels.filter((p) => p !== panel).forEach((p) => p.api.close());`——批量直关，绕守卫。
   - :278-279 注释自认：「批量关闭族（关闭其他/关闭全部）维持直关（批量确认交互未定义，遗留见 workspace/CLAUDE.md）」。
   - workspace/CLAUDE.md「共享关闭守卫（FE-49）」节：「『关闭其他/关闭全部』批量路径仍直关（批量确认交互未定义，遗留）」。

3. **修复步骤**：
   1. `tabClose.ts`——`closeTabGuarded` 之后追加批量入口（一次性定义批量确认交互：dirty 面板列表 + 单次确认）：
      ```ts
      /**
       * 守卫批量关闭（CP-036——关闭其他/关闭全部接入 dirty 守卫）：
       * 收集待关列表中 dirty 的 settings 面板 → 单次 confirmDialog 列明确认 →
       * 确认才全部 close（并清除被丢弃面板的 dirtyRegistry 条目，CP-017 契约）；
       * 无 dirty 面板零交互直关（对非 settings 面板行为零回归）。
       * @param tabs 待关闭页签（close 原语 + params.panelId 判据 + 页签标题——确认列表展示用）
       */
      export async function closeTabsGuarded(
        tabs: Array<{
          api: { close(): void };
          panelId: string | undefined;
          title: string;
        }>,
      ): Promise<void> {
        const dirtyTabs = tabs.filter(
          (t) => t.panelId?.startsWith("settings-") && isSettingsDirty(t.panelId),
        );
        if (dirtyTabs.length > 0) {
          const ok = await confirmDialog({
            title: "未保存的修改",
            message:
              `以下设置面板有未保存的修改，关闭将丢弃这些修改：\n` +
              dirtyTabs.map((t) => `· ${t.title}`).join("\n"),
            kind: "warning",
          });
          if (!ok) return;
          for (const t of dirtyTabs) clearSettingsDirty(t.panelId!);
        }
        for (const t of tabs) t.api.close();
      }
      ```
      （import 面加 `clearSettingsDirty`——与 CP-017 第 4 步同一 import 行。）
   2. `PageDockviewHost.tsx`——import 面把 `closeTabGuarded` 导入处扩为 `{ closeTabGuarded, closeTabsGuarded }`；「关闭其他」action（:289-295）改：
      ```ts
      item("关闭其他", {
        danger: true,
        // CP-036：批量路径接入 closeTabsGuarded——dirty 面板列表 + 单次确认统一入口
        action: () => {
          const group = panel.api.group;
          if (!group) return;
          void closeTabsGuarded(
            group.panels
              .filter((p) => p !== panel)
              .map((p) => ({
                api: p.api,
                panelId: (p.params as TabParams | undefined)?.panelId,
                title: p.title,
              })),
          );
        },
      }),
      ```
      「关闭全部」action（:299-303）改：
      ```ts
      item("关闭全部", {
        danger: true,
        action: () => {
          const group = panel.api.group;
          if (!group) return;
          void closeTabsGuarded(
            [...group.panels].map((p) => ({
              api: p.api,
              panelId: (p.params as TabParams | undefined)?.panelId,
              title: p.title,
            })),
          );
        },
      }),
      ```
      并删除 :278-279「批量关闭族……维持直关（批量确认交互未定义，遗留见 workspace/CLAUDE.md）」遗留注释。
   3. 页删除路径（Workspace 删页 → 整页 Dockview 实例销毁，settings 面板随之卸载）**不在本条决策范围**——S07 执行收尾时按附注「漂移/遗留」节点名复核：若页删除不经任何守卫，残余破口须在产品决策（守卫 or 显式不守卫登记）后收口。

4. **测试同步**：
   - 改 `src/__tests__/tab-close.test.ts`：新增用例组「CP-036 closeTabsGuarded」——① 无 dirty 面板 → 零 confirmDialog 全部 close；② 含 1 个 dirty settings 面板 + 确认 → 全部 close 且 `isSettingsDirty` 清除；③ 含多个 dirty → confirmDialog 消息列全部标题；④ 取消 → 一个都不 close、条目保留；⑤ 非 settings 面板 dirty（registry 无条目语义外）直关。
   - 改 `src/__tests__/workspace-page-dockview.test.tsx`（`createTabMenuItems` 测试宿主）：「关闭其他/关闭全部」既有用例适配——mock `tabClose` 模块断言改调 `closeTabsGuarded`（参数形态：数组含 api/panelId/title）；新增「批量路径不再直关」防复发用例（对照修复前 `forEach close` 形态）。
   - `workspace-callback-cache.test.tsx` / `workspace-header-actions.test.tsx` 若触及关闭族 action，同名适配。

5. **文档同步**：
   - `src/workspace/CLAUDE.md`「共享关闭守卫（FE-49）」节：「『关闭其他/关闭全部』批量路径仍直关（批量确认交互未定义，遗留）」句删除，替换——「批量路径（关闭其他/关闭全部）经 `closeTabsGuarded` 统一入口（CP-036）：dirty 面板列表 + 单次确认；确认后清除 dirtyRegistry 条目再全部 close」。
   - `src/features/settingsCenter/CLAUDE.md` dirtyRegistry 节：批量清除点（CP-036）补入条目生命周期清单。

6. **验证**：
   - `grep -n "forEach((p) => p.api.close())" src/workspace/PageDockviewHost.tsx` = 0 命中。
   - `grep -n "closeTabsGuarded" src/workspace/PageDockviewHost.tsx src/workspace/tabClose.ts` 各 ≥ 1 命中。
   - `npx vitest run tab-close workspace-page-dockview` 全绿。

---

## CP-037 · markdown preview-only 改 CM 隐藏保活（display:none） [Stage 07]

1. **位置**：
   - `src/panels/markdown/MarkdownPanel.tsx:13-16`（头注释登记「preview-only 卸载」已知行为）
   - `src/panels/markdown/MarkdownPanel.tsx:275-287`（`useCodeMirror` 调用——`:276` container 三元）
   - `src/panels/markdown/MarkdownPanel.tsx:364-368`（`{mode !== "preview" && (<Allotment.Pane>…)}` 条件渲染）

2. **现状**：
   - :276：`container: mode !== "preview" ? cmContainerRef.current : null,`
   - :364-368：`{mode !== "preview" && (<Allotment.Pane minSize={160}><div ref={cmContainerRef} style={cmAreaStyle} /></Allotment.Pane>)}`
   - 对照先例：同文件 :15「edit↔split CM pane 不卸载（React 位置保活），undo/光标保留」；panels/CLAUDE.md「preview-only 卸载 CM（快照回填，光标/undo 重置登记已知行为）」。

3. **修复步骤**：
   1. `MarkdownPanel.tsx:364-368` 条件渲染改恒挂载 + `visible` 控制（照 edit↔split 先例，借 Allotment.Pane 自带 `visible` 实现 display:none 保活）：
      ```tsx
            {/* CM pane 恒挂载（CP-037：preview-only 改 display:none 保活——undo/光标跨形态保留，
                照 edit↔split 先例）；visible=false 时 allotment 收拢不占空间 */}
            <Allotment.Pane minSize={160} visible={mode !== "preview"}>
              <div ref={cmContainerRef} style={cmAreaStyle} />
            </Allotment.Pane>
      ```
      （删除外层 `{mode !== "preview" && (` 条件包裹。）
   2. `useCodeMirror` 调用（:275-287）container 表达式去 mode 三元：
      ```ts
        useCodeMirror({
          container: cmContainerRef.current,
          filePath: params.filePath,
          panelId: params.panelId,
          initialDoc: docRef.current,
          onDocContent: (text) => {
            setDoc(text);
            scheduleRender();
          },
          gitGutterEnabled: false,
          fontSize: editorFontSize,
          onFontSizeChange: setEditorFontSize,
        });
      ```
      效果：mode 切换不再使 container 在元素/null 间跳变 → `useCodeMirror` 的 container effect（useCodeMirror.ts:281-282 `if (!container) return;`）不触发重建 → EditorView 实例跨 edit/split/preview 全形态存活，光标/undo 栈保留。preview 态 `onDocContent` 仍实时写回 doc（外部修改 reload 仅 CM 挂载时监听——preview 不监听语义与 htmlviewer 一致，不变）。
   3. 头注释 :13-16 改为：
      ```
      //   - CM 恒挂载（CP-037：preview 态 allotment visible=false 隐藏保活——undo/光标跨形态
      //     保留，照 edit↔split 先例）；回 edit/split 免 initialDoc 回填重建；preview 态
      //     onDocContent 仍写回 doc（预览渲染源）；
      ```

4. **测试同步**：
   - 改 `src/__tests__/markdown-panel.test.tsx`：
     - 既有断言「preview 态 CM 卸载/container 传 null」的用例全部翻转为「container 恒传 `cmContainerRef.current`」；
     - 新增防复发用例组「CP-037 preview 隐藏保活」——① edit 态输入后切 preview 再切回 edit → mock CM 桥的创建函数（EditorView 构造 spy）全过程中仅调用一次（不重建）；② preview 态 CM pane 仍在 DOM（`visible=false` 属性断言或容器元素存在性断言）；③ 切形态后 `onDocContent` 驱动链不断（preview 态编辑 doc 后回 edit 内容一致）。
   - 防复发锚：修复前「preview 卸载 CM」行为由翻转用例锁死。

5. **文档同步**：
   - `src/panels/CLAUDE.md` docViewer 家族节「preview-only 卸载 CM（快照回填，光标/undo 重置登记已知行为）」句删除，替换「preview-only 改 CM 隐藏保活（CP-037，display:none 照 edit↔split 先例）——代价 preview 常驻一个 CM 实例内存，已接受」。
   - `src/panels/markdown/CLAUDE.md`「CM 仅 edit/split 挂载」条目同步改写为「CM 恒挂载，preview 态 visible=false 隐藏保活」；外部坑/红线中相关已知行为句删除。

6. **验证**：
   - `grep -n "mode !== \"preview\" ? cmContainerRef" src/panels/markdown/MarkdownPanel.tsx` = 0 命中。
   - `grep -n "visible={mode !== \"preview\"}" src/panels/markdown/MarkdownPanel.tsx` ≥ 1 命中。
   - `npx vitest run markdown-panel` 全绿。
   - L4（markdown.e2e.ts 关键路径，若现有用例覆盖 edit↔preview 往返）：往返后编辑器内容/光标不丢。

---

## CP-039 · editorTheme 订阅化——方案注册表响应式取色 + Compartment 热切换 [Stage 08]

> 与 CP-002 同 agent（共碰 `src/features/cliProfiles/profiles/claude/configEditor/JsonMode.tsx`）。

1. **位置**：
   - `src/theme/overrides.ts:40`（`export const editorTheme: Extension = schemeRegistry.getActive().editor.theme;`——模块级常量）
   - `src/theme/schemeRegistry.ts:44-52`（`setActive` 无变更通知机制）
   - 4 处消费：`src/panels/editor/useCodeMirror.ts:337-340`、`src/panels/diff/DiffPanel.tsx:547-550` 与 `:597-600`、`src/panels/gitshow/GitShowPanel.tsx:192-195`、`src/features/cliProfiles/profiles/claude/configEditor/JsonMode.tsx:164-167`
   - 登记点：`src/theme/CLAUDE.md:33`（「editorTheme 为常量，新窗口重载生效」）、`:65`（「editorTheme 常量：D2 切换不生效，需重载窗口」）；`src/theme/index.ts:61`（re-export）

2. **现状**：
   - overrides.ts:16-17 头注释：「函数形导出每次调用取当前 active 方案（支持 D2 热切换）；editorTheme 为模块级常量（求值时机由 main.tsx 启动序列保证）」。
   - useCodeMirror.ts:337-340 扩展数组序：`editorSyntaxHighlight(), editorTheme, editorColorOverrides(),`（ACC-05 顺序契约——syntax 必须先于 theme）。
   - theme-overrides.test.ts:97-101 锁「editorTheme === 注册表现值」常量契约；:219-231 锁消费点数组顺序（源文本正则断言）。
   - `main.tsx:67` 为唯一生产 setActive 调用点（grep 实证，运行期切换路径尚不存在——本条先消除 CM 掉队，为运行期切换铺路）。

3. **修复步骤**：
   1. `schemeRegistry.ts`——`SchemeRegistry` 类增订阅机制：
      ```ts
      /** active 方案变化监听器集合（CP-039：editorTheme 订阅化） */
      private listeners = new Set<() => void>();

      /** 订阅 active 方案变化——返回取消函数（卸载/测试清理时调用） */
      onDidChange(listener: () => void): () => void {
        this.listeners.add(listener);
        return () => this.listeners.delete(listener);
      }

      /** 通知全部监听器（setActive 两条路径——已知 id / 未知 id 回退 linear——均触发） */
      private notifyChange(): void {
        for (const listener of this.listeners) listener();
      }
      ```
      `setActive` 两个分支（:45-52）在 `this.activeId = …` 之后、`return` 之前各加一行 `this.notifyChange();`；`_reset()` 末尾追加 `this.notifyChange();`（测试隔离一致性——listeners 集合本身不被 `_reset` 清，监听方是自己的取消函数）。
   2. `overrides.ts:40` 常量改函数：
      ```ts
      /** CM 主题扩展透出（= 当前 active 方案 editor.theme）——CP-039：函数形导出，
       *  消费点经 theme Compartment 重配置实现热切换（编辑器不重建） */
      export function getEditorTheme(): Extension {
        return schemeRegistry.getActive().editor.theme;
      }
      ```
      头注释「五导出签名」段与 :16-17 说明同步修订（editorTheme 常量 → getEditorTheme 函数形）。
   3. `src/theme/index.ts:58-64` re-export 改 `{ dockviewVarStyle, allotmentVarStyle, getEditorTheme, editorColorOverrides, editorSyntaxHighlight }`（删除 `editorTheme` 导出——breaking，4 消费点同步改）。
   4. 新建 `src/theme/editorThemeSlot.ts`：
      ```ts
      // editorThemeSlot.ts — CM 编辑器主题热切换槽（CP-039）
      //
      // 一个 EditorView 一个槽（Compartment 不可跨 view 共享——editor/CLAUDE.md 红线）。
      // extension 入扩展数组，替换原 [editorSyntaxHighlight(), editorTheme, editorColorOverrides()]
      // 三项（槽内数组顺序固化不变：syntax 必须先于 theme——ACC-05 mountStyles reverse 层叠）。
      // view 创建后 bind(view)：订阅 schemeRegistry.onDidChange，方案一切换即 Compartment
      // 重配置——文档/光标/undo 全保留，编辑器不重建。

      import { Compartment } from "@codemirror/state";
      import type { Extension } from "@codemirror/state";
      import type { EditorView } from "@codemirror/view";
      import {
        getEditorTheme,
        editorColorOverrides,
        editorSyntaxHighlight,
      } from "./overrides";
      import { schemeRegistry } from "./schemeRegistry";

      /** 主题包：顺序固化 [syntax, theme, overrides]（ACC-05——syntax 只能靠数组顺序决胜） */
      export function editorThemeBundle(): Extension[] {
        return [editorSyntaxHighlight(), getEditorTheme(), editorColorOverrides()];
      }

      export interface EditorThemeSlot {
        /** 入扩展数组的单项（Compartment 包装） */
        extension: Extension;
        /** view 创建后调用：订阅方案变更，返回取消函数（组件卸载时调用） */
        bind(view: EditorView): () => void;
      }

      /** 每 EditorView 创建一个槽 */
      export function createEditorThemeSlot(): EditorThemeSlot {
        const compartment = new Compartment();
        return {
          extension: compartment.of(editorThemeBundle()),
          bind(view) {
            return schemeRegistry.onDidChange(() => {
              view.dispatch({
                effects: compartment.reconfigure(editorThemeBundle()),
              });
            });
          },
        };
      }
      ```
   5. 消费点改造（4 处，模式一致）：
      - `useCodeMirror.ts`：删 `editorTheme` import（改 `createEditorThemeSlot, type EditorThemeSlot` from `../../theme/editorThemeSlot`）；hook 内 fontCompartment 声明附近加：
        ```ts
        /** CP-039：主题热切换槽——view 存活期方案切换不重建编辑器 */
        const themeSlotRef = useRef<EditorThemeSlot | null>(null);
        if (themeSlotRef.current === null) themeSlotRef.current = createEditorThemeSlot();
        ```
        扩展数组 :337-340 三行替换为：
        ```ts
            themeSlotRef.current.extension,
        ```
        `viewRef.current = view;`（:370）之后加 `const unbindTheme = themeSlotRef.current.bind(view);`；cleanup（:397-406）`cleanup();` 之前加 `unbindTheme();`。
      - `JsonMode.tsx`（:150-182 挂载 effect）：`const themeSlotRef = useRef<EditorThemeSlot | null>(null); if (themeSlotRef.current === null) themeSlotRef.current = createEditorThemeSlot();`（组件顶层）；扩展数组 :164-167 三行替换为 `themeSlotRef.current.extension,`；`viewRef.current = view;`（:175）后加 `const unbindTheme = themeSlotRef.current.bind(view);`；cleanup（:176-179）`view.destroy();` 前加 `unbindTheme();`。
      - `GitShowPanel.tsx`（:187-217 创建 effect）：同 JsonMode 模式——扩展数组 :192-195 三行替换、创建后 bind、effect cleanup（view 销毁处）先 `unbindTheme();`。
      - `DiffPanel.tsx`（左右两栏各一个 view，:547-550 与 :597-600 两处）：`const leftThemeSlot = useRef<EditorThemeSlot | null>(null);` / `const rightThemeSlot = …`（各栏独立槽——Compartment 不可跨 view 共享红线）；两处扩展数组三行各替换为对应槽的 `.extension`；两 view 创建后各 `bind`、两栏 cleanup 各先 `unbindTheme();`。

4. **测试同步**：
   - 改 `src/__tests__/theme-overrides.test.ts`：「editorTheme」describe（:97-101）改写——断言 `getEditorTheme()` 返回注册表现值、`setActive` 切换后再次调用跟随新方案（响应式取色）；:239-266「切换后函数形导出跟随 active 方案（editorTheme 常量契约不重绑定）」用例改写为「`getEditorTheme` 切换后跟随 + 既有槽位 reconfigure 语义」；:219-231 数组顺序守卫的正则对象从 `editorTheme,` 改为 `themeSlotRef.current.extension,`（顺序守卫意图不变——syntax 先于 theme 的槽内顺序由 `editorThemeBundle` 单点锁死，新增断言：bundle 数组 `editorSyntaxHighlight` 实例索引 < `getEditorTheme()` 结果索引）。
   - 改 `src/__tests__/theme-scheme-registry.test.ts`：新增 `onDidChange` 用例组——① 注册监听 → `setActive("custom")`（测试内已注册方案）→ 回调触发；② 未知 id 回退分支同样触发；③ 取消函数后不再触发；④ `_reset` 触发（用于测试隔离）；⑤ 多监听全触发。
   - 改消费点测试：`use-code-mirror.test.ts` / `diff-panel.test.tsx` / `gitshow-panel.test.tsx` / JsonMode 相关测试——mock 形态下补「`schemeRegistry.setActive` 后 view.dispatch 以 Compartment 效果被调」用例（JS 侧 mock EditorView 断言 `dispatch` 收到 `effects`）；既有 `editorTheme` 常量引用的 import 全部改 `getEditorTheme`。
   - 防复发用例：「方案切换后 EditorView 不重建（构造 spy 调用次数不变）」锁进 useCodeMirror 用例组。

5. **文档同步**：
   - `src/theme/CLAUDE.md:33`——「函数形导出支持 D2 热切换；`editorTheme` 为常量，新窗口重载生效」改写：「`getEditorTheme()` 函数形导出随 active 方案响应式取色；消费点经 `editorThemeSlot.ts`（Compartment + `schemeRegistry.onDidChange`）热重配置——编辑器不重建，方案切换即时生效（CP-039）」。
   - `src/theme/CLAUDE.md:65` 外部坑「editorTheme 常量：D2 切换不生效，需重载窗口」整条删除，替换为 onDidChange 订阅契约（含「监听方必须在 view 销毁前调取消函数」红线）。
   - `src/theme/CLAUDE.md`「overrides.ts 五导出」节 :28 条目同步（editorTheme 常量 → getEditorTheme 函数形，导出数仍五个）。
   - `src/panels/editor/CLAUDE.md`「CM6 主题扩展与层叠（ACC-05）」节：消费点写法由三项数组改为 `themeSlotRef.current.extension` 单槽，ACC-05 顺序契约落点改述为「槽内 `editorThemeBundle()` 数组序」。
   - `.claude/adr.md` ADR-0002 追加后果条目：运行期即时切换整体仍否决，但 editorTheme 常量化这一系统性后果已消除（CM 主题即时跟随，CP-039）——为将来运行期切换移除最后一块编辑器侧障碍。

6. **验证**：
   - `grep -rn "import.*editorTheme[^S]" src/ --include=*.ts --include=*.tsx | grep -v getEditorTheme | grep -v editorThemeSlot` 零命中（旧常量零残留）。
   - `grep -n "onDidChange" src/theme/schemeRegistry.ts` ≥ 1。
   - `npx vitest run theme-overrides theme-scheme-registry use-code-mirror diff-panel gitshow-panel` 全绿；`npx eslint src/` 与 `npx tsc --noEmit` 退出码 0。

---

## CP-042 · openSettingsPanel 改事件驱动 + 超时 toast 可观测化 [Stage 07]

1. **位置**：
   - `src/workspace/pageApis.ts:135-164`（`openSettingsPanel`——:140-159 `for (let i = 0; i < 50; i++) { … await setTimeout 100 }` 轮询、:160-162 超时仅 `console.warn`）
   - `src/workspace/pageApis.ts:27-29`（`registerPageApi`——就绪信号唯一源头）
   - `src/workspace/Workspace.tsx:155-157`（`handlePageApiReady` → `registerPageApi(pageId, api)`）
   - 登记点：`src/workspace/CLAUDE.md:62`（openSettingsPanel 同页单例节）、`src/features/settingsCenter/CLAUDE.md`（openSettingsPanel 节）

2. **现状**：
   - pageApis.ts:131 注释自述「100ms×50 轮询 getPageApi 就绪……超时 console.warn 降级（不抛异常）」。
   - pageApis.ts:17 已 import `toast`（`switchToPageShared` 的 BE-23 警告用）——本修复复用，无新增依赖。

3. **修复步骤**：
   1. `pageApis.ts`——`registerPageApi` 派发就绪事件（契约常量大写导出供测试引用）：
      ```ts
      /** 页面 DockviewApi 就绪事件名（CP-042：openSettingsPanel 事件驱动等待；detail = pageId） */
      export const PAGE_API_READY_EVENT = "slterm:page-api-ready";

      /** 注册页面 DockviewApi（就绪时派发 window CustomEvent——事件驱动替代轮询） */
      export function registerPageApi(pageId: string, api: DockviewApi): void {
        pageApiMap.set(pageId, api);
        window.dispatchEvent(
          new CustomEvent(PAGE_API_READY_EVENT, { detail: pageId }),
        );
      }
      ```
   2. `pageApis.ts`——`openSettingsPanel` 整体重写（事件驱动等待 + 超时 toast；面板 addPanel 补 `renderer: "always"` 为 CP-017 接线点，同 agent 落地）：
      ```ts
      /**
       * 打开设置中心面板（同页单例）——调用方须先切到目标页
       * （本函数不切页，见 features/settingsCenter/openSettings.ts 编排）。
       *
       * 面板 id = `settings-{pageId}`；getPanel 命中 → focus 返回 true（同页单例），
       * 未命中 → addPanel（component "settings"，renderer "always"——CP-017；
       * settingsPageId 深链时注入 params.selectedPage）。
       * 页面 api 就绪改事件驱动等待（CP-042）：registerPageApi 派发
       * `slterm:page-api-ready`，5s 超时仅作防御底线——超时经 toast 可观测化
       * （原仅 console.warn 静默降级），返回 false 不抛异常。
       * @param settingsPageId 可选深链目标配置页 id（壳据此选中该配置页）
       * @returns 面板打开成功与否（超时返回 false）
       */
      export async function openSettingsPanel(
        pageId: string,
        settingsPageId?: string,
      ): Promise<boolean> {
        const panelId = `settings-${pageId}`;
        const api = await waitPageApi(pageId, 5000);
        if (!api) {
          console.warn(
            `[slTerminal] 页面 ${pageId} 的 DockviewApi 在 5s 内未就绪，无法打开设置中心`,
          );
          toast.show("warning", "设置中心打开失败：操作页面尚未就绪，请重试");
          return false;
        }
        const existing = api.getPanel(panelId);
        if (existing) {
          existing.focus?.();
          return true;
        }
        api.addPanel({
          id: panelId,
          component: "settings",
          title: "设置",
          renderer: "always",
          params: { panelId, ...(settingsPageId ? { selectedPage: settingsPageId } : {}) },
        });
        return true;
      }

      /** 等待页面 DockviewApi 注册——事件驱动（PAGE_API_READY_EVENT）+ 超时防御底线 */
      function waitPageApi(
        pageId: string,
        timeoutMs: number,
      ): Promise<DockviewApi | undefined> {
        const existing = getPageApi(pageId);
        if (existing) return Promise.resolve(existing);
        return new Promise((resolve) => {
          const onReady = (e: Event) => {
            if ((e as CustomEvent<string>).detail !== pageId) return;
            cleanup();
            resolve(getPageApi(pageId));
          };
          const timer = setTimeout(() => {
            cleanup();
            resolve(undefined);
          }, timeoutMs);
          const cleanup = () => {
            clearTimeout(timer);
            window.removeEventListener(PAGE_API_READY_EVENT, onReady);
          };
          window.addEventListener(PAGE_API_READY_EVENT, onReady);
        });
      }
      ```
      文件头职责注释（:5-6）「共享切换函数 switchToPageShared / switchToPageAndFocus / openSettingsPanel」不动；:123-134 原 doc 注释由上方新版替换。

4. **测试同步**：
   - 改写 `src/__tests__/open-settings-panel.test.ts`：
     - 「100ms×50 轮询」类用例全部改写——页面 api 未注册时 `openSettingsPanel` 挂起 → `registerPageApi(pageId, apiMock)` → 立即 resolve 且 addPanel 参数精确（含 `renderer: "always"`）；深链 selectedPage 用例不变。
     - 「5s 超时降级」用例（fake timers 推进 5s、不注册 api）——断言返回值 false、`console.warn` 被调、`toast.show` 以 `"warning"` 被调一次（原仅断言 console.warn——可观测化增量）。
     - 「单例 focus 不新建」用例不变；新增「事件 detail 非目标 pageId 不唤醒」用例（注册别页 api 后派发事件 → 仍挂起，随后目标页事件到达 → resolve）。
   - `workspace-page-apis.test.ts`：若 mock 了 `registerPageApi`，补事件派发断言（或不触及——按现有 mock 形态适配）。
   - `registerPageApi` 新增单测（可入 `workspace-page-apis.test.ts`）：注册后 `window` 收到 `slterm:page-api-ready` 且 `detail === pageId`。

5. **文档同步**：
   - `src/workspace/CLAUDE.md:62`「openSettingsPanel 同页单例（F11，SC-FE-02）」节：「100ms×50 轮询 getPageApi 就绪，超时 console.warn 降级返回 false」改写为「事件驱动等待 registerPageApi 派发的 `slterm:page-api-ready`（CP-042），5s 超时防御底线——超时 toast 可观测化后返回 false」。
   - `src/features/settingsCenter/CLAUDE.md`「openSettingsPanel 同页单例」节同步；「openSettings 编排」节末句「面板打开失败……由 openSettingsPanel 内部 console.warn 降级」改为「……超时经 toast 提示可观测化」。

6. **验证**：
   - `grep -n "for (let i = 0; i < 50; i++)" src/workspace/pageApis.ts` 命中数 = 1（仅剩 `switchToPageAndFocus` 一处；`openSettingsPanel` 轮询已删）。
   - `grep -n "PAGE_API_READY_EVENT" src/workspace/pageApis.ts` ≥ 3（常量、派发、监听）。
   - `grep -n "toast.show" src/workspace/pageApis.ts` ≥ 2（BE-23 原有 + 本修复新增）。
   - `npx vitest run open-settings-panel workspace-page-apis` 全绿。

---

## 起草附注

### 漂移点清单（实读核对后对 compromises.md 章四登记的修订）

1. **CP-031 与 html.e2e.ts:87**：登记叙述为「escapeScriptClose 破坏宿主 script → html.e2e.ts:87 skip」。实读：:87 skip 是**空壳用例**，skip 注释登记的根因是 **CSP 'unsafe-inline'**（改动需动 tauri.conf.json、Stage 6 只允许改 e2e-tests/），与 escapeScriptClose 是两个独立缺陷通道；escapeScriptClose 的 e2e 触发通道是其它用例的 `<body onload>` 事件属性 fixture（html.e2e.ts:97-99 注释实证）。本条已按「两个独立缺陷、S10 一并消亡」起草，判定条件含 CSP 决策输入。
2. **CP-017 破口面**：登记称「页删除等绕过 closeTabGuarded 的卸载路径」为残余破口。实读后明确：页删除路径（Workspace 删页 → Dockview 实例销毁）**不在已锁定决策范围内**（CP-036 只收批量两入口），S07 收尾需产品决策（守卫 or 显式不守卫登记），已在 CP-036 第 3 步留复核点。
3. **CP-016 决策措辞**：「SideBarArea 改受控组件」实读落地为「SideBarArea 向视图透传 `viewState/onViewStateChange`，状态本体存 sideViewRegistry 模块级 Map」——因隐藏视图不渲染、无需响应式订阅，plain Map 比 React state 更简；受控语义由 props 方向（外→内回填、内→外上呼）兑现。`zones/open/splitRatio` 状态不动（仍 sideBar store）。
4. **CP-020 状态机裁定**：决策留「或新增 interrupted 态」。实读状态机（agentStatus.ts 四态 + StatusDot IC-03 单点 + 三处消费方）后裁定**不新增态**，理由写入条目第 0 段。
5. **CP-039 运行期切换路径**：登记隐含「方案可运行期切换」。实读 grep：`setActive` 生产调用点仅 main.tsx:67（启动期）——运行期切换 UI 尚不存在；本条修复先消除 CM 掉队（决策原文「editorTheme 改订阅方案注册表响应式取色」的字面兑现），运行期切换本身不在本条。
6. **CP-042 行号**：compromises 引 workspace/CLAUDE.md:62；实读 pageApis.ts 轮询体在 :140-159（登记描述「100ms×50」与 :131 doc 注释一致），以代码原文为准。

### Stage 07 内 6 项文件重叠实读结论

S07 六项 = CP-016（sideViews/explorer）+ CP-017（settings/panelRegistry/pageApis/SettingsPanel/tabClose）+ CP-036（PageDockviewHost/tabClose/dirtyRegistry）+ CP-037（panels/markdown）+ CP-042（workspace/pageApis）+ CP-019（panels/terminal/useXterm）。实读结论：

- **真重叠只有两处**：
  1. `src/workspace/tabClose.ts`——CP-017 第 4 步（closeTabGuarded 补 clearSettingsDirty）与 CP-036 第 1 步（新增 closeTabsGuarded + 同文件 import 扩展）**同文件同 import 行**；已锁同 agent，条目内互相引用落地顺序（CP-017 先、CP-036 后）。
  2. `src/workspace/pageApis.ts`——CP-042 整体重写 `openSettingsPanel` 与 CP-017 第 2 步（addPanel 补 `renderer: "always"`）**同函数**；CP-042 条目代码块已内嵌 `renderer: "always"` 一行，CP-017 条目声明「合并落地、不重复改」，两条目代码块内容自洽（以 CP-042 版本为最终形态）。
- **无冲突邻近**：CP-016（features/sideViews + features/explorer）与 CP-037（panels/markdown）与 CP-019（panels/terminal）三者文件完全不相交；CP-017/036 的 settings/panelRegistry 面与 CP-042 的 pageApis 面相交但已由同 agent + 代码块内嵌解决。
- **编排建议**：S07 内顺序 CP-017 → CP-036 → CP-042 → CP-016 → CP-037 → CP-019（先收口守卫族，再独立项）。

### 跨 Stage 同 agent 线索

- **CP-017 + CP-036**：已锁定同 agent，落地顺序 CP-017 先行。
- **CP-039 + CP-002**（S08）：共碰 `JsonMode.tsx`——CP-039 条目第 5 步已给出 JsonMode 完整改造代码块，CP-002 执行时须以 CP-039 后形态为基线（先 CP-039 后 CP-002，或同 agent 一次合并）。
- **CP-021 + CP-026**（S01，章六）：CP-021 仅契约占位；接口约定（ticker 落点 NavTree.tsx、`now` prop 传入 NavHistoryRow、useAgentStatus ticker 移交细节归 CP-026）已在条目第 3 段锁定，章六条目须与此对齐。
- **CP-018（S08）与 CP-019（S07）**：文件不相交（webgl.ts vs useXterm.ts），但运行路径相邻（`setupWebglWithRetry` 由 useXterm 调用）——无代码冲突，各自独立可验。
- **CP-022（S09）独立**：新增后端命令 + 新前端模块，与 S07/S08 全部文件不相交；唯一邻近点是 `MAX_FILE_SIZE_BYTES` 阈值常量（不动）。
- **CP-031（S10）**：零代码起草；消亡判定三条机械断言在 S10 收尾执行。
