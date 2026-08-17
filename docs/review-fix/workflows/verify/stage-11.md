# Stage 11 逐项验证断言（唯一真值源）

> stage-11 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SEC-04**：`src/panels/html/HtmlPanel.tsx` 存在 nonce 三段（语义式，须 Read 确认：① 面板挂载生成随机 nonce——crypto.getRandomValues 或等效；② nonce 经注入脚本传入 iframe；③ 父窗口 message 监听校验 nonce 不符静默丢弃）
- **SEC-04**：新增 L2 用例存在（伪造消息无 nonce/错误 nonce 不触发快捷键；合法 nonce 触发）
- **SEC-10**：`grep "innerHTML" src/main.tsx` 零命中；fail-safe 页改 createElement + textContent（Read 确认视觉元素等价）
- **SEC-06**：`src/__tests__/` 下存在剪贴板消费点守卫测试（语义式：断言 readText 仅出现于 src/ipc/clipboard.ts、src/panels/terminal/keyboard.ts 及测试文件——Read 测试确认断言路径集合与之一致）
- **SEC-06**：`grep -rn "readText" src/ --include="*.ts" --include="*.tsx"` 命中仅 src/ipc/clipboard.ts 与 src/panels/terminal/keyboard.ts（测试文件除外；新命中判 not_fixed——守卫本身须与现状一致）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
