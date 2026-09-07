// overrides.test.ts — overrides.ts 四导出单元测试（TST-03，spec §7.2）
//
// 测试策略：
// - 真实 schemeRegistry 单例 + 真实 linear 方案；beforeEach 经 _reset() + register(linear)
//   还原隔离（照 tab-title-registry.test.ts 先例，_reset 仅测试用）
// - 模块加载序：side-effect import "../theme/schemes" 先行注册内置方案——
//   CP-039 后 editorTheme 改函数形 getEditorTheme()（每次调用取 active，不再依赖
//   加载时序），colors.ts 内部 import "./schemes" 的保护模式不变
// - editorColorOverrides 用真实 @codemirror/view：EditorState.create 消费扩展后经
//   EditorView.styleModule facet 取 StyleModule，getRules() 断言规则值来自 active 方案
//   （style-mod 编译后值原样保留，实测验证；选择器前缀每次调用随机生成，故只按值断言）
// - ACC-05 层叠守卫：与 oneDark 竞争的规则另断言选择器带 .cm-editor 前缀（特异性形态
//   .ͼx.cm-editor / .ͼx.cm-editor .cm-searchMatch）——mountStyles 反转扩展数组后
//   同特异性下 oneDark 恒赢，升特异性是修复方向；jsdom 的 getComputedStyle 不支持
//   <style> 规则层叠，computed 断言不可靠，故用规则文本断言

