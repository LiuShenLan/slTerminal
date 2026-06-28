import { describe, it, expect, beforeEach } from "vitest";
import { useLayout } from "../stores/layout";

describe("useLayout store", () => {
  beforeEach(() => {
    useLayout.setState({ activePageId: null });
  });

  it("初始 activePageId 为 null", () => {
    expect(useLayout.getState().activePageId).toBeNull();
  });

  it("setActivePage 设置页面 ID", () => {
    useLayout.getState().setActivePage("page-1");
    expect(useLayout.getState().activePageId).toBe("page-1");
  });

  it("setActivePage(null) 行为正确", () => {
    useLayout.getState().setActivePage("page-1");
    useLayout.getState().setActivePage(null);
    expect(useLayout.getState().activePageId).toBeNull();
  });

  it("重复设置同一 pageId 不报错", () => {
    useLayout.getState().setActivePage("page-1");
    useLayout.getState().setActivePage("page-1");
    expect(useLayout.getState().activePageId).toBe("page-1");
  });
});
