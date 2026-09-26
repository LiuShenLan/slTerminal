// features/agentFiles/filtering.ts — 「Agent 全局文件」视图过滤/名单校验纯函数
//
// 硬约束 #12：store 不存业务逻辑——本模块承载 agentGlobalFiles 段的全部校验/净化/
// 过滤判定，零 store/React/IPC 依赖（参数注入）。
// 匹配语义统一：大小写不敏感（NTFS 语义一致）、仅根层完全同名（名称非路径）。

import { relativePath } from "../../lib/path";
import type {
  AgentGlobalFilesConfig,
  AgentGlobalFilesMap,
} from "../../types/local";
import type { CodingCliProfile } from "../cliProfiles/types";

/** 单 agent 缺省配置：全部模式 + 空名单 + 不显示运行时文件 */
export function defaultConfig(): AgentGlobalFilesConfig {
  return { mode: "all", customNames: [], showRuntimeFiles: false };
}

/** 名称集合大小写不敏感包含判定（名单/运行时清单共用匹配语义） */
function nameMatches(name: string, list: string[]): boolean {
  const lower = name.toLowerCase();
  return list.some((n) => n.toLowerCase() === lower);
}

/** 自定义名单项校验：合法返回 null，非法返回错误消息（UI 行内红字）。
 *  拒空白 / 拒含路径分隔符（名称非路径）/ 大小写不敏感去重 */
export function validateCustomName(
  name: string,
  existing: string[],
): string | null {
  const trimmed = name.trim();
  if (trimmed === "") return "名称不能为空";
  if (trimmed.includes("\\") || trimmed.includes("/"))
    return "名称不能包含路径分隔符";
  if (nameMatches(trimmed, existing)) return "名称已存在";
  return null;
}

/** 单条配置净化：非法字段逐项回退缺省，customNames 内非法项逐项剔除 */
function sanitizeConfig(raw: unknown): AgentGlobalFilesConfig {
  const fallback = defaultConfig();
  if (typeof raw !== "object" || raw === null) return fallback;
  const obj = raw as Record<string, unknown>;
  const mode =
    obj.mode === "all" || obj.mode === "custom" ? obj.mode : fallback.mode;
  const customNames = Array.isArray(obj.customNames)
    ? obj.customNames.filter(
        (n): n is string => typeof n === "string" && n.trim() !== "" && !n.includes("\\") && !n.includes("/"),
      )
    : fallback.customNames;
  // 大小写不敏感去重（保留添加序的首个）
  const deduped: string[] = [];
  for (const n of customNames) {
    if (!nameMatches(n, deduped)) deduped.push(n);
  }
  const showRuntimeFiles =
    typeof obj.showRuntimeFiles === "boolean"
      ? obj.showRuntimeFiles
      : fallback.showRuntimeFiles;
  return { mode, customNames: deduped, showRuntimeFiles };
}

/** 加载净化（settings.json 可被手改/版本残留，后端纯透传不校验——ADR-0014 先例）：
 *  段非对象 → 全量空表；过滤未注册 cliId（孤儿键丢弃，与 R9 注册表对齐哲学一致）；
 *  单条非法字段回退缺省 */
export function sanitizeAgentGlobalFiles(
  raw: unknown,
  profiles: CodingCliProfile[],
): AgentGlobalFilesMap {
  if (typeof raw !== "object" || raw === null) return {};
  const registered = new Set(profiles.map((p) => p.id));
  const result: AgentGlobalFilesMap = {};
  for (const [cliId, value] of Object.entries(raw)) {
    if (!registered.has(cliId)) continue;
    result[cliId] = sanitizeConfig(value);
  }
  return result;
}

/** 根层条目是否展示（mode + 名单 + runtimePaths 三方判定）：
 *  - custom：仅名单完全同名（名单即完整真值，运行时开关不生效）
 *  - all：全部展示；showRuntimeFiles=false 时排除 runtimePaths 命中项 */
export function shouldShowAtRoot(
  name: string,
  config: AgentGlobalFilesConfig,
  runtimePaths: string[],
): boolean {
  if (config.mode === "custom") {
    return nameMatches(name, config.customNames);
  }
  if (!config.showRuntimeFiles && nameMatches(name, runtimePaths)) {
    return false;
  }
  return true;
}

/** fs-event 路径相关性判定（agent 视图 eventPathFilter 用）：
 *  取事件路径相对根的首段，按 shouldShowAtRoot 同语义判定——
 *  首段被过滤的条目，其内部变更不影响展示（根层即排除，目录本身不出现），事件丢弃。
 *  根外路径 / 相对路径不可算 → false（防御；上层 useFileTree 已有根前缀过滤） */
export function isRootEventRelevant(
  absPath: string,
  rootPath: string,
  config: AgentGlobalFilesConfig,
  runtimePaths: string[],
): boolean {
  const rel = relativePath(absPath, rootPath);
  if (rel === null) return false;
  const firstSegment = rel.split("/")[0];
  return shouldShowAtRoot(firstSegment, config, runtimePaths);
}
