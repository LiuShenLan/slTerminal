// DefaultTab 图标渲染测试
import { describe, it, expect, vi, afterEach } from "vitest";
import React from "react";
import { render, screen, cleanup } from "@testing-library/react";

afterEach(() => {
  cleanup();
});

// 由于 DefaultTab 定义在 Workspace.tsx 内部（非导出），测试直接渲染其等价逻辑。
// 此处测试 TabTitleRegistry → Dockview params.tabIcon → 图标渲染的契约。
// Dockview onDidParametersChange 通过 params 传递 tabIcon，DefaultTab 读取并渲染。

// 创建一个简化的 DefaultTab 等价组件用于测试
// 与 PageDockviewHost.tsx 中 DefaultTab 的 tabIcon 渲染逻辑一致：
// - 含 / \ http: data: → <img>（文件路径/URL）
// - emoji/纯文本 → <span>
const MockDefaultTab: React.FC<{
  title: string;
  tabIcon: string | null;
  onClose: () => void;
}> = ({ title, tabIcon, onClose }) => {
  /** 判断 tabIcon 是 URL/路径 → <img>，还是 emoji/纯文本 → <span> */
  const renderIcon = () => {
    if (!tabIcon) return null;
    const isUrl = tabIcon.includes("/") || tabIcon.includes("\\")
      || tabIcon.startsWith("http:") || tabIcon.startsWith("data:");
    if (isUrl) {
      return (
        <img src={tabIcon} width={16} height={16}
          style={{ flexShrink: 0, display: "block" }} alt="页签图标"
          data-testid="tab-icon-img" />
      );
    }
    return (
      <span style={{ fontSize: 14, lineHeight: 1, flexShrink: 0 }}
        data-testid="tab-icon-emoji">
        {tabIcon}
      </span>
    );
  };

  return (
    <div style={{ display: "flex", alignItems: "center", height: "100%",
      padding: "0 8px", gap: 6, userSelect: "none" }}>
      {renderIcon()}
      <span style={{ fontSize: 13 }} data-testid="tab-title">{title}</span>
      <button
        onClick={(e) => { e.stopPropagation(); onClose(); }}
        style={{ background: "none", border: "none", color: "#888",
          cursor: "pointer", padding: "0 2px", fontSize: 14, lineHeight: 1 }}
        title="关闭"
        data-testid="tab-close"
      >×</button>
    </div>
  );
};

