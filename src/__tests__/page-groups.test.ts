// pageGroups 纯函数族测试（CP-004/S11 页组分组模型协议）
// ——panelIdInPage/pageOfPanelId 往返、panelBelongsToGroup 越界判定、
//    pageGroupId/pageIdOfGroupId、panelsOfPage 页过滤
import { describe, it, expect } from "vitest";
import {
  pageGroupId,
  pageIdOfGroupId,
  panelIdInPage,
  pageOfPanelId,
  panelsOfPage,
  panelBelongsToGroup,
} from "../workspace/pageGroups";

const PAGE = "page-1788531106918-2";

describe("pageGroups 页组 id", () => {
  it("pageGroupId 产出 page- 前缀页组 id,pageIdOfGroupId 可逆", () => {
    const gid = pageGroupId(PAGE);
    expect(gid).toBe(`page-${PAGE}`);
    expect(pageIdOfGroupId(gid)).toBe(PAGE);
  });

  it("pageIdOfGroupId 对非页组 id 返回 null", () => {
    expect(pageIdOfGroupId("group-abc")).toBeNull();
    // dockview 自生组/空 id 均非页组
    expect(pageIdOfGroupId("")).toBeNull();
  });
});

describe("panelIdInPage / pageOfPanelId 往返", () => {
  it("页前缀 + localId 构造与解析往返", () => {
    const pid = panelIdInPage(PAGE, "terminal-0");
    expect(pid).toBe(`${PAGE}:terminal-0`);
    expect(pageOfPanelId(pid)).toBe(PAGE);
  });

  it("pageOfPanelId 只切首个冒号前段(pageId 含中划线不受影响)", () => {
    expect(pageOfPanelId(`${PAGE}:terminal-7`)).toBe(PAGE);
    expect(pageOfPanelId("a:b:c")).toBe("a");
  });

  it("无冒号面板 id(旧格式/裸 id)→ null", () => {
    expect(pageOfPanelId("terminal-page-1-0")).toBeNull();
    expect(pageOfPanelId("settings-page-1")).toBeNull();
    expect(pageOfPanelId("")).toBeNull();
  });
});

describe("panelBelongsToGroup 越界判定", () => {
  it("面板页前缀与页组 pageId 一致 → 归属", () => {
    expect(panelBelongsToGroup(panelIdInPage(PAGE, "terminal-0"), pageGroupId(PAGE))).toBe(true);
  });

  it("面板属其他页 → 越界(跨页组)", () => {
    const other = "page-other-1";
    expect(panelBelongsToGroup(panelIdInPage(other, "terminal-0"), pageGroupId(PAGE))).toBe(false);
  });

  it("无页前缀面板落任何页组 → 越界(无法归组)", () => {
    expect(panelBelongsToGroup("terminal-page-1-0", pageGroupId(PAGE))).toBe(false);
  });

  it("目标组非页组(dockview 自生组)→ 恒越界", () => {
    expect(panelBelongsToGroup(panelIdInPage(PAGE, "terminal-0"), "group-abc")).toBe(false);
  });
});

describe("panelsOfPage 页过滤", () => {
  it("按页前缀过滤 api.panels,其他页/裸 id 面板不混入", () => {
    const fakeApi = {
      panels: [
        { id: panelIdInPage(PAGE, "terminal-0") },
        { id: panelIdInPage(PAGE, "editor-1") },
        { id: panelIdInPage("page-other-9", "terminal-0") },
        { id: "terminal-page-1-0" },
      ],
    } as unknown as Parameters<typeof panelsOfPage>[0];
    const result = panelsOfPage(fakeApi, PAGE);
    expect(result.map((p) => p.id)).toEqual([
      panelIdInPage(PAGE, "terminal-0"),
      panelIdInPage(PAGE, "editor-1"),
    ]);
  });

  it("空面板集合返回空数组", () => {
    const fakeApi = { panels: [] } as unknown as Parameters<typeof panelsOfPage>[0];
    expect(panelsOfPage(fakeApi, PAGE)).toEqual([]);
  });
});
