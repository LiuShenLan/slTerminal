# 设置中心

> 统一设置入口、页面注册表、分组模型、脏守卫与写通道。

## 决策与红线

- **统一入口 + 页面注册表**：设置页经注册表登记稳定路由 id、视觉分组、顺序与双语搜索别名；`register_builtin_pages` 幂等注册，重复构造不产生重复页。原因：新增页零框架改动。边界：路由 id 只追加不重用；视觉顺序由 `order` 单独裁定，与 id 解耦。
- **分组模型**：`SettingsPageGroup` 只有 global 与 agent；组序 global 在上、agent 在下。Agent 组按 CLI 分节，分节标题不可点，页 id 形态为 `agent.<cliId>.<page>`。原因：别名/hooks 是 agent 维度配置，不是项目维度。
- **hooks 直渲染**：`agent.<cliId>.hooks` 直接渲染 profile 的配置编辑器，不设 hub 与 CLI 选择行；页注册经 `capabilities.hooks.hasConfigEditor` 门控。
- **别名归基础页**：CLI 别名作为 `agent.<cliId>.basic` 的一个节，不单列页面。
- **动态注册补注册**：agent 页枚举抽为幂等导出函数；运行时动态注册 profile 的场景须手动调用补注册。
- **保存模型**：各配置页自治（即时保存页离散提交不上报脏）；壳层汇聚 `onDirtyChange`，导航圆点提示；切换页/关闭/自动关闭三处守卫走确认。原因：dirty 状态必须跨页面可见。边界：保存中在途一律拦截本轮。
- **写通道二分**：前端消费型配置走通用段写（`settings_store`）；后端消费型配置走域专用命令（校验 + 内存态 + 落盘一体）。禁止自建第二写通道。原因：浅合并写会互覆，双通道会产生并发竞态。
- **持久化**：一律经 `settings_store` 与 `atomic_file`（临时文件 + rename、`.bak`、损坏处理）；文件名去品牌。原因：设置是用户数据的核心持久面。
- **搜索**：节级中英混合别名在页面注册表；控件级别名从键注册表按页面归属拼接，不重复维护。
- **备份入口**：安全分区承载恢复点清单与备份/恢复操作；凭据值与任意 home 路径永不进出包。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 虚拟窗口：`cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`。
- 必测面：注册表幂等、路由 id 稳定、分组与顺序、脏守卫三处路径、写通道二分、搜索别名、备份入口状态机。
- 真实安装/备份流程由构建产物人工验收，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 新增设置页：在注册表登记 id/分组/顺序/标题/搜索别名，再实现页面；后端消费型另加专用命令。
- 新增设置键：在 `slterm_settings::keys` 登记段归属，再接入页面与写通道。
- 修改分组或顺序：只改注册表元数据，不改路由 id。
