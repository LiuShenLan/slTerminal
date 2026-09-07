# 章一「依赖与技术选型」修复清单（CP-001/002/003/032/033/038）

真值源:`docs/compromises.md` 章一(第 17-30 行)。本清单每条均经现状代码原文实读核对(2026-09-06),漂移点见末尾「起草附注」。

---

## CP-001 · 双 TS 并存——触发条件登记硬化为可机检形式 [Stage S01]

1. **位置**:
   - `package.json:71`——`"@typescript/native": "npm:typescript@^7.0.2"`(TS7 别名,tsc bin 真身)
   - `package.json:87`——`"typescript": "npm:@typescript/typescript6@^6.0.2"`(TS6 包装器,供 typescript-eslint 消费)
   - `.claude/adr.md:238`——ADR-0010「TE-07 执行结果」段,触发条件现为散文形式(「issue #10940 闭环 + TS7.1 稳定发布」)
   - `docs/compromises.md:19-20`——CP-001 登记条目
2. **现状**:
   - ADR-0010:238 末句:「**升级触发条件(同时满足)**:① typescript-eslint issue #10940 闭环(发布支持 TS7 版本)② TS7.1 稳定发布;触发后删 TS6 包装器与 `@typescript/native` 别名,`"typescript"` 直改 `^7.1.0`。」——条件无可执行判定,只能靠人读,休眠风险即登记本意所要防的。
   - 2026-09-06 核查:issue #10940 仍 open、`typescript` dist-tags.latest = 7.0.x(7.1 未发布)——双条件均未达成,本条**不动依赖**,只硬化登记。
3. **修复步骤**(全部照抄,不设计分支):
   1. 新建 `scripts/check-ts7-trigger.mjs`,内容如下(零依赖,仅用 node 内置 https):
      ```js
      // scripts/check-ts7-trigger.mjs — CP-001 双 TS 并存解除触发条件的机检脚本
      //
      // 双条件同时满足(ADR-0010 TE-07)时退出码 0 并打印解除清单:
      //   ① typescript-eslint issue #10940 闭环(GitHub state === "closed")
      //   ② typescript 7.1 稳定版发布(npm dist-tags.latest = 7.1.x,无预发布后缀)
      // 未达成退出码 1;网络/解析失败退出码 2(未知态,不误导判定)。
      // 用法:node scripts/check-ts7-trigger.mjs

      import https from "node:https";
      import { pathToFileURL } from "node:url";

      const ISSUE_URL = "https://api.github.com/repos/typescript-eslint/typescript-eslint/issues/10940";
      const REGISTRY_URL = "https://registry.npmjs.org/typescript";

      /** 纯判定:触发是否达成(两个入参均为已取回的事实,供测试直接驱动) */
      export function evaluateTrigger(issueState, latestVersion) {
        const issueClosed = issueState === "closed";
        const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(latestVersion ?? "");
        const ts71Stable = m !== null && Number(m[1]) === 7 && Number(m[2]) >= 1;
        return { issueClosed, ts71Stable, triggered: issueClosed && ts71Stable };
      }

      /** 极简 GET JSON(无外部依赖,避免为本工具引入 fetch polyfill 争论) */
      function getJson(url) {
        return new Promise((resolve, reject) => {
          https
            .get(url, { headers: { "user-agent": "slterminal-trigger-check" } }, (res) => {
              let body = "";
              res.on("data", (c) => (body += c));
              res.on("end", () => {
                try {
                  resolve(JSON.parse(body));
                } catch (e) {
                  reject(e);
                }
              });
            })
            .on("error", reject);
        });
      }

      async function main() {
        let issue;
        let registry;
        try {
          [issue, registry] = await Promise.all([getJson(ISSUE_URL), getJson(REGISTRY_URL)]);
        } catch (e) {
          console.error(`[ts7-trigger] 查询失败(网络/解析):${(e as Error).message}`);
          process.exit(2);
        }
        const latest = registry?.["dist-tags"]?.latest;
        const r = evaluateTrigger(issue?.state, latest);
        console.log(`[ts7-trigger] issue #10940 state=${issue?.state};typescript latest=${latest}`);
        if (r.triggered) {
          console.log(
            "[ts7-trigger] 双条件达成——执行 CP-001 解除:删 TS6 包装器与 @typescript/native 别名,typescript 直改 ^7.1.0",
          );
          process.exit(0);
        }
        console.log(`[ts7-trigger] 未达成(issue 闭环=${r.issueClosed},TS7.1 稳定=${r.ts71Stable})`);
        process.exit(1);
      }

      // 仅直跑时执行 main();被测试 import 时不触发网络
      if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
        main();
      }
      ```
      注:文件为 `.mjs`,`(e as Error)` 需改 JSDoc 兼容写法——照抄版用 `/** @type {Error} */` 或直接用 `e.message`(mjs 无 TS 语法,直接写 `e.message` 即可,上面行内 `(e as Error)` 仅为本清单标注,抄写时写 `${e.message}`)。
   2. `.claude/adr.md` TE-07 段(adr.md:238)末句「**升级触发条件(同时满足)**…」整句替换为:
      > **升级触发条件(机检,`scripts/check-ts7-trigger.mjs`,CP-001 登记硬化)**:`node scripts/check-ts7-trigger.mjs` 退出码 0 即双条件达成——① issue #10940 `state=closed` ② `typescript` dist-tags.latest = 7.1.x 稳定版;退出码 1 = 未达成,退出码 2 = 查询失败(未知态)。触发后删 TS6 包装器与 `@typescript/native` 别名,`"typescript"` 直改 `^7.1.0`。
   3. `docs/compromises.md` CP-001 条目「**修改方向**」句替换为:
      > **修改方向**:维持双轨不动依赖;触发条件已硬化为机检脚本 `scripts/check-ts7-trigger.mjs`(退出码 0 = 双条件达成,退出码 1 = 未达成,退出码 2 = 查询失败),登记同步 ADR-0010 TE-07。脚本退出码 0 时执行解除:删 TS6 包装器与别名、`"typescript"` 直改 `^7.1.0`。
4. **测试同步**:
   - 新建 `src/__tests__/deps-ts7-trigger.test.ts`(命名照「领域对象-能力行为」):import `evaluateTrigger` from `../../scripts/check-ts7-trigger.mjs`,用例名建议:
     - `evaluateTrigger_issue已闭环且TS71稳定_达成`(入参 `"closed","7.1.0"` → `triggered=true`)
     - `evaluateTrigger_issue未闭环_不达成`(`"open","7.1.0"` → `false,ts71Stable=true`)
     - `evaluateTrigger_TS71未发布_不达成`(`"closed","7.0.2"` → `false`)
     - `evaluateTrigger_预发布后缀_不视为稳定`(`"closed","7.1.0-beta.1"` → `ts71Stable=false`;正则 `^(\d+)\.(\d+)\.(\d+)$` 拒预发布)
     - `evaluateTrigger_查询失败入参null_不达成`(`undefined,null` → 两子条件均 false)
   - 既有用例无需适配(无 package.json 依赖变更)。
5. **文档同步**:ADR-0010 TE-07 段与 compromises.md CP-001 按步骤 2/3 改写;根 CLAUDE.md 静态门禁节不动。完成后勾选销项 compromises.md CP-001 复选框(由 Stage 收尾统一执行)。
6. **验证**:
   - `node scripts/check-ts7-trigger.mjs; echo $?` → 退出码 1,输出含 `未达成`(当前 #10940 open、latest 7.0.x;若实跑时上游已变,以退出码语义断言为准,不得为此改脚本)
   - `npx vitest run src/__tests__/deps-ts7-trigger.test.ts` 全绿
   - `grep -n "check-ts7-trigger" .claude/adr.md docs/compromises.md` 均命中
   - `npx knip --production` 退出码 0(scripts/ 下既有 gen-katex-inline.mjs 不在 entry 也不报,同形态新脚本预期一致;若报 unused file,在 knip.json `ignore` 数组追加 `"scripts/**"` 后重跑)

---

## CP-002 · 摘除 codemirror-json-schema,自绘 lint/hover 层直消费 json-schema-library 11.x(双 major 消亡) [Stage S08,与 CP-039 同 agent]

1. **位置**:
   - `src/features/cliProfiles/profiles/claude/configEditor/JsonMode.tsx:19-25`(import 块)、`:156-163`(extensions 数组)
   - `package.json:47`(`"codemirror-json-schema": "0.8.1"`)、`package.json:50`(`"json-schema": "0.4.0"`——仅 JsonMode.tsx:25 类型 import 消费,摘除后须一并删,否则 knip 报 unused dependency)
   - `src/features/cliProfiles/profiles/claude/configEditor/schema/index.ts:22-24`(TE-15 去重评估结论注释,登记失实风险点)
   - `vitest.config.ts:9-16`、`vitest.l3.config.ts:8-15`(server.deps.inline 登记,专为 codemirror-json-schema 无扩展名 ESM 导入而设)
   - `src/__tests__/hooks-config-jsonmode.test.tsx:15-47, 97-103, 160-189`(codemirror-json-schema mock 与扩展注册断言)
   - lockfile 双实例:`package-lock.json:5905-5907`(嵌套 9.3.5)与 `:9027-9029`(顶层 11.6.2)
2. **现状**(JsonMode.tsx 关键行原文):
   ```tsx
   import {
     jsonSchemaHover,
     jsonSchemaLinter,
     stateExtensions,
     handleRefresh,
   } from "codemirror-json-schema";
   import type { JSONSchema7 } from "json-schema";
   ```
   ```tsx
           linter(jsonParseLinter(), { delay: 300 }),
           linter(jsonSchemaLinter(), { needsRefresh: handleRefresh }),
           hoverTooltip(jsonSchemaHover()),
           stateExtensions(hooksSubSchema as unknown as JSONSchema7),
   ```
   schema/index.ts:45 已有 11.x 编译单例 `compileSchema(hooksSubSchema, { draft: "draft-07" })`,`validateHooksJson(text)` 返回 `{ isValid, diagnostics: { message, pointer } }`——自绘层直接复用,不重复编译。
3. **修复步骤**:
   1. 新建 `src/features/cliProfiles/profiles/claude/configEditor/jsonSchemaCm.ts`,内容照抄:
      ```ts
      // jsonSchemaCm.ts — 自绘 hooks JSON schema lint/hover 层(CP-002,替代 codemirror-json-schema)
      //
      // 直接消费 json-schema-library 11.x(compileSchema 单例见 schema/index.ts),消除
      // codemirror-json-schema 锁 9.x 造成的双 major 并存(TE-15 收编)。
      // 语义对齐原 codemirror-json-schema 用法:lint = schema 违规波浪线(delay 300ms,
      // 与语法 linter 对齐);hover = 键位置悬停显示子 schema description。
      // pointer 定位策略:JSON Pointer 逐段顺序文本搜索(数组索引段按第 n 个 '[' 近似),
      // 定位失败退回整文档下划——hooks 子树为小文档,近似定位足够收敛下划线。

      import { linter, type Diagnostic } from "@codemirror/lint";
      import { hoverTooltip, type Extension } from "@codemirror/view";
      import {
        hooksSubSchema,
        validateHooksJson,
        type JsonDiagnostic,
      } from "./schema";

      type SchemaNode = {
        description?: string;
        properties?: Record<string, SchemaNode>;
        items?: SchemaNode;
        $ref?: string;
      } & Record<string, unknown>;

      /** JSON Pointer 单段解码(RFC 6901:~0→~、~1→/) */
      function decodeSegment(seg: string): string {
        return seg.replace(/~1/g, "/").replace(/~0/g, "~");
      }

      /**
       * JSON Pointer → doc 字符区间(顺序文本搜索;失败返回 null)。
       * 数字段按「cursor 起第 n 个 '[' 之后」近似——hooks 数组元素均为对象,
       * 下划线收敛到目标行即达标,不做精确 AST 定位。
       */
      export function pointerToRange(
        doc: string,
        pointer: string,
      ): { from: number; to: number } | null {
        if (!pointer) return null;
        const segments = pointer.split("/").slice(1).map(decodeSegment);
        let cursor = 0;
        for (const seg of segments) {
          if (/^\d+$/.test(seg)) {
            let rest = Number(seg);
            let pos = cursor;
            while (rest-- >= 0) {
              pos = doc.indexOf("[", pos);
              if (pos < 0) return null;
              pos += 1;
            }
            cursor = pos;
          } else {
            const pos = doc.indexOf(`"${seg}"`, cursor);
            if (pos < 0) return null;
            cursor = pos + seg.length + 2; // 越过键引号对
          }
        }
        // 叶子下划区间:键后至最近的行尾/逗号/闭合符
        const stops = ["\n", ",", "}", "]"]
          .map((c) => doc.indexOf(c, cursor))
          .filter((p) => p >= 0);
        const to = stops.length ? Math.min(...stops) : doc.length;
        return { from: cursor, to: Math.max(to, cursor) };
      }

      /** schema 诊断 → CM6 Diagnostic(pointer 定位失败退回整文档) */
      function toCmDiagnostic(doc: string, d: JsonDiagnostic): Diagnostic {
        const range = d.pointer ? pointerToRange(doc, d.pointer) : null;
        return {
          from: range?.from ?? 0,
          to: range?.to ?? doc.length,
          severity: "error",
          message: d.message,
        };
      }

      /** lint 真值源(纯函数,供 hooksSchemaLinter 包装与测试直驱) */
      export function lintHooksSchemaDoc(doc: string): Diagnostic[] {
        const { isValid, diagnostics } = validateHooksJson(doc);
        return isValid ? [] : diagnostics.map((d) => toCmDiagnostic(doc, d));
      }

      /** hooks 子 schema 校验波浪线(delay 300ms,与原 jsonSchemaLinter 外层包装一致) */
      export function hooksSchemaLinter(): Extension {
        return linter((view) => lintHooksSchemaDoc(view.state.doc.toString()), {
          delay: 300,
        });
      }

      /** 本地 $ref 解析(仅 hooksSubSchema 内部 `#/$defs/*` 形态,8 层防环) */
      function resolveRef(node: SchemaNode): SchemaNode {
        let cur = node;
        let guard = 0;
        while (typeof cur.$ref === "string" && cur.$ref.startsWith("#/$defs/") && guard++ < 8) {
          const name = cur.$ref.slice("#/$defs/".length);
          const defs = (hooksSubSchema as unknown as { $defs: Record<string, SchemaNode> }).$defs;
          if (!defs[name]) return cur;
          cur = defs[name];
        }
        return cur;
      }

      /** 键路径 → 子 schema(逐段下钻 properties/数组 items + $ref 解析;失败 null) */
      export function resolveSchemaPath(path: string[]): SchemaNode | null {
        let node = hooksSubSchema as unknown as SchemaNode;
        for (const seg of path) {
          node = resolveRef(node);
          if (node.properties?.[seg]) {
            node = node.properties[seg];
            continue;
          }
          if (/^\d+$/.test(seg) && node.items && typeof node.items === "object") {
            node = node.items;
            continue;
          }
          return null;
        }
        return resolveRef(node);
      }

      /**
       * pos 之前最近的键路径(向前回溯 `"key":` 形态,最多 6 层,窗口 4096 字符)。
       * 数组索引段不入路径(由 resolveSchemaPath 的 items 分支按数字段兜);
       * 语义 = 悬停点所在对象链上的键序列。
       */
      export function pathAt(doc: string, pos: number): string[] {
        const path: string[] = [];
        let end = pos;
        for (let depth = 0; depth < 6; depth++) {
          const start = Math.max(0, end - 4096);
          const window = doc.slice(start, end);
          const re = /"((?:[^"\\]|\\.)*)"\s*:/g;
          let last: RegExpExecArray | null = null;
          let m: RegExpExecArray | null;
          while ((m = re.exec(window))) last = m;
          if (!last) break;
          path.unshift(last[1]);
          end = start + last.index;
        }
        return path;
      }

      /** 悬停文案真值源(纯函数,供测试直驱;无 description 返回 null 不弹层) */
      export function resolveHoverDescription(doc: string, pos: number): string | null {
        const node = resolveSchemaPath(pathAt(doc, pos));
        return node?.description ?? null;
      }

      /** hooks 子 schema 悬停浮层(键 description;布局内联样式,色值交 CM tooltip 主题) */
      export function hooksSchemaHover(): Extension {
        return hoverTooltip(
          (view, pos) => {
            const description = resolveHoverDescription(view.state.doc.toString(), pos);
            if (!description) return null;
            return {
              pos,
              end: pos,
              above: true,
              create: () => {
                const dom = document.createElement("div");
                dom.textContent = description;
                dom.style.maxWidth = "360px";
                dom.style.padding = "2px 6px";
                return { dom };
              },
            };
          },
          { hoverTime: 300 },
        );
      }
      ```
   2. `JsonMode.tsx` 三处编辑(照抄):
      - import 块(:19-25 整块)替换为:
        ```tsx
        import { hooksSchemaHover, hooksSchemaLinter } from "./jsonSchemaCm";
        ```
        同时 :26-30 的 `import { hooksSubSchema, validateHooksJson, type JsonDiagnostic } from "./schema";` 改为(摘除后 hooksSubSchema 在组件内不再直接消费):
        ```tsx
        import { validateHooksJson, type JsonDiagnostic } from "./schema";
        ```
        :14 `import { EditorView, hoverTooltip } from "@codemirror/view";` 改为(组件不再直接调 hoverTooltip):
        ```tsx
        import { EditorView } from "@codemirror/view";
        ```
      - extensions 数组(:159-162 四行)替换为:
        ```tsx
                linter(jsonParseLinter(), { delay: 300 }),
                hooksSchemaLinter(),
                hooksSchemaHover(),
        ```
        (`stateExtensions(...)` 行删除——其唯一职责是把 schema 注入 codemirror-json-schema 的内部状态,自绘层经 validateHooksJson 闭包直取。)
      - 文件头注释 :5-8 的「CM6 扩展:…codemirror-json-schema(jsonSchemaHover 悬停 + jsonSchemaLinter 波浪线,…)」改为:
        > // CM6 扩展:@codemirror/lang-json 语言 + 自绘 schema 层(jsonSchemaCm.ts——
        > // hooksSchemaLinter 波浪线 + hooksSchemaHover 悬停,直接消费 json-schema-library
        > // 11.x,CP-002 摘除 codemirror-json-schema)+ jsonParseLinter 语法波浪线。
   3. `package.json` 删两行:`:47 "codemirror-json-schema": "0.8.1",`、`:50 "json-schema": "0.4.0",`;然后 `npm install` 刷新 lockfile(嵌套 json-schema-library 9.3.5 随之消失)。
   4. `vitest.config.ts` :9-16 的 `server: { deps: { inline: ['codemirror-json-schema'] } }` 块整删(该配置随之只剩 environment/include/exclude/setupFiles);`vitest.l3.config.ts` :8-15 同删。
   5. `schema/index.ts:22-24` 注释(「去重评估结论(S19 登记):…保留双库…」)替换为:
      > // CP-002 已摘除 codemirror-json-schema:自绘 lint/hover 层(jsonSchemaCm.ts)直接消费
      > // 本模块 11.x 编译单例,json-schema-library 全仓单实例(TE-15 消解)。
4. **测试同步**:
   - 新建 `src/__tests__/hooks-json-schema-cm.test.ts`——直测 jsonSchemaCm.ts 纯函数(无 jsdom 依赖,照 hooks-config-schema.test.ts 模式),用例名建议:
     - `pointerToRange_顶层键_定位到值区间`(`{"PreToolUse": []}`,pointer `/PreToolUse` → from 在键后、to 至 `]` 前)
     - `pointerToRange_嵌套数组索引_收敛到目标行`(pointer `/PreToolUse/0/hooks/0/type`,doc 含多事件,断言 from/to 均落在 `type` 键所在行区间)
     - `pointerToRange_未知键_返回null`;`pointerToRange_空pointer_返回null`
     - `pathAt_嵌套位置_返回键序列`;`pathAt_文档头_返回空数组`
     - `resolveSchemaPath_事件键_命中hooks子schema`;`resolveSchemaPath_hookCommand$ref_解析出description`;`resolveSchemaPath_未知键_返回null`
     - `lintHooksSchemaDoc_合法配置_零诊断`;`lintHooksSchemaDoc_未知事件_单条error级诊断且message含Additional property`;`lintHooksSchemaDoc_语法错误_退回整文档区间(from=0,to=doc.length)`
     - `resolveHoverDescription_事件键_返回description`;`resolveHoverDescription_无description键_返回null`
   - 适配 `src/__tests__/hooks-config-jsonmode.test.tsx`:
     - 删除 hoisted 块中 `mockJsonSchemaHover/mockJsonSchemaLinter/mockStateExtensions/mockHandleRefresh` 四项及 :97-103 的 `vi.mock("codemirror-json-schema", ...)` 块;
     - 新增 `vi.mock("../features/cliProfiles/profiles/claude/configEditor/jsonSchemaCm", () => ({ hooksSchemaLinter: mockHooksSchemaLinter, hooksSchemaHover: mockHooksSchemaHover }))`,hoisted 两 mock 各返回 sentinel(`[{ __schemaLinter: true }]` / `[{ __schemaHover: true }]`);
     - 用例「schema 扩展注册:jsonSchemaHover / jsonSchemaLinter + hooks 子 schema + height theme」(:160-189)重写为:断言 captured extensions 含两个 sentinel、`mockLinter` 仅被调一次(jsonParseLinter,配置 `{ delay: 300 }`)、`mockHoverTooltip` 不再被组件调用(悬停包装已下沉 jsonSchemaCm)——原「包装顺序锁定 HKC-01」断言(:177-180 两 linter 身份断言)改由 hooks-json-schema-cm.test.ts 的 delay 断言承接;
     - beforeEach 的对应 mockClear 同步增删。
   - 既有 `hooks-config-schema.test.ts` 零改动(validateHooksJson 语义不动,防回归面本身)。
5. **文档同步**:
   - `src/features/cliProfiles/CLAUDE.md` TE-15 债务段(「**TE-15 债务登记(ADR-0010)**:json-schema-library 9.x/11.x 双 major 并存…待上游升级消解。」)整段替换为:
     > **TE-15 已消解(CP-002)**:codemirror-json-schema 已摘除,自绘 lint/hover 层(`configEditor/jsonSchemaCm.ts`)直接消费本模块 11.x 编译单例;json-schema-library 全仓单实例,katex 式「待上游」债务形态不再保留。
   - `.claude/adr.md` TE-15 段(:240)末尾追加一句:「**消解记录(CP-002,2026-09-06 后)**:codemirror-json-schema 已摘除,自绘层直消费 11.x,双 major 并存消亡。」
   - `vitest.config.ts` / `vitest.l3.config.ts` 删除处的注释同步消失(步骤 4 已含)。
6. **验证**:
   - `grep -rn "codemirror-json-schema" src/ package.json vitest.config.ts vitest.l3.config.ts` → 零命中(仅 docs/compromises.md、.claude/adr.md 历史登记可命中)
   - `grep -rn 'from "json-schema"' src/` → 零命中
   - `npm ls json-schema-library` → 输出仅 `json-schema-library@11.6.2` 单实例(退出码 0)
   - `npx tsc --noEmit` / `npx eslint src/` / `npx knip --production` 退出码全 0(knip 尤其会拦 json-schema 漏删)
   - `npm test` 与 `npm run test:l3` 全绿(含新增 hooks-json-schema-cm.test.ts)

---

## CP-003 · 删便携 Node 22 自动下载,Node 26 直跑 + engines 纳入主 toolchain [Stage S03,与 CP-045/046 合并单 agent]

1. **位置**:
   - `e2e-tests/run-wdio.cjs:246-288`(自动下载分支,含 :286-288 `else { fallback(); }`)、`:1-4`(文件头注释)、`:27`(`const https = require('https');`)
   - `package.json:68`(`"@types/node": "^26.2.0"`——随启动器下线自动对齐,无动作)、无 `engines` 字段
   - `e2e-tests/CLAUDE.md:23-25`(「Node 版本兼容启动器」节)
   - `e2e-tests/wdio.conf.ts:5`(文件头注释「本地用 Node 22 便携版自动切换(Node 26 undici 8 与 webdriverio 不兼容),CI 固定 Node 22。」——决策只点名 CLAUDE.md,此处置漂移,见附注 6)
2. **现状**(run-wdio.cjs:246-288 原文摘录):
   ```js
   if (major >= 26) {
     const nodeDir = path.resolve(__dirname, '..', '.temp', 'node22');
     const node22 = path.join(nodeDir, 'node.exe');
     // E2E-13①:便携 Node 22 预置 .temp/node22 或 CI 固定 Node 22 时跳过外网下载。
     if (fs.existsSync(node22)) { ... runWdio(node22); process.exit(0); ... }
     // 自动下载便携 Node 22
     console.log('[wdio-launcher] 下载便携 Node 22 (约 30MB)...');
     ...
     https.get(url, (res) => { ... 下载 v22.21.1 ... });
   } else {
     fallback();
   }
   ```
   webdriverio#15265(Node 26 undici 8 不兼容)已于 9.30.0 修复,项目解析 9.30.1(package.json:99 overrides / lockfile 实证)——自动下载分支成为「无人触发的死兜底」。
3. **修复步骤**:
   1. **门禁实证(先行,失败即走分支 B)**:确保 `.temp/node22` 不存在(存在则临时改名 `node22.bak`,验证后删除),确认 `node --version` 主版本 ≥ 26;完成步骤 2-6 后 `npm run e2e` 全量跑通。全绿 → 继续;出现 tauri-service/undici 类 Node 26 证据 → **回滚本步骤全部改动**,把新证据登记进 `e2e-tests/CLAUDE.md`「Node 版本兼容启动器」节(改成「Node 26 直跑因 <新证据> 仍不可行,自动下载分支保留」)与 compromises.md CP-003 条目,本条转休眠。
   2. `run-wdio.cjs:246-288` 整块替换为(显式预置约定保留,自动下载删除):
      ```js
      if (major >= 26) {
        const nodeDir = path.resolve(__dirname, '..', '.temp', 'node22');
        const node22 = path.join(nodeDir, 'node.exe');

        // 显式预置约定(E2E-13①):.temp/node22 存在且 > 1MB 时强制切便携 Node 22
        // (判活只看大小,防中断残留的损坏文件被误用);不自动下载——
        // webdriverio 9.30.0 已修复 Node 26 undici 8 兼容(webdriverio#15265,CP-003)。
        if (fs.existsSync(node22)) {
          let size = 0;
          try { size = fs.statSync(node22).size; } catch { size = 0; }
          if (size > 1024 * 1024) {
            console.log(`[wdio-launcher] Node ${process.version} → 使用便携 Node 22`);
            runWdio(node22);
            process.exit(0);
          }
          console.warn('[wdio-launcher] 便携 Node 22 文件不完整(<1MB),改用当前 Node');
        }
      }
      fallback();
      ```
   3. `run-wdio.cjs:27` 删 `const https = require('https');`(全文件仅 :269/:271 下载分支两处消费,步骤 2 后零引用)。
   4. `run-wdio.cjs:1-4` 文件头注释替换为:
      > ```
      > /**
      >  * WDIO 启动器(CP-003):webdriverio 9.30.0 已修复 Node 26 undici 8 兼容
      >  * (webdriverio#15265),Node >= 22 直跑;.temp/node22 显式预置便携 Node 22 时优先。
      >  * CI 固定 Node 22(见 ci.yml)。
      >  *
      >  * 数据隔离(BE-01/TE-02):…(以下原文保留不动)
      > ```
   5. `package.json` 在 `"scripts"` 块之后、`"dependencies"` 之前插入:
      ```json
        "engines": {
          "node": ">=22"
        },
      ```
   6. `e2e-tests/CLAUDE.md:23-25` 节(「### Node 版本兼容启动器」及其正文一行)整体替换为:
      > ```
      > ### Node 版本(CP-003)
      >
      > `npm run wdio` 由 `run-wdio.cjs` 启动,Node >= 22 直跑(webdriverio 9.30.0+ 已修复 Node 26 undici 8 兼容,webdriverio#15265);`.temp/node22` 显式预置便携 Node 22 时优先使用(不自动下载,预置方法自行从 nodejs.org 取 node.exe 放入)。版本约束经根 package.json `engines.node ">=22"` 纳入主 toolchain。
      > ```
   7. `e2e-tests/wdio.conf.ts:5` 注释替换为:
      > `// Node >= 22 直跑(run-wdio.cjs,webdriverio 9.30.0+ 修复 Node 26 兼容),CI 固定 Node 22。`
   8. 若步骤 1 验证时曾改名 `.temp/node22`,验证通过后删除 `.temp/node22.bak` 与 `.temp/node22`(存在时)。
4. **测试同步**:
   - run-wdio.cjs 为进程编排壳(分支语义 = spawn 外部进程),无可单元化点;**全量 e2e 在 Node 26 下跑通即本条的防复发验证**,按纪律 11 在 `.claude/test-exemptions.md` 登记豁免:「run-wdio.cjs 启动器分支——进程编排无单测锚点,由 L4 全量 e2e(Node 26 直跑)兜底,豁免原因:spawn 外部进程行为不可 jsdom 化」。
   - 既有用例无需适配(wdio.conf.ts specs/能力零改动)。
5. **文档同步**:
   - `e2e-tests/CLAUDE.md` :23-25 按步骤 6 改写;`:42`(glyph-repro 节内「Node 26→便携 22 切换兜底,裸 `npx wdio` 会踩版本坑」)顺手改为「Node >= 22 直跑,裸 `npx wdio` 缺启动器的数据/假屋隔离链(SLTERM_DATA_DIR/USERPROFILE),禁止绕过启动器」——理由从版本坑换为隔离链,事实更准确。
   - `wdio.conf.ts:5` 按步骤 7;`run-wdio.cjs:1-4` 按步骤 4。
   - `.claude/test-exemptions.md` 按步骤 4 末尾追加豁免行。
   - compromises.md CP-003 销项勾选(Stage 收尾统一)。
6. **验证**:
   - `node --version` → v26.x;`.temp/node22` 不存在时 `npm run e2e` 全绿(含 terminal.e2e.ts 末位杀 app 用例),输出无 `下载便携 Node 22` 字样
   - `grep -n "nodejs.org\|https\.get\|require('https')" e2e-tests/run-wdio.cjs` → 零命中
   - `npm pkg get engines` → `{"node":">=22"}`
   - `npm ls @types/node` → `@26.x`(类型锚定与运行时对齐)
   - `npx tsc --noEmit` / `npx eslint src/` 退出码 0(engines 不影响,确认无次生破坏)

---

## CP-032 · wdio overrides 成因登记 + 「谁钉谁」对齐契约 [Stage S01]

1. **位置**:
   - `package.json:92-100`(overrides 段 7 条目:serialize-javascript / deepmerge-ts / @puppeteer/browsers / glob / @wdio/globals / expect-webdriverio / webdriverio)
   - `package-lock.json:4796-4815`(@wdio/tauri-service@1.3.0 硬钉:`@wdio/globals 9.29.1`、`@wdio/logger 9.29.1`、`@wdio/spec-reporter 9.29.1`、`@wdio/types 9.29.1`、`webdriverio 9.30.0`)
   - `.claude/adr.md:153`(ADR-0006「overrides 段保持现状(^),不随本策略调整」——仅状态,无成因)
   - 成因一手来源(本次实挖):`a027b17`(2026-08-18,TE-01/02/05/06/12 批)首次加入前 4 项;`1233336`(2026-08-22,TE-06/07/14「WDIO dedupe」)加入后 3 项
2. **现状**:`npm ls` 实证——serialize-javascript ← mocha 10.8.2(@wdio/mocha-framework 传递);deepmerge-ts ← @wdio/config/@wdio/utils/webdriver;@puppeteer/browsers ← @wdio/utils;glob ← @wdio/config/archiver-utils/mocha。家族解析:@wdio/globals 9.31.1 / webdriverio 9.30.1 顶层单实例(dedupe 生效),tauri-service 子树硬钉经 overrides 强扭对齐。
3. **修复步骤**:
   1. `e2e-tests/CLAUDE.md` 在「### Node 版本兼容启动器」节之前(CP-003 后该节改名,插入位置锚定为「### E2E helper 命名与挂载位置」节(:27)之前)插入新节,内容照抄:
      ```markdown
      ### wdio 版本矩阵与 overrides 对齐契约（CP-032）

      真值源：主声明在根 package.json devDependencies（@wdio/* `^9.30.1`、@wdio/globals `^9.31.0`、expect-webdriverio `^6.0.5`）；`@wdio/tauri-service@1.3.0` **硬钉** @wdio/* `9.29.1` + webdriverio `9.30.0`（package-lock 实测，上游约束）；overrides 段把家族强扭到主声明线——e2e 版本真值源 = 主声明 + overrides 两处，缺一即漂移。

      各 override 成因（2026-09-06 挖掘补登记，来源 git log -S）：

      | override | 成因 | 引入提交 |
      |---|---|---|
      | `serialize-javascript` `^7.0.5` | 消 mocha 传递依赖 RCE（npm audit high 阻断），经 @wdio/mocha-framework 传递 | a027b17（TE-01 批） |
      | `deepmerge-ts` `^8.0.1` | @wdio/config/@wdio/utils/webdriver 传递依赖版本统一（dedupe） | a027b17 |
      | `@puppeteer/browsers` `^3.2.1` | @wdio/utils 传递依赖版本统一（dedupe） | a027b17 |
      | `glob` `^10.5.0` | @wdio/config/mocha/archiver-utils 传递依赖版本统一（dedupe） | a027b17 |
      | `@wdio/globals` `^9.31.0` | 对齐 tauri-service 硬钉 9.29.1 → 主声明 | 1233336（TE-06/07/14） |
      | `expect-webdriverio` `^6.0.5` | dedupe 对齐主声明 | 1233336 |
      | `webdriverio` `^9.30.1` | 对齐 tauri-service 硬钉 9.30.0 → 主声明 | 1233336 |

      **对齐契约**：
      1. 升 `@wdio/tauri-service` 或任一 `@wdio/*` 主声明时，先 `npm view @wdio/tauri-service@latest dependencies` 查新版硬钉；硬钉与主声明不一致 → 更新 overrides 对应条目保持家族单实例；一致 → 删对应 override 条目（去 overrides 化）。
      2. 版本评审看两处：主声明 `^` 浮动结果 + overrides 是否仍与主声明同线；`npm ls webdriverio @wdio/globals` 输出单实例即健康态。
      3. 前 4 项（serialize-javascript/deepmerge-ts/@puppeteer/browsers/glob）为传递依赖治理，与 tauri-service 无关——wdio 升 major 时逐条重估是否仍需。
      ```
   2. **评估动作(去 overrides 化可行性,当场执行)**:`npm view @wdio/tauri-service@latest version dependencies --json`:
      - 若 latest > 1.3.0 且硬钉已放开(peer 化或区间化)→ 升 tauri-service 至 latest,删 overrides 中 `@wdio/globals`/`webdriverio` 两条,`npm install` 后 `npm ls webdriverio @wdio/globals` 断言单实例,跑 `npm run e2e` 全量;
      - 若 latest = 1.3.0(硬钉仍在)→ 维持,登记不动作,契约第 1 条成为后续同步点。
4. **测试同步**:纯登记 + 条件触发的依赖变更;变更触发时(步骤 2 升 tauri-service 分支)以 `npm run e2e` 全量为回归面,无新单测。无变更则无测试动作。
5. **文档同步**:
   - `e2e-tests/CLAUDE.md` 新增上节;
   - `.claude/adr.md:153` ADR-0006 该句后追加:「成因与『谁钉谁』对齐契约登记于 `e2e-tests/CLAUDE.md`(CP-032);上游放开硬钉后逐条去 overrides 化。」
   - 若步骤 2 走了升级分支,主 package.json devDependencies 的 `@wdio/tauri-service` 版本号同步改。
6. **验证**:
   - `grep -n "对齐契约" e2e-tests/CLAUDE.md` 命中;表格 7 行 override 全列
   - `npm ls webdriverio @wdio/globals 2>&1` 输出 dedupe 单实例树(退出码 0)
   - 未走升级分支时 `git diff --stat package.json package-lock.json` 为空(登记不夹带依赖漂移)
   - 走升级分支时 `npm run e2e` 全绿

---

## CP-033 · KaTeX 字体内联产物:维持期 CI diff 守卫 + S10 独立 webview 重实证 [Stage S10]

1. **位置**:
   - `scripts/gen-katex-inline.mjs`(全 57 行,生成脚本)、`src/panels/markdown/generated/katexInlineCss.ts`(产物,实读 369,263 字节 ≈ 361KB,与 ADR-0018「~360KB」一致)
   - `src/panels/markdown/mdPipeline.ts:36`(`import { KATEX_INLINE_CSS } from "./generated/katexInlineCss";`)、`:237-243`(`buildPreviewDocument` head `<style>${buildMdPreviewStyleCss()}${KATEX_INLINE_CSS}</style>`)
   - `src-tauri/tauri.conf.json:25-26`(CSP:`img-src 'self' data: asset: https://asset.localhost; font-src 'self' data:`;`dangerousDisableAssetCspModification: ["script-src"]`)
   - `src/panels/markdown/CLAUDE.md:23`(KaTeX 字体节)、`.claude/adr.md:430/435`(ADR-0018 决策与被否决备选)
   - `.github/workflows/ci.yml`(无现有守卫步骤;L2/L3/E2E 段 :84-110)
   - 既有 L4 断言:`e2e-tests/markdown.e2e.ts:136-138`(渲染产物含 katex/katex-display + 内联字体装配断言)
2. **现状**:gen 脚本头注释自述「~0.9MB 文本」为脚本初版遗留描述,实际产物 361KB;防漏跑仅靠产物头「勿手改」注释与 mdPipeline CLAUDE.md 口头约定,无 CI 守卫。ADR-0018:435 被否决备选(asset 协议 + convertFileSrc)的否决前提是「opaque origin iframe 内行为未实证」——S10 预览迁独立 webview 后此前提变化,正是重实证窗口。
3. **修复步骤**:
   - **A. 维持期 CI diff 守卫(S10 随批落地,两分支都要做)**:
     1. `.github/workflows/ci.yml` 在「Dead code check (knip)」步骤(:55-57)之后插入:
        ```yaml
            # KaTeX 内联产物 diff 守卫（CP-033）：katex 升级漏跑 gen 脚本即红
            - name: Guard — KaTeX 内联产物与生成脚本一致
              run: |
                node scripts/gen-katex-inline.mjs
                git diff --exit-code src/panels/markdown/generated/katexInlineCss.ts
        ```
     2. `scripts/gen-katex-inline.mjs:8` 注释「~0.9MB 文本」改为「产物约 361KB(2026-09-06 实测),勿手改」——消除登记失真。
   - **B. S10 重实证(前置:S10 预览迁独立 webview 完成之后执行)**:
     1. **实证步骤(固定)**:真实 WebView2 环境渲染含数学公式(`$x^2$` + 块级)的 md,断言:公式 DOM `getComputedStyle` font-family 命中 KaTeX 字体族(非 serif 回退)、DevTools 网络面板无字体请求失败/CORS 拒绝。实证通道:新增临时 L4 spec 或在 markdown.e2e.ts 加守卫用例(见测试同步)。
     2. **分支 B1(asset 通道可行)→ 删生成物**:
        - 删 `scripts/gen-katex-inline.mjs`、`src/panels/markdown/generated/katexInlineCss.ts`;
        - `mdPipeline.ts:36` 删 import,`:240` head 改为经 asset 协议引用 katex css/字体(具体 URL 形态按 S10 webview 架构定,约束:运行时取字体、渲染产物不含 data: 字体串);
        - `tauri.conf.json:25` CSP `font-src` 增 `asset: https://asset.localhost`(与 img-src 同形态);
        - 按「测试同步」改 L4 断言通道;
        - `src/panels/markdown/CLAUDE.md:23` 节改写为运行时取字体口径;
        - ADR-0018 追加「逆转记录」节(见文档同步)。
     3. **分支 B2(仍不可行)→ 局部 CSP 兜底**:保留生成物;在 S10 新 webview 的局部 CSP 中显式放行 `font-src data:`(主窗口 CSP 不动);ADR-0018 追加实证结论(不可行证据 + 保留决策),CI 守卫(A)成为长期形态。
4. **测试同步**:
   - A 步:CI 守卫自身即测试,无单测;
   - B1 分支:`e2e-tests/markdown.e2e.ts:136-138` 的「KaTeX 内联字体已装配」断言改为断言运行时字体通道(具体选择器/URL 随 S10 架构定,断言语义 = 公式字形命中 KaTeX 字体族);新增用例名建议「数学公式_块级与行内_字体族命中 KaTeX(非回退 serif)」。L2 面:`markdown-render-pipeline.test.ts` 若断言 head 含 `KATEX_INLINE_CSS` 内联特征串,同步改接新通道;
   - B2 分支:L4 无新增,维持 A 守卫。
5. **文档同步**:
   - A 步:`src/panels/markdown/CLAUDE.md:23` 句尾追加「一致性由 CI diff 守卫(.github/workflows/ci.yml,katex 升级漏跑即红,CP-033)」;
   - B1 分支:`src/panels/markdown/CLAUDE.md:23` 整节改写(运行时 asset 通道 + CSP 放行口径);ADR-0018:443「逆转触发点」节后追加「**逆转记录(CP-033)**:<日期>独立 webview 前提下重实证 asset 字体 CORS 通过,生成物删除,转运行时通道」;
   - B2 分支:ADR-0018 追加「**维持记录(CP-033)**:<日期>重实证仍不可行(<证据>),保留构建期内联 + 新 webview 局部 font-src data: 放行」。
6. **验证**:
   - A 步:守卫步骤存在 `grep -n "KaTeX 内联产物 diff 守卫" .github/workflows/ci.yml`;**红测**:本地临时改 katexInlineCss.ts 一个字符后 `node scripts/gen-katex-inline.mjs && git diff --exit-code src/panels/markdown/generated/katexInlineCss.ts` 退出码非 0,还原后退出码 0;
   - B1:`test -f src/panels/markdown/generated/katexInlineCss.ts` 假、`grep -rn "gen-katex-inline" scripts/ src/` 零命中、`npm run e2e` 中 markdown spec 全绿且字体断言命中;
   - B2:`grep -n "font-src" src-tauri/tauri.conf.json` 维持 `'self' data:`,新 webview CSP 处可见局部放行。

---

## CP-038 · @types/markdown-it 精确 pin 改回 ^14.2.0 [Stage S01,与 CP-027 同 agent]

1. **位置**:`package.json:67`(`"@types/markdown-it": "14.2.0",`);ADR-0006 依赖版本策略 `.claude/adr.md:143-159`(:152「devDependencies(开发工具)全 `^`」)
2. **现状**:pin 自 `78cb1b8`(md 渲染纯管线 S5,首次引入即精确形态 `14.2.0`)至今零改动;`git log -S '"@types/markdown-it": "^14.2.0"'` 全历史无命中,即**从未存在过 ^ 形态,也无 pin 成因登记**——无成因路径坐实(注:markdown-it 本体 15.0.1 与 @types 14.x 的 major 错位是 DefinitelyTyped 版本号常态,非 pin 理由)。
3. **修复步骤**:
   1. `package.json:67` 改为:
      ```json
          "@types/markdown-it": "^14.2.0",
      ```
   2. `npm install` 刷新 lockfile(`^14.2.0` 解析至 14.2.x 最新;若上游已发 14.3.x 亦合法,ADR-0006 允许 dev `^` 浮动);
   3. 若刷新后 `npx tsc --noEmit` 报 @types/markdown-it 相关新错误 → **pin 成因浮现路径**:不改回,在 ADR-0006「后果」节补例外登记(形态照 :159 xterm 例外句:「例外登记:@types/markdown-it 精确 pin(类型定义与 markdown-it 15.x 不兼容,<错误摘要>),升级 markdown-it/types 时重估」),compromises.md CP-038 条目按「有成因」改写;
   4. 无错误 → 无需任何登记(改回即符合 ADR-0006 现状口径)。
4. **测试同步**:无新用例(版本声明变更无新行为);**回归面** = L2 markdown 系全量:`src/__tests__/markdown-assets.test.ts`、`markdown-links.test.ts`、`markdown-mermaid.test.ts`、`markdown-panel.test.tsx`、`markdown-render-async.test.ts`、`markdown-render-pipeline.test.ts`(六件全跑,锁定类型漂移不破坏渲染管线)。
5. **文档同步**:无(正常路径零文档变更);仅步骤 3 异常分支触发的 ADR-0006 例外登记。
6. **验证**:
   - `grep -n '"@types/markdown-it": "\^14.2.0"' package.json` 命中
   - `npm ls @types/markdown-it` 退出码 0,版本 ∈ 14.2.x+
   - `npx tsc --noEmit` / `npx eslint src/` 退出码 0
   - `npx vitest run src/__tests__/markdown-` 六文件全绿

---

## 起草附注

1. **CP-003 行号精确**:compromises.md 登记的 run-wdio.cjs:246-288 与现状一致;替换时注意 :286-288 的 `else { fallback(); }` 属同一块,新块末尾的 `fallback();` 已合并其语义,勿残留旧 else。
2. **CP-002 触点扩面**:决策只点名 JsonMode.tsx,实读发现同摘触点还有 package.json:50(`json-schema` 仅 JsonMode.tsx:25 类型 import 消费)、vitest.config.ts:9-16、vitest.l3.config.ts:8-15(inline 登记)、hooks-config-jsonmode.test.tsx 大面积 mock——清单已全部写入步骤,执行 agent 不得只改 JsonMode.tsx。
3. **CP-038 成因定论**:pin 是 78cb1b8 首次引入即精确形态,非后期改动引入;「git log/blame 查成因」的答案就是「无成因」,清单按无成因路径(改回 ^ + L2 回归)落步骤,异常分支(pin 成因浮现)也有落点。
4. **CP-032 成因已实挖**:无需执行 agent 再考古——a027b17(前 4 项,serialize-javascript 为 RCE 消缺,其余 dedupe)与 1233336(后 3 项,对齐 tauri-service 硬钉)两条提交即全造成因;评估动作只剩「上游是否放开硬钉」一项(`npm view` 当场可判)。
5. **knip.json 旧路径遗留(非本批范围)**:`knip.json` ignoreIssues 仍列 `src/panels/hooksConfig/*`(:156-184,F11 迁址后旧目录已不存在,实读 `src/panels/hooksConfig` 缺席);CI 的 `npx knip --production` 当前绿,knip 对不存在文件键不报错。CP-002 删 `json-schema` 依赖时 knip 会实际校验 unusedDependencies——若漏删必红,这正是验证段列 knip 的原因。旧路径清理建议归 S08 同 agent 顺手或另立条目。
6. **CP-003 同步面扩面**:决策只点名 e2e-tests/CLAUDE.md:23-25,实读同述「Node 22 便携启动器」的还有 wdio.conf.ts:5 与 run-wdio.cjs:1-4,及 CLAUDE.md:42 glyph-repro 节的「Node 26→便携 22 切换兜底」句——清单已全列;三处文本在 S01(CP-032 插入新节)与 S03(本项改写)间有邻接,S01 插入点锚定「### E2E helper 命名与挂载位置」节可避免行号漂移。
7. **CP-033 产物体积登记失真**:gen 脚本头注释「~0.9MB」为初版遗留,实际产物 369,263 字节(≈361KB,与 ADR-0018 一致)——A 步已含注释修正。
8. **e2e-tests/CLAUDE.md:42 禁用裸 npx wdio 的理由换锚**:CP-003 后版本坑消失,禁用理由换为隔离链(SLTERM_DATA_DIR/USERPROFILE 由启动器注入),语义不变、事实更新。
9. **Stage 编排文件重叠线索**:S01(CP-001/032/038)只碰 scripts/(新增)、e2e-tests/CLAUDE.md(插入)、package.json:67(改字符)、adr.md/compromises.md(改写);S03(CP-003)碰 run-wdio.cjs、package.json(engines)、e2e-tests/CLAUDE.md(相邻节改写)——S01 先于 S03 无冲突;S08(CP-002)碰 vitest 双配置 + cliProfiles 域 + __tests__ 两件,S10(CP-033)碰 ci.yml + mdPipeline + tauri.conf.json,与 S08 零重叠。
