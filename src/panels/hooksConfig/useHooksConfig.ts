// useHooksConfig — hooks 配置面板数据 hook（P3-FE-15 + P3-FE-16/17）
//
// 职责：
// - 从 useProjects + useLayout 推导当前活跃项目 rootPath（照 useCommitStatus 模式）
// - rootPath 为空时 project/local 层禁用（仅 user 层可用）
// - 加载：readHooksConfig(layer, rootPath?)，null 视为 {}；挂载时加载禁用状态 store
// - 双模式同步（P3-FE-16）：configJson（hooks 子树）与 guiModel 共享于此；
//   JsonMode.onChange 经 updateConfigJson（JSON 合法 → jsonToGui 重算 guiModel），
//   GuiMode.onChange 经 updateGui（guiToJson 更新 configJson）
// - 保存（P3-FE-17）：JSON 语法校验 → json-schema-library schema 校验（validateHooksJson，
//   Stage 04 已建）→ 任一失败弹窗提示、拒绝写盘 → filterDisabled 剔除当前层禁用条目
//   → writeHooksConfig；成功后置 saved（状态条显示重启提示）。不做 .bak，其他字段
//   保留由后端 merge 保证（P3-BE-03）
// - 轻量重读（外部修改检测）：切层 / 面板聚焦（focusin）时重新 readHooksConfig；
//   dirty 时用 dialog.ask 提示（照编辑器外部修改先例，不用 window.confirm），用户确认丢弃才覆盖

import { useState, useEffect, useRef, useCallback } from "react";
import { readHooksConfig, writeHooksConfig } from "../../ipc/hooksConfig";
import { ask } from "../../ipc/dialog";
import { useProjects } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import { useHooksConfig as useHooksConfigStore } from "../../stores/hooksConfig";
import type { HooksLayer, HooksConfigJson, HooksConfigGui } from "../../types/hooksConfig";
import { validateHooksJson } from "../../features/hooksConfig/schema";
import {
  jsonToGui,
  guiToJson,
  filterDisabled,
  type HooksConfigJson as ConfigJson,
  type HooksConfigGui as ConfigGui,
} from "./configModel";

/** 配置损坏错误文案——read 返回 Err（与无配置返回 null 区分开） */
export const CONFIG_CORRUPTED_TEXT = "配置文件损坏，请先修复";

export interface UseHooksConfigResult {
  /** 当前编辑层级 */
  layer: HooksLayer;
  /** 切换层级：dirty 时 ask 确认丢弃后重读目标层 */
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
  /** 保存：语法 + schema 双校验（失败弹窗拒绝写盘）→ filterDisabled → writeHooksConfig，成功清除 dirty + 置 saved */
  save: () => Promise<void>;
  /** 轻量重读（面板 focusin 外部修改检测）：dirty 时 ask 确认才覆盖 */
  reload: () => Promise<void>;
}

export function useHooksConfig(): UseHooksConfigResult {
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

  const [layer, setLayerState] = useState<HooksLayer>("user");
  const [configJson, setConfigJson] = useState<HooksConfigJson>({});
  const [guiModel, setGuiModel] = useState<HooksConfigGui>({ events: [] });
  const [dirty, setDirty] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState(false);
  const [loading, setLoading] = useState(true);

  // ref 镜像：回调/effect 闭包内读取最新值（照 useCommitStatus rootPathRef 模式）
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

  /** 加载指定层配置（generation 取消竞态；null 视为 {}，Err 置损坏错误态） */
  const load = useCallback(async (target: HooksLayer, gen: number) => {
    setLoading(true);
    setError(false);
    try {
      const raw = await readHooksConfig(target, rootPathRef.current ?? undefined);
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

  /** dirty 守卫：有未保存修改时 ask 确认（用户确认丢弃才放行），无 dirty 直接放行 */
  const confirmDiscard = useCallback(async (message: string): Promise<boolean> => {
    if (!dirtyRef.current) return true;
    return ask(message, { title: "未保存的修改", kind: "warning" });
  }, []);

  /** 切层：dirty 时 ask 确认丢弃，确认后重读目标层 */
  const setLayer = useCallback(
    (l: HooksLayer) => {
      if (l === layerRef.current) return;
      void (async () => {
        const ok = await confirmDiscard("当前层有未保存的修改，切换层级将丢弃这些修改。");
        if (!ok) return;
        const gen = ++genRef.current;
        setLayerState(l);
        await load(l, gen);
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

  /** 保存：语法 + schema 双校验（失败弹窗提示、拒绝写盘）→ filterDisabled 剔除禁用条目 → writeHooksConfig */
  const save = useCallback(async () => {
    const json = configJsonRef.current;
    // ① JSON.parse 语法校验：configJson 只容纳 parse 合法快照（编辑时已门控），此处防御性确认对象形态
    if (json === null || typeof json !== "object" || Array.isArray(json)) {
      await ask("hooks 配置必须是 JSON 对象", { title: "保存失败", kind: "error" });
      return;
    }
    // ② json-schema-library schema 校验（validateHooksJson = JSON.parse + Draft07(hooksSubSchema).validate，
    //    Stage 04 已建，禁止 ajv）；任一失败弹窗提示、拒绝调用 writeHooksConfig
    const result = validateHooksJson(JSON.stringify(json));
    if (!result.isValid) {
      await ask(result.diagnostics[0]?.message ?? "hooks 配置不符合 schema", {
        title: "保存失败",
        kind: "error",
      });
      return;
    }
    // ③ filterDisabled 剔除当前层禁用条目（四元组 layer 过滤，C13-8）→ 写盘（后端 merge 保留其他字段，P3-BE-03）
    // json 经强转对齐 configModel 契约（types/hooksConfig 无索引签名，结构等价）
    const layer = layerRef.current;
    const disabled = useHooksConfigStore.getState().disabledHooks.filter((k) => k.layer === layer);
    const filtered = filterDisabled(json as unknown as ConfigJson, disabled);
    await writeHooksConfig(layer, filtered, rootPathRef.current ?? undefined);
    setDirty(false);
    setSaved(true);
  }, []);

  /** 轻量重读（外部修改检测）：dirty 时 ask 确认丢弃才覆盖 */
  const reload = useCallback(async () => {
    const ok = await confirmDiscard("配置文件可能已被外部修改。重载将丢弃当前未保存的修改。");
    if (!ok) return;
    const gen = ++genRef.current;
    await load(layerRef.current, gen);
  }, [confirmDiscard, load]);

  // 挂载：加载禁用状态 store（store 不在 App init 中加载，见 store 头注释）
  useEffect(() => {
    void useHooksConfigStore.getState().loadFromDisk();
  }, []);

  // 加载/重载：挂载首次加载 + rootPath 变化时重读当前层
  // rootPath 为空时 project/local 层禁用：当前层非 user 则回退 user 层
  useEffect(() => {
    let target: HooksLayer = layerRef.current;
    if (!rootPath && target !== "user") {
      target = "user";
      setLayerState("user");
    }
    const gen = ++genRef.current;
    void load(target, gen);
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
