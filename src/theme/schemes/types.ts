// schemes/types — 配色方案接口定义（权威消费注释所在，决策 D8）
//
// 消费注释粒度：区域级（语义 + 消费区域/组件），行号不入注释（决策 D8）；
// 高频 token 标引用计数 +「以 grep 为准」。
// linear.ts 等值文件的对象字面量标注 : ColorScheme 后，
// 编辑器 hover 槽位即显示本文件的 JSDoc——注释单点不漂移，新方案零注释负担。
// 内置默认方案 linear.ts 文件头另含 fail-safe 交叉引用（启动链静态硬编码色登记），
// 本文件为权威，两者保持一致。

import type { Extension } from "@codemirror/state";

/** 配色方案（Color Scheme）——一套完整配色定义的注册单元，仅暗色系（项目定位约束） */
export interface ColorScheme {
  /** 方案唯一 id——settings.json colorScheme 段取值（缺省/未知 → 回退 linear） */
  id: string;
  /** 展示名（未来设置 UI 枚举用，D3 预留） */
  label: string;
  /** UI token 槽位——组件经 theme/colors.ts facade 引用（硬约束 #6） */
  ui: UiTokens;
  /** xterm 调色板 25 键——经 panels/terminal/theme.ts adapter 展开进 xterm ITheme */
  terminal: TerminalPalette;
  /** CM 主题引用 + 覆盖——经 overrides.ts editorTheme/editorColorOverrides 应用 */
  editor: EditorScheme;
  /** 三方库 CSS 变量覆盖——经 overrides.ts dockviewVarStyle/allotmentVarStyle 内联注入 */
  libraries: LibraryOverrides;
}

/** UI token 槽位——组件经 theme/colors.ts facade 引用 token，禁止硬编码颜色 */
export interface UiTokens {
  /** 文件名 git 状态色——Commit 视图文件名（CommitFileList）、FileIcon、FileTree 行内状态色 */
  gitFile: {
    /** 已修改文件 */ modified: string;
    /** 已暂存（added）文件 */ added: string;
    /** 未跟踪文件 */ untracked: string;
    /** 已删除文件 */ deleted: string;
    /** 已重命名文件 */ renamed: string;
    /** 冲突文件 */ conflict: string;
    /** 被忽略文件 */ ignored: string;
  };
  /** 行内 diff 边栏色——编辑器/diff 面板 gutter 标记（editor/gitGutter.ts） */
  gitGutter: {
    /** 修改行标记 */ modified: string;
    /** 新增行标记 */ added: string;
    /** 删除行标记 */ deleted: string;
  };
  /** 文件浏览器通用色——文件树（FileTree）+ agentStatus/agentHistory/commit 视图借用 */
  explorer: {
    /** 树背景 */ bg: string;
    /** 树文字 */ fg: string;
    /** 悬停背景 */ hover: string;
    /** 折叠箭头 */ arrowClosed: string;
    /** 展开箭头 */ arrowOpen: string;
  };
  /** 侧栏配色——侧栏树/活动栏/agent 行/右键菜单/树形引导线（27 处 10 文件，以 grep 为准） */
  sidebar: {
    /** 侧栏背景 */ bg: string;
    /** 侧栏文字 */ fg: string;
    /** 悬停背景 */ hover: string;
    /** 选中背景 */ selected: string;
    /** 分隔边框 */ border: string;
    /** 右键菜单边框 */ contextMenuBorder: string;
    /** 右键菜单阴影 */ contextMenuShadow: string;
    /** 树形引导线（agent 侧栏层级缩进竖线） */ treeGuide: string;
  };
  /** 沙箱错误横幅——ExplorerPanel 路径沙箱拒绝提示 */
  errorBanner: {
    /** 横幅背景 */ bg: string;
    /** 横幅边框 */ border: string;
    /** 横幅文字 */ fg: string;
  };
  /** 用量条分段色——AgentStatusRow 上下文用量条（阈值 ≥90/≥70/≥50 由组件逻辑决定） */
  agentStatusUsage: {
    /** 低用量（<50%） */ low: string;
    /** 中用量（50-70%） */ medium: string;
    /** 高用量（70-90%） */ high: string;
    /** 临界用量（≥90%） */ critical: string;
  };

