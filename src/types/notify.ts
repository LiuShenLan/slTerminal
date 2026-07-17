/** 文件系统事件负载 —— 对应后端 notify 模块推送的 fs-event 数据结构 */
export interface FsEventPayload {
  paths: string[];
  kind: string;
  detail?: string;
}
