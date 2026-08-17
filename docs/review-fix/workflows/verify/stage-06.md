# Stage 06 逐项验证断言（唯一真值源）

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> **高风险 Stage**：自动化断言通过不代表流畅度——人工实测点（claude 高输出/滚轮/kill）见 execution-plan.md 第 5 节。

## 断言清单

- **BE-05**：`src-tauri/src/pty/reader.rs` 的 reader_loop 存在 read 成功后非阻塞续读累积逻辑（语义式，须 Read 确认：try_read 或等效非阻塞读，累积上限 64KB——不限变量名）
- **BE-05**：reader_loop **无固定延迟定时器**（语义式，须 Read 确认：微批靠「读到即续读」，不存在 sleep/timer 等待攒批——发现定时器攒批判 not_fixed）
- **BE-12**：`grep "ring_buffer_append" src-tauri/src/pty/reader.rs` 调用点 = 1（批量一处）
- **BE-06**：`src-tauri/src/pty/spawn.rs` 的 `pty_kill` 不再丢弃 kill 返回值（`let _ = child.kill()` 零命中）；失败有 warn 日志（Read 确认）
- **BE-06**：reader join 改为带超时机制（语义式，须 Read 确认：is_finished 轮询 + 约 3s 超时上限，超时放弃 join 记 warn——无裸 `join().unwrap()` 永久阻塞）
- **FE-18**：`src/panels/terminal/usePtyOutput.ts` 导出 `dispose()`（清双定时器 + buffer，Read 确认）；`src/panels/terminal/useXterm.ts` cleanup 调用 dispose（grep 命中）
- **FE-18**：usePtyOutput 直接写阈值常量 = 256（grep 命中）；2ms 空闲/16ms 强制参数未变（Read 确认）
- **FE-18**：新增 L2 用例存在（卸载后定时器不触发、阈值边界）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npm run test:l3`
