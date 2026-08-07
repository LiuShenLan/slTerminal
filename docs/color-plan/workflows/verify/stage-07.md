# Stage 07 逐项验证断言（唯一真值源）

> 验收 Stage：spec §10 六项全过。ACC-03/04/05 为人工验证点，自动化部分只验 ACC-01/02/06。

## 断言清单

| # | 对应项 | 断言 | 检验方法 |
|---|--------|------|---------|
| 1 | ACC-01 | `npx tsc --noEmit` exit 0；`npx eslint src/` exit 0 | 门禁产出 |
| 2 | ACC-02 | `npm test`（L2）exit 0；`npm run test:l3`（L3）exit 0 | 门禁产出 |
| 3 | ACC-06 | `npm run build:e2e` 成功；`npm run wdio` 关键路径（启动/终端/编辑器 spec）通过 | 门禁产出 |
| 4 | ACC-03 | **人工**：零视觉截图对比——重构前后主界面截图逐项一致（终端/编辑器/diff/侧栏/活动栏/dockview 页签/allotment sash），用户签字确认 | 人工记录 |
| 5 | ACC-04 | **人工**：降级冒烟——settings.json 写 `colorScheme: "不存在"` → 启动回退 darcula + console.warn，界面正常，用户签字确认 | 人工记录 |
| 6 | ACC-05 | **人工**：五通道切换冒烟——临时注册改单色测试方案 → 指向 → 重载 → 五通道（React inline style / xterm ITheme / CM6 theme / dockview CSS 变量 / allotment CSS 变量）全生效 → 还原，用户签字确认 | 人工记录 |
| 7 | 完整性 | 全部 Stage（01-06）commit 存在且 message 与 execution-plan 总表一致 | git log --oneline |

## 全量测试（全部通过为门禁）

- `npx tsc --noEmit` → exit 0
- `npx eslint src/` → exit 0
- `npm test` → exit 0
- `npm run test:l3` → exit 0
- `npm run build:e2e` → exit 0
- `npm run wdio` → 关键 spec 通过
