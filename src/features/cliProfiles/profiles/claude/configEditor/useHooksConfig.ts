// useHooksConfig — hooks 配置面板数据 hook（P3-FE-15 + P3-FE-16/17）
//
// 职责：
// - cliId 参数 = hub 面板选中 CLI（Stage 03 临时代理常量已回收，MC-220）——
//   readHooksConfig/writeHooksConfig 的 cliId 实参唯一来源
// - 初始层 = initialLayer 参数（编辑器传 profile.configLayers[0].id，KZ-4）；
//   缺省回退 "user"（防御——configLayers 缺失不崩）
// - 从 useProjects + useLayout 推导当前活跃项目 rootPath（照 useCommitStatus 模式）
// - rootPath 为空时 project/local 层禁用（仅 user 层可用）
// - 加载：readHooksConfig(cliId, layer, rootPath?)，null 视为 {}；挂载时加载禁用状态 store
// - 双模式同步（P3-FE-16）：configJson（hooks 子树）与 guiModel 共享于此；
//   JsonMode.onChange 经 updateConfigJson（JSON 合法 → jsonToGui 重算 guiModel），
//   GuiMode.onChange 经 updateGui（guiToJson 更新 configJson）
// - 保存（P3-FE-17 + SEC-05/D9）：JSON 语法校验 → json-schema-library schema 校验
//   （validateHooksJson，Stage 04 已建）→ 任一失败弹窗提示、拒绝写盘 → user 层
//   confirmDialog 二次确认（hooks 可执行任意命令，取消不写盘；project/local 不确认）
//   → writeHooksConfig(cliId)；成功后置 saved（状态条显示重启提示）。不做 .bak，
//   其他字段保留由后端 merge 保证（P3-BE-03）
// - 轻量重读（外部修改检测）：切层 / 页面重新可见（document.visibilitychange 且
//   visibilityState === "visible"，面板可见时）重新 readHooksConfig；dirty 时用
//   confirmDialog 提示（照编辑器外部修改先例，不用 window.confirm），用户确认丢弃才覆盖；
//   弹窗打开/关闭的回归触发由 askGuard 抑制（防循环）；保存校验失败为纯告警（无确认
//   语义）→ toast.show("error")（OV-02 执行期决策）

import { useState, useEffect, useRef, useCallback } from "react";
import { readHooksConfig, writeHooksConfig } from "../../../../../ipc/hooksConfig";
// FE-25: 错误消息统一经 getErrorMessage（契约：src/ipc/appError.ts，src/lib re-export）
import { confirmDialog, toast, getErrorMessage } from "../../../../../lib";
import { useProjects } from "../../../../../stores/projects";
import { useLayout } from "../../../../../stores/layout";
import type { HooksLayer, HooksConfigJson, HooksConfigGui } from "../../../../../types/hooksConfig";
import { validateHooksJson } from "./schema";
import {
  jsonToGui,
  guiToJson,
  type HooksConfigJson as ConfigJson,
  type HooksConfigGui as ConfigGui,
} from "./configModel";

/** 配置损坏错误文案——read 返回 Err（与无配置返回 null 区分开） */
export const CONFIG_CORRUPTED_TEXT = "配置文件损坏，请先修复";

/** confirmDialog 弹窗关闭后守卫窗口（ms）——期间内的回归触发的重读被抑制（防循环） */
const ASK_GUARD_MS = 500;

export interface UseHooksConfigResult {
  /** 当前编辑层级 */
  layer: HooksLayer;
  /** 切换层级：dirty 时 confirmDialog 确认丢弃后重读目标层 */
  setLayer: (l: HooksLayer) => void;
  /** 当前活跃项目 rootPath（null = 无项目，project/local 层禁用） */
  rootPath: string | null;
  /** hooks 子树原始 JSON（null 视为 {}） */
  configJson: HooksConfigJson;
  /** GUI 模型（configModel 转换，Stage 05 表单模式数据源） */
  guiModel: HooksConfigGui;
  /** 是否有未保存修改 */
  dirty: boolean;
  /** 配置损坏错误态（read 返回 Err） */
  error: boolean;
  /** 加载中 */
  loading: boolean;
  /** 保存成功标志（状态条显示「hooks 改动需重启 claude 会话生效」）；编辑/重载后清除 */
  saved: boolean;
  /** 更新 hooks 子树（JsonMode 编辑回调，JSON.parse 已通过）：置 dirty + guiModel 同步重算 */
  updateConfigJson: (json: HooksConfigJson) => void;
  /** 更新 GUI 模型（GuiMode 编辑回调）：guiToJson 更新 configJson + guiModel 同步重算 */
  updateGui: (gui: ConfigGui) => void;
  /** 保存：语法 + schema 双校验（失败 toast 提示拒绝写盘）→ user 层 confirmDialog 二次确认（SEC-05/D9）→ writeHooksConfig，成功清除 dirty + 置 saved */
  save: () => Promise<void>;
  /** 轻量重读（外部修改检测）：dirty 时 confirmDialog 确认才覆盖 */
  reload: () => Promise<void>;
}