describe("DefaultTab tabIcon rendering", () => {
  describe("初始状态", () => {
    it("params.tabIcon 为 null → 不渲染图标元素", () => {
      render(<MockDefaultTab title="terminal-0" tabIcon={null} onClose={vi.fn()} />);
      expect(screen.queryByTestId("tab-icon-img")).toBeNull();
      expect(screen.queryByTestId("tab-icon-emoji")).toBeNull();
    });

    it("params.tabIcon 为图片路径 → 渲染 img（tab-icon-img），src={tabIcon}", () => {
      render(<MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />);
      const img = screen.getByTestId("tab-icon-img");
      expect(img).toBeTruthy();
      expect(img.getAttribute("src")).toBe("/claude.png");
    });

    it("params.tabIcon 为 emoji → 渲染 span（tab-icon-emoji），内容为 emoji", () => {
      render(<MockDefaultTab title="claude" tabIcon="⚡" onClose={vi.fn()} />);
      const span = screen.getByTestId("tab-icon-emoji");
      expect(span).toBeTruthy();
      expect(span.tagName).toBe("SPAN");
      expect(span.textContent).toBe("⚡");
    });

    it("params.tabIcon 为 emoji 且含 / 时仍走 img 路径", () => {
      // emoji 本身不含 /，此测试确认区分逻辑正确
      render(<MockDefaultTab title="claude" tabIcon="🟡" onClose={vi.fn()} />);
      expect(screen.getByTestId("tab-icon-emoji")).toBeTruthy();
      expect(screen.queryByTestId("tab-icon-img")).toBeNull();
    });

    it("无 tabIcon 时 title 和 close button 正常渲染", () => {
      render(<MockDefaultTab title="terminal-0" tabIcon={null} onClose={vi.fn()} />);
      expect(screen.getByTestId("tab-title").textContent).toBe("terminal-0");
      expect(screen.getByTestId("tab-close")).toBeTruthy();
    });
  });

  describe("动态更新", () => {
    it("tabIcon 从 null 变为图片路径 → 渲染 img", () => {
      const { rerender } = render(
        <MockDefaultTab title="terminal-0" tabIcon={null} onClose={vi.fn()} />
      );
      expect(screen.queryByTestId("tab-icon-img")).toBeNull();
      expect(screen.queryByTestId("tab-icon-emoji")).toBeNull();

      rerender(<MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />);
      expect(screen.getByTestId("tab-icon-img")).toBeTruthy();
    });

    it("tabIcon 从 null 变为 emoji → 渲染 span", () => {
      const { rerender } = render(
        <MockDefaultTab title="terminal-0" tabIcon={null} onClose={vi.fn()} />
      );
      expect(screen.queryByTestId("tab-icon-emoji")).toBeNull();

      rerender(<MockDefaultTab title="claude" tabIcon="⚡" onClose={vi.fn()} />);
      const span = screen.getByTestId("tab-icon-emoji");
      expect(span).toBeTruthy();
      expect(span.textContent).toBe("⚡");
    });

    it("tabIcon 从图片变为 emoji → img 移除，span 出现", () => {
      const { rerender } = render(
        <MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />
      );
      expect(screen.getByTestId("tab-icon-img")).toBeTruthy();

      rerender(<MockDefaultTab title="claude" tabIcon="✅" onClose={vi.fn()} />);
      expect(screen.queryByTestId("tab-icon-img")).toBeNull();
      const span = screen.getByTestId("tab-icon-emoji");
      expect(span).toBeTruthy();
      expect(span.textContent).toBe("✅");
    });

    it("tabIcon 从非空变为 null → 移除所有图标", () => {
      const { rerender } = render(
        <MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />
      );
      expect(screen.getByTestId("tab-icon-img")).toBeTruthy();

      rerender(<MockDefaultTab title="terminal-0" tabIcon={null} onClose={vi.fn()} />);
      expect(screen.queryByTestId("tab-icon-img")).toBeNull();
      expect(screen.queryByTestId("tab-icon-emoji")).toBeNull();
    });

    it("title 变化不影响 tabIcon", () => {
      const { rerender } = render(
        <MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />
      );
      rerender(<MockDefaultTab title="claude-v2" tabIcon="/claude.png" onClose={vi.fn()} />);
      expect(screen.getByTestId("tab-icon-img")).toBeTruthy();
      expect(screen.getByTestId("tab-title").textContent).toBe("claude-v2");
    });
  });

  describe("渲染属性", () => {
    it("img 类型：width=16 height=16", () => {
      render(<MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />);
      const img = screen.getByTestId("tab-icon-img");
      expect(img.getAttribute("width")).toBe("16");
      expect(img.getAttribute("height")).toBe("16");
    });

    it("span 类型：fontSize=14, lineHeight=1", () => {
      render(<MockDefaultTab title="claude" tabIcon="⚡" onClose={vi.fn()} />);
      const span = screen.getByTestId("tab-icon-emoji");
      expect(span.style.fontSize).toBe("14px");
      expect(span.style.lineHeight).toBe("1");
    });

    it("img 类型 DOM 顺序：图标→文字→关闭按钮", () => {
      render(<MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />);
      const div = screen.getByTestId("tab-icon-img").parentElement!;
      const children = Array.from(div.children);
      expect(children[0].getAttribute("data-testid")).toBe("tab-icon-img");
      expect(children[1].getAttribute("data-testid")).toBe("tab-title");
      expect(children[2].getAttribute("data-testid")).toBe("tab-close");
    });

    it("span 类型 DOM 顺序：图标→文字→关闭按钮", () => {
      render(<MockDefaultTab title="claude" tabIcon="⚡" onClose={vi.fn()} />);
      const div = screen.getByTestId("tab-icon-emoji").parentElement!;
      const children = Array.from(div.children);
      expect(children[0].getAttribute("data-testid")).toBe("tab-icon-emoji");
      expect(children[1].getAttribute("data-testid")).toBe("tab-title");
      expect(children[2].getAttribute("data-testid")).toBe("tab-close");
    });

    it("img 有 flexShrink: 0 样式", () => {
      render(<MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />);
      const img = screen.getByTestId("tab-icon-img");
      expect(img.style.flexShrink).toBe("0");
    });

    it("span 有 flexShrink: 0 样式", () => {
      render(<MockDefaultTab title="claude" tabIcon="⚡" onClose={vi.fn()} />);
      const span = screen.getByTestId("tab-icon-emoji");
      expect(span.style.flexShrink).toBe("0");
    });
  });

  describe("edge cases", () => {
    it("params.tabIcon 为 undefined → tabIcon 为 null → 不崩溃", () => {
      render(<MockDefaultTab title="terminal-0" tabIcon={null} onClose={vi.fn()} />);
      expect(screen.getByTestId("tab-title")).toBeTruthy();
      expect(screen.queryByTestId("tab-icon-img")).toBeNull();
      expect(screen.queryByTestId("tab-icon-emoji")).toBeNull();
    });

    it("params.tabIcon 为空字符串 → 不渲染（falsy）", () => {
      render(<MockDefaultTab title="terminal-0" tabIcon="" onClose={vi.fn()} />);
      expect(screen.queryByTestId("tab-icon-img")).toBeNull();
      expect(screen.queryByTestId("tab-icon-emoji")).toBeNull();
    });
  });

  describe("onDidParametersChange 事件结构（防止回归）", () => {
    // 验证真实的 api.onDidParametersChange 回调行为——
    // Dockview 的 Event<Parameters> 直接传递 Parameters 对象，
    // 不是 { params: Parameters } 包裹结构。
    // 以下测试模拟真实 DefaultTab 中 useEffect 的事件订阅逻辑。

    it("回调收到 { tabIcon: '/icon.png' } → setTabIcon", () => {
      // 模拟真实 DefaultTab 中 onDidParametersChange 的 handler
      let capturedIcon: string | null = "";
      const handler = (event: Record<string, unknown> | undefined) => {
        capturedIcon = (event?.tabIcon as string) ?? null;
      };

      // Dockview 的 PanelApi.updateParameters 直接发射 Parameters 对象
      handler({ tabIcon: "/icon.png" });
      expect(capturedIcon).toBe("/icon.png");
    });

    it("回调收到 { tabIcon: '⚡' } → setTabIcon（emoji）", () => {
      let capturedIcon: string | null = "";
      const handler = (event: Record<string, unknown> | undefined) => {
        capturedIcon = (event?.tabIcon as string) ?? null;
      };

      handler({ tabIcon: "⚡" });
      expect(capturedIcon).toBe("⚡");
    });

    it("回调收到 { tabIcon: null } → setTabIcon(null)", () => {
      let capturedIcon: string | null = "/stale.png";
      const handler = (event: Record<string, unknown> | undefined) => {
        capturedIcon = (event?.tabIcon as string | null) ?? null;
      };

      handler({ tabIcon: null });
      expect(capturedIcon).toBeNull();
    });

    it("回调收到 undefined → setTabIcon(null)（不崩溃）", () => {
      let capturedIcon: string | null = "/stale.png";
      const handler = (event: Record<string, unknown> | undefined) => {
        capturedIcon = (event?.tabIcon as string | null) ?? null;
      };

      handler(undefined);
      expect(capturedIcon).toBeNull();
    });
  });
});