  // --- 标量（27 键）---
  /** 全部面板背景（21 处，以 grep 为准） */
  panelBg: string;
  /** 右键菜单底色（4 处） */
  sidebarBg: string;
  /** 页签按钮/弹窗次级背景（3 处） */
  secondaryBg: string;
  /** App 根容器背景 */
  appBg: string;
  /** 全局背景 → ROOT_CSS_VARS → --sl-bg-primary（App.css :root 注入） */
  appBgPrimary: string;
  /** 全局默认文字色 → ROOT_CSS_VARS → --sl-fg-primary（收编自 App.css:6） */
  appFg: string;
  /** 编辑器类面板容器背景（5 处） */
  editorBg: string;
  /** 侧栏/hooks 配置面板主要文字（27 处，以 grep 为准） */
  sidebarFg: string;
  /** 错误文案/状态（17 处） */
  errorFg: string;
  /** 占位符/禁用项/关闭按钮（5 处） */
  placeholderFg: string;
  /** 按钮文字（7 处） */
  buttonFg: string;
  /** 次要说明文字（10 处） */
  dimFg: string;
  /** 输入框背景（11 处） */
  inputBg: string;
  /** 输入框/卡片边框——全应用最高频（34 处，以 grep 为准） */
  inputBorder: string;
  /** 聚焦边框/活动指示条（10 处） */
  focusBorder: string;
  /** 列表/树选中背景（6 处） */
  activeSelectionBg: string;
  /** 分隔线（8 处） */
  separatorBg: string;
  /** 右键菜单边框（5 处） */
  contextMenuBorder: string;
  /** 弹窗遮罩阴影 */
  shadowMenu: string;
  /** 「加载中…」文案（15 处） */
  htmlPanelLoadingFg: string;
  /** HTML 预览 iframe 白底 */
  htmlPanelIframeBg: string;
  /** 强调底色上的文字（收编 JsonMode 事件导航 hover 硬编码） */
  onAccentFg: string;
  /** 文件树/历史行选中背景 */
  explorerSelectionBg: string;
  /** 强调派生前景色——活动栏激活图标/状态行模型段文字 */
  accentFg: string;
  /** 选中行 hover（accent-dim-2）——导航树/文件树/侧栏选中行悬停背景 */
  selectionHoverBg: string;
  /** 自绘标题栏 chrome 底（明度阶梯 l2）——TitleBar 容器背景 */
  titlebarBg: string;
  /** 自绘标题栏关闭钮 hover 底（UI-301 定值）——TitleBar 关闭按钮 hover 背景 */
  titlebarCloseHover: string;
}

/** xterm 调色板——25 键，ITheme 兼容（经 panels/terminal/theme.ts adapter 展开进 ITheme） */
export interface TerminalPalette {
  /** 默认前景色 */ foreground: string;
  /** 默认背景色 */ background: string;
  /** 光标色 */ cursor: string;
  /** 光标底色（光标悬于字符上时字符的前景色） */ cursorAccent: string;
  /** 选中文本背景 */ selectionBackground: string;
  /** 选中文本前景 */ selectionForeground: string;
  /** 滚动条滑块（默认/hover/激活）——ITheme 原生支持，显式化原运行期派生值 */
  scrollbarSliderBackground: string;
  scrollbarSliderHoverBackground: string;
  scrollbarSliderActiveBackground: string;
  /** ANSI 16 色基本色（0-7） */
  black: string;
  red: string;
  green: string;
  yellow: string;
  blue: string;
  magenta: string;
  cyan: string;
  white: string;
  /** ANSI 16 色亮色系（8-15） */
  brightBlack: string;
  brightRed: string;
  brightGreen: string;
  brightYellow: string;
  brightBlue: string;
  brightMagenta: string;
  brightCyan: string;
  brightWhite: string;
}

/** CM 主题引用 + 覆盖——4 个 oneDark 导入点经 overrides.ts 替换为 editorTheme + editorColorOverrides() */
export interface EditorScheme {
  /** CM 基础主题引用——linear = oneDark（@codemirror/theme-one-dark 直 import 透出，D6） */
  theme: Extension;
  /** 编辑器颜色覆盖（EditorView.theme 规则，dark: true） */
  overrides: {
    /** 编辑器背景——对齐 ui.editorBg */
    background: string;
    /** lint 诊断色——JsonMode 语法/schema 波浪线（@codemirror/lint） */
    lint: {
      /** 错误波浪线 */ error: string;
      /** 警告波浪线 */ warning: string;
      /** 信息波浪线 */ info: string;
      /** 提示波浪线 */ hint: string;
      /** lint 消息激活背景 */ activeBackground: string;
      /** lint 工具提示背景 */ tooltipBackground: string;
      /** lint 工具提示边框 */ tooltipBorder: string;
    };
    /** 搜索匹配高亮——editor/diff/gitshow 面板（highlightSelectionMatches） */
    searchMatch: {
      /** 匹配文本背景 */ match: string;
      /** 匹配文本描边 */ matchOutline: string;
      /** 选中匹配背景 */ selected: string;
      /** 多匹配整体背景 */ selectionMatch: string;
    };
    /** 语法高亮 token 色——CM 正文 token 着色（HighlightStyle 映射，经 overrides.ts editorSyntaxHighlight 应用） */
    syntax: {
      /** 属性名 */ property: string;
      /** 字符串 */ string: string;
      /** 数字 */ number: string;
      /** 关键字 */ keyword: string;
      /** 函数名 */ function: string;
      /** 类型名 */ type: string;
      /** 运算符 */ operator: string;
      /** 标点 */ punctuation: string;
      /** 注释 */ comment: string;
    };
    /** 正文前景色——editorColorOverrides .cm-content 规则 */
    plainText: string;
    /** 行号前景色——editorColorOverrides gutter 规则 */
    lineNumber: string;
    /** 活跃行行号前景色——editorColorOverrides gutter 规则 */
    lineNumberActive: string;
  };
}

/** 三方库 CSS 变量覆盖——dockview + allotment（值随方案切换） */
export interface LibraryOverrides {
  /** dockview CSS 变量（"--dv-*" → 值，20 条）——PageDockviewHost 挂载点内联注入（样式表加载顺序免疫） */
  dockview: Record<string, string>;
  /** allotment CSS 变量（2 键）——Workspace 根容器注入，CSS 变量继承覆盖内层 SideBarArea */
  allotment: {
    /** 分割线颜色 → --separator-border */ separatorBorder: string;
    /** 聚焦边框 → --focus-border */ focusBorder: string;
  };
}
