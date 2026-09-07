// jsonSchemaCm.ts — 自绘 hooks JSON schema lint/hover 层（CP-002，替代第三方
// json-schema 编辑器扩展）
//
// 直接消费 json-schema-library 11.x（compileSchema 单例见 schema/index.ts），消除
// 原扩展锁 9.x 造成的双 major 并存（TE-15 收编）。
// 语义对齐原扩展用法：lint = schema 违规波浪线（delay 300ms，与语法 linter 对齐）；
// hover = 键位置悬停显示子 schema description。
// pointer 定位策略：JSON Pointer 逐段顺序文本搜索（数组索引段按第 n 个 '[' 近似），
// 定位失败退回整文档下划——hooks 子树为小文档，近似定位足够收敛下划线。
//
// 导出分层（knip 销项成因，2026-09-08）：生产接线仅
// hooksSchemaLinter/hooksSchemaHover（JsonMode.tsx import）；
// 其余五导出（pointerToRange/lintHooksSchemaDoc/resolveSchemaPath/pathAt/
// resolveHoverDescription）供模块内互调 + hooks-json-schema-cm.test.ts 直测——
// knip 忽略 __tests__ 且互调不计消费，按合法导出登记 knip.json ignoreIssues。

import { linter, type Diagnostic } from "@codemirror/lint";
import { hoverTooltip } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import {
  hooksSubSchema,
  validateHooksJson,
  type JsonDiagnostic,
} from "./schema";

type SchemaNode = {
  description?: string;
  properties?: Record<string, SchemaNode>;
  items?: SchemaNode;
  $ref?: string;
} & Record<string, unknown>;

/** JSON Pointer 单段解码（RFC 6901：~0→~、~1→/） */
function decodeSegment(seg: string): string {
  return seg.replace(/~1/g, "/").replace(/~0/g, "~");
}

/**
 * JSON Pointer → doc 字符区间（顺序文本搜索；失败返回 null）。
 * 数字段按「cursor 起第 n 个 '[' 之后」近似——hooks 数组元素均为对象，
 * 下划线收敛到目标行即达标，不做精确 AST 定位。
 */
export function pointerToRange(
  doc: string,
  pointer: string,
): { from: number; to: number } | null {
  if (!pointer) return null;
  const segments = pointer.split("/").slice(1).map(decodeSegment);
  let cursor = 0;
  for (const seg of segments) {
    if (/^\d+$/.test(seg)) {
      let rest = Number(seg);
      let pos = cursor;
      while (rest-- >= 0) {
        pos = doc.indexOf("[", pos);
        if (pos < 0) return null;
        pos += 1;
      }
      cursor = pos;
    } else {
      const pos = doc.indexOf(`"${seg}"`, cursor);
      if (pos < 0) return null;
      cursor = pos + seg.length + 2; // 越过键引号对
    }
  }
  // 叶子下划区间：键后至最近的行尾/逗号/闭合符
  const stops = ["\n", ",", "}", "]"]
    .map((c) => doc.indexOf(c, cursor))
    .filter((p) => p >= 0);
  const to = stops.length ? Math.min(...stops) : doc.length;
  return { from: cursor, to: Math.max(to, cursor) };
}

/** schema 诊断 → CM6 Diagnostic（pointer 定位失败退回整文档） */
function toCmDiagnostic(doc: string, d: JsonDiagnostic): Diagnostic {
  const range = d.pointer ? pointerToRange(doc, d.pointer) : null;
  return {
    from: range?.from ?? 0,
    to: range?.to ?? doc.length,
    severity: "error",
    message: d.message,
  };
}

/** lint 真值源（纯函数，供 hooksSchemaLinter 包装与测试直驱） */
export function lintHooksSchemaDoc(doc: string): Diagnostic[] {
  const { isValid, diagnostics } = validateHooksJson(doc);
  return isValid ? [] : diagnostics.map((d) => toCmDiagnostic(doc, d));
}

/** hooks 子 schema 校验波浪线（delay 300ms，与原 jsonSchemaLinter 外层包装一致） */
export function hooksSchemaLinter(): Extension {
  return linter((view) => lintHooksSchemaDoc(view.state.doc.toString()), {
    delay: 300,
  });
}

/** 本地 $ref 解析（仅 hooksSubSchema 内部 `#/$defs/*` 形态，8 层防环） */
function resolveRef(node: SchemaNode): SchemaNode {
  let cur = node;
  let guard = 0;
  while (typeof cur.$ref === "string" && cur.$ref.startsWith("#/$defs/") && guard++ < 8) {
    const name = cur.$ref.slice("#/$defs/".length);
    const defs = (hooksSubSchema as unknown as { $defs: Record<string, SchemaNode> }).$defs;
    if (!defs[name]) return cur;
    cur = defs[name];
  }
  return cur;
}

/** 键路径 → 子 schema（逐段下钻 properties/数组 items + $ref 解析；失败 null） */
export function resolveSchemaPath(path: string[]): SchemaNode | null {
  let node = hooksSubSchema as unknown as SchemaNode;
  for (const seg of path) {
    node = resolveRef(node);
    if (node.properties?.[seg]) {
      node = node.properties[seg];
      continue;
    }
    if (/^\d+$/.test(seg) && node.items && typeof node.items === "object") {
      node = node.items;
      continue;
    }
    return null;
  }
  return resolveRef(node);
}

/**
 * pos 之前最近的键路径（向前回溯 `"key":` 形态，最多 6 层，窗口 4096 字符）。
 * 数组索引段不入路径（由 resolveSchemaPath 的 items 分支按数字段兜）；
 * 语义 = 悬停点所在对象链上的键序列。
 */
export function pathAt(doc: string, pos: number): string[] {
  const path: string[] = [];
  let end = pos;
  for (let depth = 0; depth < 6; depth++) {
    const start = Math.max(0, end - 4096);
    const window = doc.slice(start, end);
    const re = /"((?:[^"\\]|\\.)*)"\s*:/g;
    let last: RegExpExecArray | null = null;
    let m: RegExpExecArray | null;
    while ((m = re.exec(window))) last = m;
    if (!last) break;
    path.unshift(last[1]);
    end = start + last.index;
  }
  return path;
}

/** 悬停文案真值源（纯函数，供测试直驱；无 description 返回 null 不弹层） */
export function resolveHoverDescription(doc: string, pos: number): string | null {
  const node = resolveSchemaPath(pathAt(doc, pos));
  return node?.description ?? null;
}

/** hooks 子 schema 悬停浮层（键 description；布局内联样式，色值交 CM tooltip 主题） */
export function hooksSchemaHover(): Extension {
  return hoverTooltip(
    (view, pos) => {
      const description = resolveHoverDescription(view.state.doc.toString(), pos);
      if (!description) return null;
      return {
        pos,
        end: pos,
        above: true,
        create: () => {
          const dom = document.createElement("div");
          dom.textContent = description;
          dom.style.maxWidth = "360px";
          dom.style.padding = "2px 6px";
          return { dom };
        },
      };
    },
    { hoverTime: 300 },
  );
}
