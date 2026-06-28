// ErrorBoundary — 通用错误边界组件
//
// 两种模式：
// - "fullscreen"：顶层兜底，捕获后全屏显示错误（100vh）
// - "inline"：单页面包裹，捕获后显示占位 UI，不影响其他页面
//
// 硬约束 #1：本组件为纯 UI，不涉及 OS/文件/进程调用

import React from "react";
import {
  PANEL_BG, ERROR_FG, DIM_FG, SECONDARY_BG,
  SEPARATOR_BG, PLACEHOLDER_FG,
} from "../theme";

interface ErrorBoundaryProps {
  children: React.ReactNode;
  /** 展示模式：fullscreen 全屏兜底 | inline 页面级隔离 */
  variant?: "fullscreen" | "inline";
}

interface ErrorBoundaryState {
  error: Error | null;
}

export class ErrorBoundary extends React.Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  static defaultProps = { variant: "fullscreen" as const };

  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo): void {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).__sltermError = {
      message: error.message,
      stack: error.stack,
      componentStack: info.componentStack,
    };
    console.error("[slTerminal] 渲染错误:", error, info);
  }

  render(): React.ReactNode {
    if (this.state.error) {
      if (this.props.variant === "inline") {
        return (
          <div
            style={{
              width: "100%",
              height: "100%",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              background: PANEL_BG,
              color: ERROR_FG,
              fontFamily: "monospace",
              fontSize: 13,
              gap: 8,
              padding: 16,
            }}
          >
            <div style={{ fontSize: 16, fontWeight: 600 }}>
              页面渲染出错
            </div>
            <div
              style={{
                maxWidth: 480,
                textAlign: "center",
                color: PLACEHOLDER_FG,
                fontSize: 12,
                lineHeight: 1.5,
              }}
            >
              该操作页面因渲染错误无法显示，其他页面不受影响。
              请切换到其他页面或重启应用。
            </div>
            <details style={{ marginTop: 8, maxWidth: "100%" }}>
              <summary
                style={{
                  cursor: "pointer",
                  fontSize: 12,
                  color: DIM_FG,
                }}
              >
                查看错误详情
              </summary>
              <pre
                style={{
                  marginTop: 8,
                  padding: 8,
                  background: SECONDARY_BG,
                  border: `1px solid ${SEPARATOR_BG}`,
                  borderRadius: 4,
                  whiteSpace: "pre-wrap",
                  fontSize: 11,
                  maxHeight: 200,
                  overflow: "auto",
                }}
              >
                {this.state.error.message}
                {"\n\n"}
                {this.state.error.stack}
              </pre>
            </details>
          </div>
        );
      }

      // fullscreen 模式：全屏兜底（与旧行为一致）
      return (
        <div
          style={{
            padding: 20,
            color: ERROR_FG,
            background: PANEL_BG,
            height: "100vh",
            fontFamily: "monospace",
            fontSize: 13,
          }}
        >
          <h2>应用渲染错误</h2>
          <pre style={{ whiteSpace: "pre-wrap" }}>
            {this.state.error.message}
          </pre>
          <pre
            style={{ whiteSpace: "pre-wrap", color: DIM_FG, fontSize: 11 }}
          >
            {this.state.error.stack}
          </pre>
        </div>
      );
    }
    return this.props.children;
  }
}