export function useHooksConfig(
  cliId: string,
  initialLayer?: HooksLayer,
): UseHooksConfigResult {
  const projects = useProjects((s) => s.projects);
  const activePageId = useLayout((s) => s.activePageId);

  // 推导 rootPath（照 useCommitStatus：页面 cwd 优先，回退项目根）
  let rootPath: string | null = null;
  if (activePageId) {
    for (const [, proj] of Object.entries(projects)) {
      const activePage = proj.pages.find((p) => p.pageId === activePageId);
      if (activePage) {
        rootPath = activePage.cwd || proj.rootPath;
        break;
      }
    }
  }

  // 初始层 = 调用方传入（编辑器传 profile.configLayers[0].id——KZ-4）；缺省回退 "user"（防御）
  const [layer, setLayerState] = useState<HooksLayer>(initialLayer ?? "user");
  const [configJson, setConfigJson] = useState<HooksConfigJson>({});
  const [guiModel, setGuiModel] = useState<HooksConfigGui>({ events: [] });
  const [dirty, setDirty] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState(false);
  const [loading, setLoading] = useState(true);

  // ref 镜像：回调/effect 闭包内读取最新值（照 useCommitStatus rootPathRef 模式）。
  // cliId 由 hub 选中态传入——hub 切换 = 卸载重挂载（ADR-0001），组件生命周期内恒定；
  // ref 形态保持 load/save 的 useCallback deps 稳定（与 layerRef/rootPathRef 一致）
  const cliIdRef = useRef(cliId);
  cliIdRef.current = cliId;
  const layerRef = useRef(layer);
  layerRef.current = layer;
  const rootPathRef = useRef(rootPath);
  rootPathRef.current = rootPath;
  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;
  const configJsonRef = useRef(configJson);
  configJsonRef.current = configJson;
  // generation 取消：切层/切项目/重读竞态下丢弃过期回调结果
  const genRef = useRef(0);
  // confirmDialog 弹窗守卫：confirmDiscard 弹窗打开期间 + 关闭后短暂窗口内抑制回归触发的
  // 重读——弹窗打开/关闭伴随的回归触发若无守卫将再次弹窗（验收 2.1「点否无限
  // 循环 / 点是重弹」根因）
  const askGuardRef = useRef(false);
  // FE-25: askGuard 复位定时器 id——存 ref 供卸载 cleanup clearTimeout（防卸载后定时器
  // 回调仍执行改 ref；异步竞态窗口内重挂载也不被旧定时器误关守卫）
  const askGuardTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // FE-25: 卸载时清理 askGuard 复位定时器（泄漏防护——cleanup 中 clearTimeout）
  useEffect(() => {
    return () => {
      if (askGuardTimerRef.current !== null) {
        clearTimeout(askGuardTimerRef.current);
        askGuardTimerRef.current = null;
      }
    };
  }, []);

  /** 加载指定层配置（generation 取消竞态；null 视为 {}，Err 置损坏错误态）
      showLoading=true 时显示 loading 遮罩（首次加载/切层/切项目）；
      false（默认，reload 路径）保留旧内容渲染，数据到达后替换——避免重读 blank 面板吞点击（验收 #1） */
  const load = useCallback(async (target: HooksLayer, gen: number, showLoading = false) => {
    if (showLoading) {
      setLoading(true);
      setError(false);
    }
    try {
      const raw = await readHooksConfig(cliIdRef.current, target, rootPathRef.current ?? undefined);
      if (gen !== genRef.current) return; // 过期结果丢弃
      const json = (raw === null || raw === undefined ? {} : raw) as HooksConfigJson;
      setConfigJson(json);
      setGuiModel(jsonToGui(json));
      setDirty(false);
      setSaved(false);
      setLoading(false);
    } catch {
      if (gen !== genRef.current) return;
      setError(true);
      setLoading(false);
    }
  }, []);

  /** dirty 守卫：有未保存修改时 confirmDialog 确认（用户确认丢弃才放行），无 dirty 直接放行。
      confirmDialog 打开前置 askGuardRef（弹窗关闭后 500ms 内回归触发不重读——防循环） */
  const confirmDiscard = useCallback(async (message: string): Promise<boolean> => {
    if (!dirtyRef.current) return true;
    askGuardRef.current = true;
    try {
      return await confirmDialog({
        title: "未保存的修改",
        message,
        kind: "warning",
      });
    } finally {
      // FE-25: 复位定时器 id 存 ref——卸载 cleanup clearTimeout；重入时先清旧定时器
      if (askGuardTimerRef.current !== null) {
        clearTimeout(askGuardTimerRef.current);
      }
      askGuardTimerRef.current = setTimeout(() => {
        askGuardRef.current = false;
        askGuardTimerRef.current = null;
      }, ASK_GUARD_MS);
    }
  }, []);

  /** 切层：dirty 时 confirmDialog 确认丢弃，确认后重读目标层 */
  const setLayer = useCallback(
    (l: HooksLayer) => {
      if (l === layerRef.current) return;
      void (async () => {
        try {
          const ok = await confirmDiscard("当前层有未保存的修改，切换层级将丢弃这些修改。");
          if (!ok) return;
          const gen = ++genRef.current;
          setLayerState(l);
          await load(l, gen, true); // 切层显示 loading 遮罩（内容替换有清晰反馈）
        } catch (err) {
          // FE-25: 切层异步链异常捕获——不静默吞错（confirmDialog 异常/未来 load 抛错），
          // toast 提醒 + 日志（load 内部既有 generation 过期检查与 try/catch，此处兜外层）
          toast.show("error", `切换配置层失败: ${getErrorMessage(err)}`);
          console.error("[slTerminal] 切换 hooks 配置层失败:", err);
        }
      })();
    },
    [confirmDiscard, load],
  );

  /** 更新 hooks 子树（JsonMode 编辑回调，JSON.parse 已通过）：置 dirty + guiModel 同步重算 */
  const updateConfigJson = useCallback((json: HooksConfigJson) => {
    setConfigJson(json);
    setGuiModel(jsonToGui(json));
    setDirty(true);
    setSaved(false);
  }, []);

  /** 更新 GUI 模型（GuiMode 编辑回调）：guiToJson 更新 configJson + guiModel 同步重算（双模式同步 P3-FE-16） */
  const updateGui = useCallback((gui: ConfigGui) => {
    const json = guiToJson(gui) as unknown as HooksConfigJson;
    setConfigJson(json);
    setGuiModel(jsonToGui(json));
    setDirty(true);
    setSaved(false);
  }, []);

  /** 保存：语法 + schema 双校验（失败 toast 提示、拒绝写盘）→ user 层 confirmDialog
      二次确认（SEC-05/D9）→ writeHooksConfig。
      校验失败为纯告警（无确认/取消语义）→ toast.show("error")（OV-02 执行期决策）；
      user 层确认语义：hooks 可执行任意命令，取消则不写盘（dirty 保留），
      project/local 层不确认直接写 */
  const save = useCallback(async () => {
    const json = configJsonRef.current;
    // ① JSON.parse 语法校验：configJson 只容纳 parse 合法快照（编辑时已门控），此处防御性确认对象形态
    if (json === null || typeof json !== "object" || Array.isArray(json)) {
      toast.show("error", "hooks 配置必须是 JSON 对象");
      return;
    }
    // ② json-schema-library schema 校验（validateHooksJson = JSON.parse + Draft07(hooksSubSchema).validate，
    //    Stage 04 已建，禁止 ajv）；任一失败 toast 提示、拒绝调用 writeHooksConfig
    const result = validateHooksJson(JSON.stringify(json));
    if (!result.isValid) {
      toast.show("error", result.diagnostics[0]?.message ?? "hooks 配置不符合 schema");
      return;
    }
    // ③ user 层二次确认（SEC-05/D9）：用户级配置影响所有项目会话且 hooks 可执行
    //    任意命令——确认才写盘；取消保持 dirty（不丢用户修改）
    const layer = layerRef.current;
    if (layer === "user") {
      const ok = await confirmDialog({
        title: "确认写入用户级 hooks 配置",
        message: "hooks 可执行任意命令",
        kind: "warning",
      });
      if (!ok) return;
    }
    // ④ 写盘（后端 read-modify-write merge 保留其他字段，P3-BE-03）
    await writeHooksConfig(cliIdRef.current, layer, json as unknown as ConfigJson, rootPathRef.current ?? undefined);
    setDirty(false);
    setSaved(true);
  }, []);

  /** 轻量重读（外部修改检测）：dirty 时 confirmDialog 确认丢弃才覆盖。
      开头检查 askGuard——自身弹窗关闭后的回归触发在此拦截（防无限循环） */
  const reload = useCallback(async () => {
    if (askGuardRef.current) return;
    const ok = await confirmDiscard("配置文件可能已被外部修改。重载将丢弃当前未保存的修改。");
    if (!ok) return;
    const gen = ++genRef.current;
    await load(layerRef.current, gen);
  }, [confirmDiscard, load]);

  // 加载/重载：挂载首次加载 + rootPath 变化时重读当前层
  // rootPath 为空时 project/local 层禁用：当前层非 user 则回退 user 层
  useEffect(() => {
    let target: HooksLayer = layerRef.current;
    if (!rootPath && target !== "user") {
      target = "user";
      setLayerState("user");
    }
    const gen = ++genRef.current;
    void load(target, gen, true); // 首次挂载/切项目显示 loading 遮罩
  }, [rootPath, load]);

  return {
    layer,
    setLayer,
    rootPath,
    configJson,
    guiModel,
    dirty,
    saved,
    error,
    loading,
    updateConfigJson,
    updateGui,
    save,
    reload,
  };
}
