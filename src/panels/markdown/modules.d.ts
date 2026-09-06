// modules.d.ts — 无类型依赖的本地声明（md 渲染管线）
//
// markdown-it-texmath / markdown-it-task-lists 为 CJS 无类型包，此处按实际
// 导出形态声明（interop 由 bundler 处理）。升级上述包时核对本声明。

declare module "markdown-it-texmath" {
  import type MarkdownIt from "markdown-it";
  type MarkdownItInstance = InstanceType<typeof MarkdownIt>;
  interface TexmathOptions {
    /** KaTeX 引擎（katex 默认导出） */
    engine?: unknown;
    /** 分隔符模式：'dollars' | 'brackets' | 'dollars+brackets' 或数组 */
    delimiters?: string | string[];
    /** 透传 katex.render 选项（throwOnError 默认 false 由插件兜底） */
    katexOptions?: Record<string, unknown>;
  }
  /** 插件函数（md.use(texmath, opts)）+ 静态 use(engine) */
  const texmath: ((md: MarkdownItInstance, options?: TexmathOptions) => void) & {
    use(engine: unknown): void;
  };
  export default texmath;
}

declare module "markdown-it-task-lists" {
  import type MarkdownIt from "markdown-it";
  type MarkdownItInstance = InstanceType<typeof MarkdownIt>;
  interface TaskListOptions {
    /** 仅渲染带 checkbox 的列表项 */
    enabled?: boolean;
    /** checkbox 旁是否带无障碍 label */
    label?: boolean;
  }
  /** 插件函数（md.use(taskLists, opts)） */
  const plugin: (md: MarkdownItInstance, options?: TaskListOptions) => void;
  export default plugin;
}
