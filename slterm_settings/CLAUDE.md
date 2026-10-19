# slterm_settings（core 层）

> 设置与主题色表的值权威：值模型、JSON 值层、键注册表与路径推导；不持写通道硬机制。

## 决策与红线

- **零生产依赖**：JSON 值层由内部模块自实现；不得新增任何生产依赖（`architecture/dependencies.toml` 机械强制）。
- **路径唯一权威**：`settings_dir()`/`settings_path()` 是数据目录与文件名唯一定义点；app 侧只委托不推导。默认 `%APPDATA%\slterm`，`SLTERM_CONFIG_DIR` 优先，空串视为未设置；回退链 APPDATA → USERPROFILE → temp。原因：路径推导双源会导致读写分叉。边界：其他模块不得自行拼接数据目录。
- **段嵌套是磁盘契约**：每个键归属一个语义段，「键 → 段」映射单源在 `keys` 注册表；未登记键读不出，未知键/用户手改段在对象树原位保留。原因：浅合并写不得丢用户手改内容。边界：新增键必须先登记段归属。
- **宽容读取语义**：键缺失/类型不符/null 一律回退默认；授权路径（安装器）用 `try_load`，IO 错误与损坏不得当默认。原因：用户手改文件不应导致启动失败。边界：授权路径必须区分「缺失」与「损坏」。
- **契约数值**：`MAX_PERSIST_BYTES` 等上限是外部可依赖面，改动须评审并补测试。
- **主题语义合同**：`ThemeName::BUILTIN` 是内置暗色主题目录，Linear 为默认，未知/退休 id 回退 Linear；`ReviewedPalette` 与语义槽 WCAG 对比度阈值是准入门。原因：仅暗色定位与可读性是产品约束。边界：新增主题必须过校验，不达标不得入目录。
- **BlurMode 与外观值**：`BlurMode` 枚举表达窗口材质档位，默认 `None`（性能安全档）；具体档位与映射以类型定义为准。原因：bool 无法表达材质与性能档位。边界：材质平台调用只在平台封装模块。
- **零迁移**：新版只读写 `%APPDATA%\slterm`；旧版 exe 同级 `settings.json` 不看不迁。原因：全新 fork，迁移机制成本高于重配。边界：旧数据不构成兼容义务。
- **文件命名去品牌**：`settings.json`、`session.json`、`providers.json`、`history.jsonl`、`directory_history.json`、`runtime.port`、`themes/*.slterm-theme.json`。原因：目录名已带品牌，文件名不重复品牌，避免下次改名产生迁移链。

## 验收与测试

- 定向测试：`cargo test -p slterm-settings --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- JSON 往返、键注册表完备性、宽容读取语义、路径优先级与 WCAG 边界是必测面。
- 测试分层与隔离规则见 `.claude/docs/testing/strategy.md`。

## 操作

- 修改设置键：先在 `keys` 注册表登记段归属，再改值模型与读写转换；同步补往返测试。
- 新增主题：加入内置目录 + 调色板，并通过语义槽 WCAG 校验。
- 数据目录解析顺序与文件名是本 crate 的唯一权威，app 侧不得复制。
