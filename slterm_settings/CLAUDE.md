# slterm_settings（core 层）

## 存在理由与边界

设置与主题色表的**值权威**：本 crate 只做值模型、JSON 值层、键注册表与路径推导，不持任何写通道硬机制（白名单写盘、保存锁、`.bak`、损坏隔离归 app `settings_store`）。

- **零生产依赖契约**：JSON 值层由内部 `json` 模块自实现；不得新增任何生产依赖（`architecture/dependencies.toml` 机械强制）。
- **路径唯一权威**：`settings_dir()`/`settings_path()` 是数据目录与文件名唯一定义点；app 侧只委托不推导。`SLTERM_CONFIG_DIR` 优先、空串视为未设置。
- **段嵌套是磁盘契约**：每个键归属一个语义段，「键 → 段」映射单源在 `keys` 注册表；未登记键读不出，未知键/用户手改段在对象树原位保留（写通道浅合并不得丢）。
- **宽容读取语义**：键缺失/类型不符/null 一律回退默认；授权路径（安装器）用 `try_load`，IO 错误与损坏不得当默认。
- **契约数值**：`MAX_PERSIST_BYTES` 等上限是外部可依赖面，改动须评审并补测试。
- 主题目录与 WCAG 校验（`ThemeName::BUILTIN`、`ReviewedPalette`、对比度阈值）是本 crate 的语义合同；新增主题必须过语义槽校验。

## 测试

- 定向测试：`cargo test -p slterm-settings --test lib_tests <filter> -- --test-threads=1`；禁 `--lib`。
- JSON 往返、键注册表完备性、宽容读取三态、路径优先级与 WCAG 边界是必测面。
