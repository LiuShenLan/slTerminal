# Review 05 · 测试覆盖缺口 + 遗留清理（CP-023/024/025/026/027/028/029/030/040/041/045/046）

## 问题清单

### 1. CP-024 销项注记「漂移守卫入门禁」失实——守卫仅为文档操作指令，CI 无机检承接
- **CP**: CP-024
- **位置**: `docs/compromises.md:101`（销项注记）；`.github/workflows/ci.yml`（无对应 step）；`src/types/CLAUDE.md:16`
- **问题**: CP-024 销项注记称「导出测试 + git diff --exit-code 漂移守卫入门禁」，但 ci.yml 中没有任何针对 `src/types/` 生成物的 Guard step（对照同批 CP-033 的 KaTeX 守卫在 ci.yml:59-63 有实体 `Guard —` step），package.json scripts 亦无。漂移守卫只存在于 CLAUDE.md 操作指令（src/types/CLAUDE.md:16、src-tauri/src/CLAUDE.md:71）。手改生成物后不跑导出的漂移在 CI 全绿下不可检出，「入门禁」系夸大。
- **证据**: `grep -rn "export_bindings\|git diff.*src/types" .github/ package.json .claude/package.ps1` 零命中（仅 knip/静态门禁 step）；ci.yml:59-63 仅为 KaTeX 同类守卫的实体形态，可作对照。
- **建议**: 在 ci.yml 增 `cargo test --test lib_tests export_bindings -- --test-threads=1 && git diff --exit-code -- src/types` Guard step（照 KaTeX 先例），或把注记改回「操作指令登记」口径。

### 2. CP-046 哨兵键集合声明失实——`env` 是套件自身写入面却未入哨兵，存在漏检口
- **CP**: CP-046
- **位置**: `e2e-tests/run-wdio.cjs:86-88`；`e2e-tests/CLAUDE.md:92`；反证 `e2e-tests/background-tasks.e2e.ts:261-272`、`e2e-tests/settings.e2e.ts:202-217`
- **问题**: 哨兵声明称「E2E **唯一**可能写入真实屋 settings.json 的键（hooks 注入 matcher / statusLine 桥接）；其余键（env/permissions/用户配置）外部并发修改合法」——但套件自身经 Node 侧 `writeFakePlanEnv` 向 user 层 `~/.claude/settings.json` 写 **`env` 键**（background-tasks C 用例与 settings ④用例两处逐字同构）。env 不属哨兵键：若该写入链（测试进程 `homedir()` 跟随 USERPROFILE）因任何原因落到真实屋，exit 哨兵校验对 env 不比对 → 静默漏检。注释把 env 归类为「外部并发修改合法」与套件实际写入面相矛盾。
- **证据**: background-tasks.e2e.ts:267-271 `env.ANTHROPIC_BASE_URL=...; env.ANTHROPIC_AUTH_TOKEN="sk-test-e2e"; root.env=env; writeFileSync(claudeSettingsPath,...)`；settings.e2e.ts:207-216 同构；`SETTINGS_SENTINEL_KEYS=["hooks","statusLine"]`（run-wdio.cjs:86）不含 env。
- **建议**: 哨兵键集合加 `"env"`（套件写入面全量入哨兵），并修订两处「唯一可能写入」声明。

### 3. CP-030 登记同条自相矛盾——「前提满足后 `$` 族命令正常速度」已被同条修正段推翻
- **CP**: CP-030
- **位置**: `e2e-tests/CLAUDE.md:116`
- **问题**: 同一条目内先写「前提满足后 `$` 族命令正常速度，cli-aliases 已回归真实手势（CP-030）」，随后 2026-09-08 修正段写明「聚焦前提满足（TQ-E-10 通过）时窗口态查询通道仍可能不可用……每 focus 命令确定性 +5s、**与窗口是否聚焦无关**」。前句在事实层面已被后句否证却未清理，读者无法判定 `$` 族命令的真实成本语义。附带：`this.timeout(120000)` 按同条自述吸收确定性惩罚（13 次 WARN ≈ +65s，预算余量 ~55s）——per-command 延迟增长在 ~1.8x 以内不会被任何断言感知，性能回归感知 sole 依赖该宽松预算。
- **证据**: e2e-tests/CLAUDE.md:116 单条内两句并置（「前提满足后 `$` 族命令正常速度」vs「每 focus 命令确定性 +5s、与窗口是否聚焦无关」）。
- **建议**: 删除/改写前句为「前提满足后命令可正确执行但每 focus 命令仍确定性 +5s（core.invoke 通道缺失）」，并把性能回归感知挂到更可观测的面（如 WARN 计数断言）。

