# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SEC-01**：`rg "script-src 'unsafe-inline'" src-tauri/src/preview.rs` ≥ 2 命中（HOST_PAGE 常量 + 测试断言各至少 1）；`rg "style-src 'unsafe-inline'" src-tauri/src/preview.rs` ≥ 2 命中；Read 确认两断言位于 `host_page_carries_domain_csp` 用例内且既有四断言未删改。
- **SEC-04**：`rg "下轮轮询自愈" src/` 零命中（含测试文件）；Read PreviewFrame.tsx syncNow catch 段确认：注释与 warn 文案均表述「等值轮询不重发，几何变化/主窗移动时重试」口径（语义式——与 syncNow 等值早退（几何五元组比较 return）的真实行为一致，不再出现「轮询自愈」类暗示下轮自动重发的措辞）；html-panel.test.tsx 若有 warn 文案断言则已同步适配（Read 确认，npm test 绿承载）。

## 全量测试（全部通过为门禁；逐条串行执行，禁并行）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `npm test`
6. `cargo test --test lib_tests preview -- --test-threads=1`（L1 定向红线：禁 --lib/裸 filter）