import { describe, it, expect, beforeEach } from "vitest";
import { EditorState, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

// side-effect：注册内置 linear（必须先于 overrides 求值）
import "../theme/schemes";
import { schemeRegistry } from "../theme/schemeRegistry";
import { linear } from "../theme/schemes/linear";
import type { ColorScheme } from "../theme/schemes/types";
import {
  dockviewVarStyle,
  allotmentVarStyle,
  getEditorTheme,
  editorColorOverrides,
  editorSyntaxHighlight,
} from "../theme/overrides";

/** 仓库根（ACC-05 消费点顺序断言读生产源码用；照 no-claude-literals.test.ts 先例） */
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

/** 提取 CM6 主题扩展编译后的 CSS 规则文本——EditorState.create 验证 + styleModule facet 读取 */
function themeRules(ext: Extension): string {
  const state = EditorState.create({ extensions: [ext] });
  return state
    .facet(EditorView.styleModule)
    .map((m) => m.getRules())
    .join("\n");
}

/** 从编译后规则文本中定位含指定值的规则，返回 { selector, props }（未命中 null） */
function ruleForValue(
  rules: string,
  value: string,
): { selector: string; props: string } | null {
  for (const line of rules.split("\n")) {
    if (line.includes(value)) {
      const brace = line.indexOf("{");
      return { selector: line.slice(0, brace).trim(), props: line.slice(brace + 1) };
    }
  }
  return null;
}

describe("overrides", () => {
  // 每用例独立：清空注册表 + active 复位 linear + 重注册内置方案
  beforeEach(() => {
    schemeRegistry._reset();
    schemeRegistry.register(linear);
  });

  describe("dockviewVarStyle", () => {
    it("键集合恰 20 条（全 --dv- 前缀）且值与 active 方案 libraries.dockview 一致", () => {
      const style = dockviewVarStyle();
      expect(Object.keys(style)).toHaveLength(20);
      for (const key of Object.keys(style)) {
        expect(key).toMatch(/^--dv-/);
      }
      // active = linear：与 getActive() 及内置方案值均一致
      expect(style).toEqual(schemeRegistry.getActive().libraries.dockview);
      expect(style).toEqual(linear.libraries.dockview);
    });
  });

  describe("allotmentVarStyle", () => {
    it("恰 2 键（--separator-border / --focus-border）且值来自 active 方案 allotment 段", () => {
      const style = allotmentVarStyle();
      expect(Object.keys(style)).toHaveLength(2);
      expect(style).toEqual({
        "--separator-border": linear.libraries.allotment.separatorBorder,
        "--focus-border": linear.libraries.allotment.focusBorder,
      });
      // 值非空守卫
      expect(linear.libraries.allotment.separatorBorder).not.toBe("");
      expect(linear.libraries.allotment.focusBorder).not.toBe("");
    });
  });

  describe("getEditorTheme", () => {
    it("每次调用返回当前 active 方案 editor 段 theme（linear = oneDark 透出）", () => {
      expect(getEditorTheme()).toBe(schemeRegistry.getActive().editor.theme);
      expect(getEditorTheme()).toBe(linear.editor.theme);
      expect(getEditorTheme()).toBeTruthy();
    });

    it("setActive 切换后再次调用跟随新方案（响应式取色，CP-039 函数形契约）", () => {
      // 临时方案改 editor.theme 为独立扩展引用——与 linear 的 oneDark 透出可区分
      const testTheme = EditorView.theme({ "&": { color: "#112233" } });
      const testScheme: ColorScheme = {
        ...linear,
        id: "get-theme-follow",
        label: "Get Theme Follow",
        editor: { ...linear.editor, theme: testTheme },
      };
      schemeRegistry.register(testScheme);
      expect(getEditorTheme()).toBe(linear.editor.theme);
      schemeRegistry.setActive("get-theme-follow");
      expect(getEditorTheme()).toBe(testTheme);
      expect(getEditorTheme()).toBe(schemeRegistry.getActive().editor.theme);
    });
  });

  describe("editorColorOverrides", () => {
    it("返回合法 CM6 扩展（EditorState.create 可消费）且 lint/searchMatch/background 键生效", () => {
      const ext = editorColorOverrides();
      // EditorState.create 验证：扩展可入 state，styleModule facet 含编译后规则
      const state = EditorState.create({ extensions: [ext] });
      const modules = state.facet(EditorView.styleModule);
      expect(modules).not.toHaveLength(0);
      const rules = modules.map((m) => m.getRules()).join("\n");
      // background 键
      expect(rules).toContain(linear.editor.overrides.background);
      // searchMatch 键（match 背景 + outline 描边）
      expect(rules).toContain(linear.editor.overrides.searchMatch.match);
      expect(rules).toContain(linear.editor.overrides.searchMatch.matchOutline);
      // lint 键：波浪线 SVG 内色值经 encodeURIComponent 编码（# → %23）
      expect(rules).toContain(encodeURIComponent(linear.editor.overrides.lint.error));
    });

    it("层叠胜出（ACC-05 修复守卫）：与 oneDark 竞争的规则选择器带 .cm-editor 前缀", () => {
      // mountStyles 将扩展数组 reverse() 后挂载：扩展 [editorTheme(oneDark),
      // editorColorOverrides()] 编译后 oneDark 规则在 <style> 标签内排在后，同特异性下
      // 后声明者胜 → oneDark 恒赢。故竞争规则必须升特异性——background 用 .ͼx.cm-editor
      // （0,2,0 > oneDark .ͼo 0,1,0），searchMatch 三键用 .ͼx.cm-editor .cm-searchMatch
      // （0,3,0 > oneDark .ͼo .cm-searchMatch 0,2,0），胜负与扩展数组顺序无关。
      const rules = themeRules(editorColorOverrides());
      const { overrides } = linear.editor;

      // background：选择器 = .ͼx.cm-editor
      const bgRule = ruleForValue(rules, overrides.background);
      expect(bgRule).not.toBeNull();
      expect(bgRule!.selector).toMatch(/^\.ͼ[0-9a-z]+\.cm-editor$/);

      // searchMatch match：选择器 = .ͼx.cm-editor .cm-searchMatch
      const matchRule = ruleForValue(rules, overrides.searchMatch.match);
      expect(matchRule).not.toBeNull();
      expect(matchRule!.selector).toMatch(/^\.ͼ[0-9a-z]+\.cm-editor \.cm-searchMatch$/);

      // selected：选择器 = .ͼx.cm-editor .cm-searchMatch.cm-searchMatch-selected
      const selectedRule = ruleForValue(rules, overrides.searchMatch.selected);
      expect(selectedRule).not.toBeNull();
      expect(selectedRule!.selector).toMatch(
        /^\.ͼ[0-9a-z]+\.cm-editor \.cm-searchMatch\.cm-searchMatch-selected$/,
      );

      // selectionMatch：选择器 = .ͼx.cm-editor .cm-selectionMatch
      // 注意：附录 A 契约下 match 与 selectionMatch 同为 rgba(214,178,94,0.25)，
      // ruleForValue 按值定位会误命中 match 规则——改按选择器定位（守卫形态不变）
      const selectionMatchLine = rules
        .split("\n")
        .find((line) => line.includes(".cm-selectionMatch"));
      expect(selectionMatchLine).toBeDefined();
      expect(selectionMatchLine!.slice(0, selectionMatchLine!.indexOf("{")).trim()).toMatch(
        /^\.ͼ[0-9a-z]+\.cm-editor \.cm-selectionMatch$/,
      );

      // 无前缀裸规则不存在（防回归：若又写回与 oneDark 平级的 .cm-searchMatch 选择器，
      // 层叠胜负回到 by-order，此断言失败）
      expect(rules).not.toMatch(/^\.ͼ[0-9a-z]+ \.cm-searchMatch\b/m);
    });

    it("两连调用规则值同源——均来自 active 方案 editor.overrides 全 12 值", () => {
      // 选择器前缀每次调用随机生成（StyleModule.newName），值原样保留——只按值断言
      const assertAllOverrideValues = (rules: string) => {
        const { overrides } = linear.editor;
        const plain = [
          overrides.background,
          overrides.searchMatch.match,
          overrides.searchMatch.matchOutline,
          overrides.searchMatch.selected,
          overrides.searchMatch.selectionMatch,
          overrides.lint.activeBackground,
          overrides.lint.tooltipBackground,
          overrides.lint.tooltipBorder,
        ];
        const encoded = [
          overrides.lint.error,
          overrides.lint.warning,
          overrides.lint.info,
          overrides.lint.hint,
        ].map(encodeURIComponent);
        for (const value of [...plain, ...encoded]) {
          expect(rules).toContain(value);
        }
      };
      assertAllOverrideValues(themeRules(editorColorOverrides()));
      assertAllOverrideValues(themeRules(editorColorOverrides()));
    });
  });

  describe("editorSyntaxHighlight", () => {
    it("导出存在且为函数（TH-07 新增导出）", () => {
      expect(typeof editorSyntaxHighlight).toBe("function");
    });

    it("返回合法 CM6 扩展（EditorState.create 可消费，不抛异常）", () => {
      const ext = editorSyntaxHighlight();
      expect(() => {
        EditorState.create({ extensions: [ext] });
      }).not.toThrow();
    });

    it("syntax 9 组 tag→color 映射全部来自 active 方案 overrides.syntax", () => {
      // syntaxHighlighting 将 HighlightStyle.module 挂入 EditorView.styleModule facet——
      // themeRules 提取编译后规则文本，逐组断言色值来自 active 方案 syntax 段
      const syntax = schemeRegistry.getActive().editor.overrides.syntax;
      const rules = themeRules(editorSyntaxHighlight());
      // 9 组键：property/string/number/keyword/function/type/operator/punctuation/comment
      expect(Object.keys(syntax)).toHaveLength(9);
      for (const [tag, color] of Object.entries(syntax)) {
        expect(rules, `syntax.${tag} → ${color}`).toContain(color);
      }
      // 每组 spec 独立编译为一条 .ͼx {color:...} 规则（type/operator 同值但规则各自存在）
      expect(rules.match(/\{color:/g) ?? []).toHaveLength(9);
    });

    it("ACC-05：主题扩展由 editorThemeSlot 槽单点装配（syntax 先于 theme 顺序槽内固化，CP-039）", () => {
      // 消费点（useCodeMirror.ts）扩展数组现以 themeSlotRef.current.extension 单槽接入——
      // syntax→theme→overrides 顺序与「syntax 先于 theme」契约由 editorThemeBundle()
      // 单点锁死。双重守卫：① 消费点不再直拼三项（防绕过槽裸拼破坏层叠）；
      // ② editorThemeSlot.ts 槽内顺序 syntax < theme < overrides（HighlightStyle 与
      // oneDark 同机制竞争只能靠数组顺序决胜，mountStyles reverse 后 syntax 排最后=恒胜）。
      const src = readFileSync(resolve(repoRoot, "src/panels/editor/useCodeMirror.ts"), "utf8");
      // lastIndexOf：save 对话框 filters 段（"extensions: [\"*\"]"）同名，取 EditorView
      // 扩展装配点（全文件唯一，且位于 handleSave 之后）
      const arrStart = src.lastIndexOf("extensions: [");
      expect(arrStart).toBeGreaterThanOrEqual(0);
      const extArr = src.slice(arrStart, src.indexOf("]", arrStart));
      expect(extArr.indexOf("themeSlotRef.current!.extension,")).toBeGreaterThanOrEqual(0);
      expect(extArr.indexOf("editorSyntaxHighlight()")).toBe(-1);

      const slotSrc = readFileSync(resolve(repoRoot, "src/theme/editorThemeSlot.ts"), "utf8");
      const bundleStart = slotSrc.indexOf("editorThemeBundle");
      expect(bundleStart).toBeGreaterThanOrEqual(0);
      // 切片须从函数体 return [ 起——函数签名返回类型 Extension[] 自身含 []，从函数名
      // 后首个 [ 切会命中返回类型（切出空串，三项 indexOf 全 -1，守卫失效）
      const retStart = slotSrc.indexOf("return [", bundleStart);
      expect(retStart).toBeGreaterThanOrEqual(0);
      const bundleArr = slotSrc.slice(
        slotSrc.indexOf("[", retStart),
        slotSrc.indexOf("]", retStart),
      );
      const syntaxIdx = bundleArr.indexOf("editorSyntaxHighlight()");
      const themeIdx = bundleArr.indexOf("getEditorTheme()");
      const overridesIdx = bundleArr.indexOf("editorColorOverrides()");
      expect(syntaxIdx).toBeGreaterThanOrEqual(0);
      expect(themeIdx).toBeGreaterThanOrEqual(0);
      expect(overridesIdx).toBeGreaterThanOrEqual(0);
      expect(syntaxIdx).toBeLessThan(themeIdx);
      expect(themeIdx).toBeLessThan(overridesIdx);
    });
  });

  describe("setActive 切换", () => {
    it("切换后函数形导出跟随 active 方案（含 getEditorTheme 响应式取色，CP-039）", () => {
      // 临时方案：linear 基础上改三处单色（dockview 1 条 / allotment 1 键 / editor
      // background）+ theme 独立引用（与 oneDark 透出可区分）
      const testTheme = EditorView.theme({ "&": { color: "#010101" } });
      const testScheme: ColorScheme = {
        ...linear,
        id: "overrides-test",
        label: "Overrides Test",
        libraries: {
          ...linear.libraries,
          dockview: {
            ...linear.libraries.dockview,
            "--dv-group-view-background-color": "#123456",
          },
          allotment: { ...linear.libraries.allotment, separatorBorder: "#654321" },
        },
        editor: {
          ...linear.editor,
          theme: testTheme,
          overrides: { ...linear.editor.overrides, background: "#010203" },
        },
      };
      schemeRegistry.register(testScheme);
      schemeRegistry.setActive("overrides-test");

      // 函数形导出每次调用取当前 active 方案（D2 热切换）
      expect(dockviewVarStyle()["--dv-group-view-background-color"]).toBe("#123456");
      expect(allotmentVarStyle()["--separator-border"]).toBe("#654321");
      expect(themeRules(editorColorOverrides())).toContain("#010203");
      // getEditorTheme 函数形跟随 active（原常量契约 = 不重绑定，CP-039 消除该系统性后果）
      expect(getEditorTheme()).toBe(testTheme);
      expect(getEditorTheme()).toBe(schemeRegistry.getActive().editor.theme);

      // _reset 还原：清空注册表 + active 复位 linear + 重注册
      schemeRegistry._reset();
      schemeRegistry.register(linear);
      expect(schemeRegistry.getActive().id).toBe("linear");
      expect(dockviewVarStyle()).toEqual(linear.libraries.dockview);
      expect(getEditorTheme()).toBe(linear.editor.theme);
    });
  });
});
