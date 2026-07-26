// consts.ts — F5 Agent Status 模块常量单点
// 跨边界契约：CLAUDE_CONTEXT_LIMIT 仅在此文件定义，其他文件引用不复制。

/** Claude 上下文窗口上限（tokens）。用于用量条百分比计算。 */
export const CLAUDE_CONTEXT_LIMIT = 200_000;
