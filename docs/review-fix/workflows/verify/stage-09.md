# Stage 09 逐项验证断言（唯一真值源）

> stage-09 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **BE-16**：`src-tauri/src/app_dir.rs` 存在；`grep "use crate::app_dir" src-tauri/src/settings.rs src-tauri/src/projects.rs` 命中；`src-tauri/src/lib.rs` 含 `mod app_dir;`；settings.rs 不再自有 `app_data_dir` 定义（grep 确认已上提）
- **BE-14**：`load_settings` 返回结构序列化形态为 `{ data, corrupted }`（Read serde 结构/命令返回类型确认字段名）；`load_projects` 同形态（data 为 String）
- **BE-14**：corrupted 判定三态正确（语义式，须 Read 确认：无文件 corrupted:false；解析失败回退 corrupted:true；**.bak 命中也 corrupted:true**）；L1 三态用例存在
- **SEC-11**：save_settings/save_projects 存在 1MB 大小上限（grep 命中常量）；settings 顶层键白名单校验（语义式核对集合 = fontSize/keybindings/sideBar/colorScheme）；projects 须为 JSON 对象校验；L1 用例存在（超限/非法键拒绝）
- **FE-11**：`src/ipc/settings.ts` `loadSettings` 返回 `Promise<{ data: Record<string, unknown> | null; corrupted: boolean }>`；`src/ipc/projects.ts` `loadProjects` 返回 `Promise<{ data: string; corrupted: boolean }>`（Read 确认）
- **FE-11**：stores/projects.ts、fontSize.ts、keybindings.ts、sideBar.ts 四 store 的 loadFromDisk 消费 corrupted → toast.show（grep 逐文件命中）
- **FE-11**：`src/main.tsx` 早期 loadSettings 调用适配新返回结构（Read 确认访问 `.data` 而非整体当数据用；corrupted 时 console.warn）
- **FE-11**：新增 L2 用例存在（corrupted:true → toast；正常数据消费）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