### 4. CP-030 探针引入 runner 形态收窄——多 `--spec` 定向调用确定性 fast-fail，销项注记未提
- **CP**: CP-030
- **位置**: `e2e-tests/CLAUDE.md:123`；`e2e-tests/wdio.conf.ts:82-92`（探针本体）
- **问题**: TQ-E-10 探针的副作用：多 spec 定向形态（`run-wdio.cjs --spec a.e2e.ts --spec b.e2e.ts`）会为每个 spec 起独立 worker/独立应用实例，第 2 位应用实例在 Windows 前台锁定期内无法取得前台聚焦 → 探针确定性 fast-fail（「谁在第 2 位谁失败」）。官方定向形态被收窄为「单 spec 分次调用」。该副作用只在 e2e-tests/CLAUDE.md 登记，compromises CP-030 销项注记只字未提（注记仅有探针 + 真实手势 + timeout 承接）。属修复引入的新债务，登记位置偏离销项主档。
- **证据**: e2e-tests/CLAUDE.md:123「`--spec a --spec b` 会为每个 spec 起独立 worker……后者无法取得前台聚焦，beforeSuite TQ-E-10 探针确定性快失败……多 spec 验证请分次单 spec 调用」。
- **建议**: compromises CP-030 注记补一句探针副作用与官方形态收窄，或将探针改为「首 worker 探针 + 其余 worker 跳过/降级 warn」。

### 5. CP-040 红线文档括注计数漂移——「721 例等价覆盖 --lib」已失真
- **CP**: CP-040
- **位置**: `.claude/CLAUDE.md:77`
- **问题**: 维持 TQ-COV-06 红线后，括注「（lib_tests = src/lib.rs 显式 test target，**721 例**等价覆盖 --lib）」的用例计数已随套件增长失真——S12 commit（6af3791）自述 L1 已达 825 例。操作规则本身仍准确（`--test lib_tests` 定向形态维持），仅计数括注失真；同文件豁免表 :29 的 827/711 计数带 2026-08-31 日期戳属历史记录不算失真，根文件括注无日期戳。
- **证据**: `.claude/CLAUDE.md:77`「721 例等价覆盖 --lib」 vs 6af3791 commit message「全量回归全绿：……L1 825」。
- **建议**: 括注去掉具体计数（改「全量 lib 单测」），或补日期戳对齐 test-exemptions 先例。

### 6. CP-046 文档同步遗漏——test-exemptions L4 运行机制节仍写旧 sha256 整文件口径
- **CP**: CP-046
- **位置**: `.claude/test-exemptions.md:70`
- **问题**: 真实屋校验机制已改为 settings.json 哨兵键级（hooks/statusLine 存在性+值比对，sha256 仅保留给 statusline-backup/hooks 树），但该文「L4 运行机制」节仍写「exit 时对真实屋做存在性 + sha256 快照比对（任何泄漏独立报红 exitCode=1）」——机制描述停留在 CP-046 前的整文件 sha256 形态。checklist CP-046 步骤 5 只点名 e2e-tests/CLAUDE.md 与 run-wdio 头注同步，遗漏了 test-exemptions 这处同机制登记，违反「修复若触及登记点原文须同步更新」。
- **证据**: `.claude/test-exemptions.md:70`「存在性 + sha256 快照比对」 vs 同文 :9 表头「三列以本表为唯一真值源」及 run-wdio.cjs:144-155 键级比对实现。
- **建议**: 该句改键级口径（照 e2e-tests/CLAUDE.md:92 表述）。

## 界外观察

- `e2e-tests/background-tasks.e2e.ts:261` 与 `e2e-tests/settings.e2e.ts:202` 的 `writeFakePlanEnv` 为逐字重复的两份副本（仅差 mkdirSync 容错三行）——pre-existing（3349e20/38037b1 引入，早于本修复链），非本次修复引入；可考虑收进 helpers.ts。注：上文问题 2 的 env 哨兵缺口与本观察同源于该函数的写入面。
- CP-023 的 89.55% 登记（23309/26029，llvm-cov Line 列）算术自洽（23309/26029=89.550%），本轮受「禁止全量套件」纪律限制未实跑 llvm-cov 复现，亦无反证；`#[coverage(off)]` 全仓零标注（唯一命中为 pty/CLAUDE.md:119 文档提及），无趁机豁免生产代码面。CP-040 embed-manifest「bins-only + 无宏 API」判定经 crate 源码（registry 缓存 embed-manifest-1.5.0，src/embed/mod.rs:93-111 仅 emit `cargo:rustc-link-arg-bins`，全 crate 无 macro_rules/#[macro_export]）一手证实，失实判定属实。
