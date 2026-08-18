# slTerminal review 修复——执行编排参数

- **配套**：`checklist.md`（93 项）、`stages.md`（19 Stage 划分与契约）——本文件只写编排参数，通用执行规则见 `/systematic-changes-execute`
- **命令真值源**：`.claude/skills/systematic-changes-plan/config.json` `commands`——本计划只引用键名，不复制命令字符串

## 1. Stage 编排表

| Stage | 脚本 | 并行结构 | 门禁命令（config 键名） | commit message |
|-------|------|----------|------------------------|----------------|
| S01 | `stage-01-deps.js` | 3 并行 | tsCheck, eslint, clippy, frontendTest, rustTest, viteBuild, e2eBuild | `chore(deps): WDIO 9.30.1/git2 0.21/Tauri patch 升级消 serialize-javascript RCE + 补隐式依赖声明 + knip 配置（TE-01/02/05/06/12）` |
| S02 | `stage-02-backend-security.js` | 2 并行 | 基础五条 | `fix(security): shell 白名单真实路径校验 + PTY 会话上限 32 + 信号文件 symlink 过滤（SEC-01/02、BE-01）` |
| S03 | `stage-03-whitelist.js` | 2 串行 pipeline | 基础五条 | `fix(security): 删除无消费 pty_reattach + 32 条命令白名单化（SEC-03/07）` |
| S04 | `stage-04-async-root.js` | 2 并行 | 基础五条 | `fix(backend): set_project_root/notify_watch 异步化 + 失败清空旧 root + 前端失败 toast 降级（BE-04、SEC-14、FE-04）` |
| S05 | `stage-05-watcher.js` | 2 并行 | 基础五条 | `fix(notify): watcher 事件侧排除大目录 + symlink 过滤 + notify_stop_watch 生命周期命令 + 池容量 8（BE-02/10/11、SEC-08）` |
| S06 | `stage-06-reader-batch.js` | 2 并行 | 基础五条 + l3Test | `perf(pty): reader 微批处理降 IPC 频次 + kill 可靠性加固 + 前端输出 dispose 与阈值上调（BE-05/06/12、FE-18）` |
| S07 | `stage-07-fs-chunk.js` | 2 并行 | 基础五条 | `perf(fs): fs_read_file 改 Channel 分块推送削大文件内存/IPC 峰值（BE-03，D3）` |
| S08 | `stage-08-errors.js` | 4 并行 | 基础五条 | `feat(error): 统一 AppError 解析器 + ConfigParse 变体 + 启动链/终端/编辑器错误可感知化（FE-02/03/05~10、BE-13/15）` |
| S09 | `stage-09-corrupted.js` | 2 并行 | 基础五条 | `feat(persist): load 返回 corrupted 标志 + app_dir 模块上提 + 保存大小/schema 校验（BE-14/16、FE-11、SEC-11，D11）` |
| S10 | `stage-10-dto.js` | 2 并行 | 基础五条 | `fix(types): DirEntry/detail DTO 与 Rust 真实形态对齐 + HooksLayer 收窄 + pty 参数前置校验 + 后端 Layer 枚举（FE-12/13/14、BE-18）` |
| S11 | `stage-11-frontend-security.js` | 1 | 基础五条 | `fix(security): HTML 预览 postMessage nonce 校验 + fail-safe 页去 innerHTML + 剪贴板权限消费点守卫测试（SEC-04/06/10）` |
| S12 | `stage-12-frontend-perf.js` | 4 并行 | 基础五条 + l3Test | `perf(frontend): 页数上限 20 + 树/订阅/启动加载性能优化 + 历史扫描缓存（FE-01/15/16/17/19/20/21/29/32/33/34、BE-19）` |
| S13 | `stage-13-stability.js` | 3 并行 | 基础五条 | `fix(stability): 面板级错误边界 + 生命周期守卫（AbortSignal/genRef/dispose）+ pty_kill_all 兜底 + git 缓存 LRU（FE-22~28、BE-07/08/09）` |
| S14 | `stage-14-deadcode.js` | 3 并行 | 基础五条 | `refactor(deadcode): 删除无消费 barrel/常量/setFocus + 移除 allow(dead_code) + 测试 cfg 改运行时分支（FE-35、BE-17/20）` |
| S15 | `stage-15-major-upgrades.js` | 1+3 串行 pipeline | 步骤内：tsc+eslint+npm test+l3；Stage 门禁：基础五条 + l3Test, viteBuild, e2eBuild | 单条 `chore(deps): major 升级——jsdom 30/typescript 7/json-schema-library 11/dockview-react 8（TE-07/08/09/10）`（被回滚步骤注明） |
| S16 | `stage-16-ci-version.js` | 2 并行 | 基础五条 | `chore(deps): 版本策略统一（生产精确/开发 ^）+ CI 增 audit/knip 门禁 + ADR 登记（TE-03/04/11/13）` |
| S17 | `stage-17-hooks-validate.js` | 2 并行 | 基础五条 | `fix(security): hooks 写入语义校验 + user 层二次确认 + statusline 审查 warn + 脚本哈希比对（SEC-05/12/13，D9）` |
| S18 | `stage-18-filetree-virtual.js` | 1 | 基础五条 | `perf(explorer): FileTree 虚拟化（零新依赖手实现）（FE-30）` |
| S19 | `stage-19-docs.js` | 3 并行 | tsCheck（文档 Stage 仅静态门禁，防误改代码） | `docs: 约束修订 + 豁免/决策汇总登记 + 模块 CLAUDE.md/test-inventory 全量同步（DOC-01~10）` |

