// appError.ts — AppError 解析统一入口（Stage 08 跨边界契约）
//
// 契约（写死）：
// - parseAppError(err: unknown): { variant: string; message: string } | null
//   ——按 camelCase 变体名解析后端 AppError 序列化形态（src-tauri/src/error.rs，
//   serde(rename_all = "camelCase")）；非 AppError 形态返回 null
// - getErrorMessage(err: unknown): string ——提取用户可读消息，兜底 String(err)
//
// 后端 AppError 序列化形态（serde camelCase 枚举，字段变体 → { <variant>: payload }）：
// - 元组变体（如 Pty("...")）→ { pty: "..." }，message = 字符串值
// - 结构体变体（IoKind { kind, message }）→ { ioKind: { kind, message } }，message = 对象.message
//
// 变体名清单须与 error.rs 的 AppError 枚举逐一对应（当前 11 变体，含 ConfigParse）

/** AppError 全部变体名（camelCase，与后端 serde 序列化一一对应） */
export const APP_ERROR_VARIANTS = [
  "ioKind",
  "pty",
  "git",
  "serde",
  "unknown",
  "sessionNotFound",
  "taskJoin",
  "notify",
  "validation",
  "pathNotAllowed",
  "configParse",
] as const;

/** parseAppError 返回的解析结果 */
export interface ParsedAppError {
  variant: string;
  message: string;
}

/** 从变体 payload 提取用户可读消息：字符串直取；对象取 message 字段；其余 String() */
function extractMessage(payload: unknown): string {
  if (typeof payload === "string") return payload;
  if (typeof payload === "object" && payload !== null) {
    const msg = (payload as Record<string, unknown>).message;
    if (typeof msg === "string") return msg;
  }
  return String(payload);
}

/**
 * 解析后端 AppError 序列化形态 → { variant, message }。
 * 非对象 / 无已知变体键 → null（交由调用方兜底）。
 */
export function parseAppError(err: unknown): ParsedAppError | null {
  if (typeof err !== "object" || err === null) return null;
  const record = err as Record<string, unknown>;
  for (const variant of APP_ERROR_VARIANTS) {
    const payload = record[variant];
    if (payload !== undefined) {
      return { variant, message: extractMessage(payload) };
    }
  }
  return null;
}

/** 提取用户可读错误消息：AppError → message 字段；其它 → String(err) 兜底 */
export function getErrorMessage(err: unknown): string {
  const parsed = parseAppError(err);
  if (parsed) return parsed.message;
  return String(err);
}
