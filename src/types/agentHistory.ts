// agent 历史会话 DTO 类型定义（硬约束 #4：与后端 agent_history/mod.rs 双边对应）
//
// 跨边界契约——TS camelCase ↔ Rust snake_case，八键与后端 serde 输出逐字一致，防字段漂移。

/** 标题来源（开放字符串——claude 值集 customTitle/aiTitle/summary/firstPrompt/none 不变；UI 不消费具体值） */
export type TitleSource = string;

/** 历史会话元数据 DTO（后端 AgentHistorySession 八字段，cliId 由 provider 打标） */
export interface AgentHistorySession {
  /** 会话 ID（文件名主干 = UUID） */
  sessionId: string;
  /** 会话启动时工作目录（从 JSONL 内容解析，不反解码目录名） */
  cwd: string | null;
  /** 标题（后端已按回退链解析；null → 前端显示 sessionId 前 8 位） */
  title: string | null;
  /** 标题来源（开放字符串，UI 不消费具体值） */
  titleSource: TitleSource;
  /** 首条可见 user prompt（≤200 字符，后端截断） */
  firstPrompt: string | null;
  /** 文件修改时间（毫秒时间戳） */
  mtimeMs: number;
  /** cwd 目录当前是否存在（cwd 为 null 时恒 false） */
  cwdExists: boolean;
  /** 来源 CLI 标识（provider 打标——聚合扫描结果由各 provider 写入自己的 cliId） */
  cliId: string;
}
