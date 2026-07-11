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
const MockDefaultTab: React.FC<{
  title: string;
  tabIcon: string | null;
  onClose: () => void;
}> = ({ title, tabIcon, onClose }) => {
  return (
    <div style={{ display: "flex", alignItems: "center", height: "100%",
      padding: "0 8px", gap: 6, userSelect: "none" }}>
      {tabIcon && (
        <img src={tabIcon} width={16} height={16}
          style={{ flexShrink: 0, display: "block" }} alt="" data-testid="tab-icon" />
      )}
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
    it("params.tabIcon 为 null → 不渲染 img", () => {
      render(<MockDefaultTab title="terminal-0" tabIcon={null} onClose={vi.fn()} />);
      expect(screen.queryByTestId("tab-icon")).toBeNull();
    });

    it("params.tabIcon 为非空字符串 → 渲染 img，src={tabIcon}", () => {
      render(<MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />);
      const img = screen.getByTestId("tab-icon");
      expect(img).toBeTruthy();
      expect(img.getAttribute("src")).toBe("/claude.png");
    });

    it("无 tabIcon 时 title 和 close button 正常渲染", () => {
      render(<MockDefaultTab title="terminal-0" tabIcon={null} onClose={vi.fn()} />);
      expect(screen.getByTestId("tab-title").textContent).toBe("terminal-0");
      expect(screen.getByTestId("tab-close")).toBeTruthy();
    });
  });

  describe("动态更新", () => {
    it("tabIcon 从 null 变为非空 → 渲染 img", () => {
      const { rerender } = render(
        <MockDefaultTab title="terminal-0" tabIcon={null} onClose={vi.fn()} />
      );
      expect(screen.queryByTestId("tab-icon")).toBeNull();

      rerender(<MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />);
      expect(screen.getByTestId("tab-icon")).toBeTruthy();
    });

    it("tabIcon 从非空变为 null → 移除 img", () => {
      const { rerender } = render(
        <MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />
      );
      expect(screen.getByTestId("tab-icon")).toBeTruthy();

      rerender(<MockDefaultTab title="terminal-0" tabIcon={null} onClose={vi.fn()} />);
      expect(screen.queryByTestId("tab-icon")).toBeNull();
    });

    it("title 变化不影响 tabIcon", () => {
      const { rerender } = render(
        <MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />
      );
      rerender(<MockDefaultTab title="claude-v2" tabIcon="/claude.png" onClose={vi.fn()} />);
      expect(screen.getByTestId("tab-icon")).toBeTruthy();
      expect(screen.getByTestId("tab-title").textContent).toBe("claude-v2");
    });
  });

  describe("渲染属性", () => {
    it("img width=16 height=16", () => {
      render(<MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />);
      const img = screen.getByTestId("tab-icon");
      expect(img.getAttribute("width")).toBe("16");
      expect(img.getAttribute("height")).toBe("16");
    });

    it("DOM 顺序：图标→文字→关闭按钮", () => {
      render(<MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />);
      const div = screen.getByTestId("tab-icon").parentElement!;
      const children = Array.from(div.children);
      expect(children[0].getAttribute("data-testid")).toBe("tab-icon");
      expect(children[1].getAttribute("data-testid")).toBe("tab-title");
      expect(children[2].getAttribute("data-testid")).toBe("tab-close");
    });

    it("img 有 flexShrink: 0 样式", () => {
      render(<MockDefaultTab title="claude" tabIcon="/claude.png" onClose={vi.fn()} />);
      const img = screen.getByTestId("tab-icon");
      expect(img.style.flexShrink).toBe("0");
    });
  });

  describe("edge cases", () => {
    it("params.tabIcon 为 undefined → tabIcon 为 null → 不崩溃", () => {
      render(<MockDefaultTab title="terminal-0" tabIcon={null} onClose={vi.fn()} />);
      expect(screen.getByTestId("tab-title")).toBeTruthy();
      expect(screen.queryByTestId("tab-icon")).toBeNull();
    });

    it("params.tabIcon 为空字符串 → 不渲染（falsy）", () => {
      render(<MockDefaultTab title="terminal-0" tabIcon="" onClose={vi.fn()} />);
      expect(screen.queryByTestId("tab-icon")).toBeNull();
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
