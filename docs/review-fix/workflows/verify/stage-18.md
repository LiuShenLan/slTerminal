# Stage 18 逐项验证断言（唯一真值源）

> stage-18 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> **注意**：自动化断言通过不代表交互无回归——大目录实测点见 execution-plan.md 第 5 节。

## 断言清单

- **FE-30**：`src/features/explorer/FileTree.tsx` 存在窗口化渲染逻辑（语义式，须 Read 确认：扁平化可见节点数组 + 固定行高 + overscan 滚动窗口——三要素齐备，不限实现细节）
- **FE-30**：零新依赖——`git diff package.json` 无新增 dependencies（grep 确认）
- **FE-30**：L2 测试存在窗口化断言（构造约 1000 节点树，断言实际渲染行数远小于节点总数——Read 测试确认）
- **FE-30**：键盘导航/右键菜单/选中模型既有 L2 用例全绿（门禁命令 4 佐证；若用例被删改削弱判 partial 并说明）
- **FE-30**：S08/FE-07 错误占位在虚拟化下仍渲染（语义式，须 Read 确认错误占位未被窗口化裁掉）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
