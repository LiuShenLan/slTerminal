// pageGroups 纯函数族测试（CP-004/S11 页组分组模型协议 + ADR-0020 派生归属）
// ——panelIdInPage/pageOfPanelId 往返、pageGroupId/pageIdOfGroupId、
//    pageIdOfGroup 派生归属（主组快车道/自生组慢车道/空壳 null）、
//    groupsOfPage 页内组枚举、resolvePageGroupForAdd 落组解析、
//    panelBelongsToGroup 归属判定（组对象语义）、panelsOfPage 页过滤
import { describe, it, expect } from "vitest";
import {
  pageGroupId,
  pageIdOfGroupId,
  panelIdInPage,
  pageOfPanelId,
  pageIdOfGroup,
  groupsOfPage,
  resolvePageGroupForAdd,
  panelsOfPage,
  panelBelongsToGroup,
} from "../workspace/pageGroups";

const PAGE = "page-1788531106918-2";

/** fake 组对象（pageIdOfGroup 消费面 = id + panels） */
function fakeGroup(id: string, panelIds: string[] = []) {
  return { id, panels: panelIds.map((pid) => ({ id: pid })) } as never;
}

/** fake 宿主 api（groups/getGroup/panels） */
function fakeApi(groups: Array<{ id: string; panels: Array<{ id: string }> }>, panels: Array<{ id: string }> = []) {
  return {
    groups,
    panels,
    getGroup: (id: string) => groups.find((g) => g.id === id),
  } as never;
}

describe("pageGroups 页组 id", () => {
  it("pageGroupId 产出 page- 前缀页组 id,pageIdOfGroupId 可逆", () => {
    const gid = pageGroupId(PAGE);
    expect(gid).toBe(`page-${PAGE}`);
    expect(pageIdOfGroupId(gid)).toBe(PAGE);
  });

  it("pageIdOfGroupId 对非页组 id 返回 null", () => {
    expect(pageIdOfGroupId("group-abc")).toBeNull();
    // dockview 自生组/空 id 均非主组
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

describe("pageIdOfGroup 派生归属（ADR-0020）", () => {
  it("主组 id 快车道：空主组归属仍成立（Watermark 载体）", () => {
    expect(pageIdOfGroup(fakeGroup(pageGroupId(PAGE)))).toBe(PAGE);
  });

  it("自生组慢车道：组内首面板页前缀派生", () => {
    const g = fakeGroup("4", [panelIdInPage(PAGE, "terminal-0"), panelIdInPage(PAGE, "editor-1")]);
    expect(pageIdOfGroup(g)).toBe(PAGE);
  });

  it("混组以首面板页为属主", () => {
    const g = fakeGroup("4", [panelIdInPage("page-b", "terminal-0"), panelIdInPage(PAGE, "terminal-1")]);
    expect(pageIdOfGroup(g)).toBe("page-b");
  });

  it("空自生组/无协议面板组 → null（无归属空壳）", () => {
    expect(pageIdOfGroup(fakeGroup("4"))).toBeNull();
    expect(pageIdOfGroup(fakeGroup("4", ["terminal-page-a-0"]))).toBeNull();
  });
});

describe("groupsOfPage 页内组枚举", () => {
  it("主组 + 分屏自生组同页命中,他页/空壳组排除", () => {
    const api = fakeApi([
      { id: pageGroupId(PAGE), panels: [{ id: panelIdInPage(PAGE, "terminal-0") }] },
      { id: "4", panels: [{ id: panelIdInPage(PAGE, "editor-1") }] },
      { id: pageGroupId("page-b"), panels: [{ id: panelIdInPage("page-b", "terminal-0") }] },
      { id: "9", panels: [] },
    ]);
    const result = groupsOfPage(api, PAGE);
    expect(result.map((g) => g.id)).toEqual([pageGroupId(PAGE), "4"]);
  });

  it("页无任何组 → 空数组", () => {
    const api = fakeApi([{ id: pageGroupId("page-b"), panels: [] }]);
    expect(groupsOfPage(api, PAGE)).toEqual([]);
  });
});

describe("resolvePageGroupForAdd 落组解析", () => {
  it("主组在 → 主组（常规路径）", () => {
    const api = fakeApi([
      { id: pageGroupId(PAGE), panels: [] },
      { id: "4", panels: [{ id: panelIdInPage(PAGE, "terminal-0") }] },
    ]);
    expect(resolvePageGroupForAdd(api, PAGE)?.id).toBe(pageGroupId(PAGE));
  });

  it("主组缺失（分屏后拖空删除）→ 页内首组", () => {
    const api = fakeApi([
      { id: "4", panels: [{ id: panelIdInPage(PAGE, "terminal-0") }] },
      { id: "5", panels: [{ id: panelIdInPage(PAGE, "editor-1") }] },
    ]);
    expect(resolvePageGroupForAdd(api, PAGE)?.id).toBe("4");
  });

  it("页无任何组 → null（调用方显式失败，不落活跃组防错页）", () => {
    const api = fakeApi([{ id: pageGroupId("page-b"), panels: [] }]);
    expect(resolvePageGroupForAdd(api, PAGE)).toBeNull();
  });
});

describe("panelBelongsToGroup 归属判定（组对象语义）", () => {
  it("组派生属主页与面板页前缀一致 → 归属", () => {
    expect(panelBelongsToGroup(panelIdInPage(PAGE, "terminal-0"), fakeGroup(pageGroupId(PAGE)))).toBe(true);
    // 分屏自生组（id 无前缀，归属经首面板派生）
    expect(panelBelongsToGroup(
      panelIdInPage(PAGE, "editor-1"),
      fakeGroup("4", [panelIdInPage(PAGE, "terminal-0")]),
    )).toBe(true);
  });

  it("面板属其他页 → 不归属", () => {
    expect(panelBelongsToGroup(panelIdInPage("page-other-1", "t"), fakeGroup(pageGroupId(PAGE)))).toBe(false);
  });

  it("无页前缀面板落任何组 → 不归属", () => {
    expect(panelBelongsToGroup("terminal-page-1-0", fakeGroup(pageGroupId(PAGE)))).toBe(false);
  });

  it("无归属组（空壳/无协议面板）→ 恒不归属", () => {
    expect(panelBelongsToGroup(panelIdInPage(PAGE, "terminal-0"), fakeGroup("4"))).toBe(false);
  });
});

describe("panelsOfPage 页过滤", () => {
  it("按页前缀过滤 api.panels,其他页/裸 id 面板不混入", () => {
    const api = fakeApi([], [
      { id: panelIdInPage(PAGE, "terminal-0") },
      { id: panelIdInPage(PAGE, "editor-1") },
      { id: panelIdInPage("page-other-9", "terminal-0") },
      { id: "terminal-page-1-0" },
    ]);
    const result = panelsOfPage(api, PAGE);
    expect(result.map((p) => p.id)).toEqual([
      panelIdInPage(PAGE, "terminal-0"),
      panelIdInPage(PAGE, "editor-1"),
    ]);
  });

  it("空面板集合返回空数组", () => {
    const api = fakeApi([], []);
    expect(panelsOfPage(api, PAGE)).toEqual([]);
  });
});
