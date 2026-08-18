/** 文件系统事件负载 —— 对应后端 notify 模块推送的 fs-event 数据结构 */
export interface FsEventPayload {
  paths: string[];
  kind: string;
  /** 事件附加详情（FE-13：Rust 必填，非可选） */
  detail: string;
}
