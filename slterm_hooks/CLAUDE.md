# slterm_hooks（core 层）

> hook 事件目录、schema、草稿模型与托管识别核心，不写文件。

## 决策与红线

- **非写盘边界**：只承载事件目录、schema、草稿模型与托管识别。原因：hook 语义与持久化必须分离。边界：不得写文件。
- **目录与订阅分离**：`HookEvent` 承载 Claude 全量事件目录；编辑器、schema 与保存校验用完整目录，安装器只注入 `CLAUDE_EVENTS` 订阅子集。原因：合法事件不得因未订阅而被拒绝保存，注入面也不得擅自扩张。
- **字段目录单源**：`HookHandlerField` 声明键名、控件形态、适用 handler 类型与必填子集；`HookHandlerType::fields`／`required_fields` 是 GUI 表单与保存校验的唯一来源。原因：两套字段矩阵会让新增字段漏改一侧。边界：字段文案与控件仍归 application 层，本 crate 不依赖 i18n。

## 验收与测试

- 定向测试：`cargo test -p slterm_hooks --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
