# Stage 08 逐项验证断言（唯一真值源）

> stage-08 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **GL-01**：grep `::-webkit-scrollbar` 命中 `src/App.css`；Read 确认 9px/轨道透明/滑块三档透明度/圆角 5/无箭头
- **GL-02**：grep `:focus-visible` 命中 `src/App.css`（1px #6e9ff2 outline）；grep `outline` 全仓 `src/`（排除测试）逐处 Read 确认——outline none 仅存于鼠标语义处，键盘可达元素有焦点环路径
- **GL-03**：grep `borderRadius\|border-radius` 全仓 `src/`（排除测试）命中值均在 4/5/6/8/pill/0 档内（语义式：不限写法，抽样 Read 至少 10 处确认；阶梯外残留须为零或附豁免理由）
- **GL-04**：Read 确认活动栏 46px/树行 28px/会话行 30px（前序 Stage 产出）；grep `padding\|margin` 抽样确认间距 4/8/12/16/24 档（语义式，豁免须附理由）
- **GL-05**：Read Watermark/navTree 空态/ExplorerPanel 空树确认统一形态（图标 fg-4 + 说明 fg-3 + 可选次按钮）；Watermark 按钮 addPanel 行为不变（测试断言）
- **GL-06**：`grep "fontSize.*\`[^ ]*1[4-9]px\|font-size: *1[4-9]px\|fontSize.*20px\|font-size: *20px" src/`（排除测试与终端/编辑器语义值）零残留或逐处豁免理由；grep `fontWeight\|font-weight` 仅 400/500/medium/normal（语义式确认）
- **GL-07（人工验证点，标注 skipped-manual）**：dockview sash 拖拽热区 ≥4px 实机手感确认——不纳入 allFixed 判定

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