> 「基础五条」= tsCheck + eslint + clippy + frontendTest + rustTest。
> S19 特殊：文档 Stage 不跑测试套件，但 verify 前须确认工作树无代码文件变更（git diff 仅 `.md`/`CONTEXT.md`/`README.md`）。

## 2. git add 路径枚举（本任务）

config `workflow.gitAddPaths` 默认值 + 本任务扩展，合并为：

```
src/
src-tauri/
e2e-tests/
test/
docs/
package.json
package-lock.json
knip.json
.claude/CLAUDE.md
.claude/test-inventory.md
.claude/adr.md
.github/
CONTEXT.md
README.md
```

## 3. fix-loop 调用规范

`docs/review-fix/workflows/fix-loop.js` 每 Stage 修复循环统一 args（与脚本内强制校验一致）：

```js
{
  stage: <N>,                                          // 1..19
  failedItems: [...],                                  // 必填非空，来自 Stage verify 结果
  fixContext: "...",                                   // verify agent 的 details 证据原文
  verifyFile: "docs/review-fix/workflows/verify/stage-NN.md",  // 必填
  constraints: "<可选，Stage 特殊纪律——值见对应 Stage 脚本头注释>",
  testCommands: ["<可选>"]  // 缺省 = 基础五条；S01/S06/S12/S15 按第 1 节门禁列全量传入；S19 传 ["npx tsc --noEmit"]
}
```

## 4. 进度跟踪表（执行期填写）

| Stage | 状态 | commit | 人工验证点 | 验证结果 |
|-------|------|--------|-----------|----------|
| S01 | ✅ | a027b17 | E2E 全量实跑 | 待收尾实测 |
| S02 | ✅ | 2d66e05 | — | — |
| S03 | ✅ | fa77b87 | 白名单后全功能实测 + E2E | 待收尾实测 |
| S04 | ✅ | ac51bf2 | — | — |
| S05 | ✅ | 3789d61 | 大仓库事件量实测 | 待收尾实测 |
| S06 | ✅ | 3e30ab6 | claude 高输出流畅度 + 滚轮 + kill | 待收尾实测 |
| S07 | ✅ | 00afa38 | 10MB 大文件打开 | 待收尾实测 |
| S08 | ✅ | a185b40 | — | — |
| S09 | ✅ | eede19e | 损坏 settings/projects toast | 待收尾实测 |
| S10 | ✅ | 20e88da | — | — |
| S11 | ✅ | 3d27187 | — | — |
| S12 | ✅ | 88c4caa | WebGL 焦点切换 + 20 页上限 | 待收尾实测 |
| S13 | ✅ | fb98740 | 面板崩溃隔离 + 关窗 PTY 全灭 | 待收尾实测 |
| S14 | ✅ | 774a99a | — | — |
| S15 | ✅ | 6216921 | dockview 布局实测 + E2E | 待收尾实测 |
| S16 | ✅ | f08a30f | CI 三项新门禁实跑 | 待收尾实测 |
| S17 | ✅ | a5fa4fc | user 层确认弹窗 | 待收尾实测 |
| S18 | ✅ | 89fa372 | 万级节点目录实测 | 待收尾实测 |
| S19 | ✅ | b920027 | — | — |

## 5. 人工验证点汇总（收尾实测清单）

全部 Stage 自动化通过后，逐项人工实测（来源：stages.md 各 Stage 标注）：

1. **S01**：`npm run e2e` 全量（WDIO 9.30 链路）
2. **S03**：debug 构建产物全功能实测（终端/文件/git/hooks/历史）+ E2E——白名单权限缺失表现为 invoke reject
3. **S05**：含 node_modules/target 的大仓库文件变更刷新正常且 CPU 占用下降
4. **S06**：claude 高输出流畅度 + 鼠标滚轮 + kill 终端——**批处理致可见延迟/吞输入即回滚 S06**
5. **S07**：近 10MB 文件编辑器正常渲染；超 10MB 拒绝不变
6. **S09**：手改 settings.json 非法 → 启动 toast + 默认值兜底；bak 恢复同样 toast
7. **S12**：多终端焦点快速切换 WebGL 无异常；第 21 页 addPage 拒绝 toast；侧栏切换行为符合 ADR-0001
8. **S13**：面板渲染抛错同页其他面板存活；关窗后无残留 pwsh/cmd 子进程
9. **S15**：dockview 8 拖拽/分屏/旧布局 JSON 恢复 + E2E
10. **S16**：push 后 CI 三门禁（npm audit / knip / cargo audit）通过
11. **S17**：面板改 user 层 hooks 弹确认；非法事件名后端拒绝
12. **S18**：万级节点目录展开/滚动/键盘导航/文件操作刷新
