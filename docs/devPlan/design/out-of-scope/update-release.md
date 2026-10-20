# 更新与发布链迁移

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

> 职责：设计更新检查、下载、校验、安装交接、安装后恢复，以及发布链、体积预算与合规落点；更新链最后迁移。

### 更新状态与检查

- 状态文件迁至 `state/update/`：
  - `prompt.json`：`prompted`、`skipped`、`remind_after`，按版本生效；新版本不继承旧版本延迟。
  - `downloads/`：已校验安装包与 `.part`。
  - `handoffs/<transaction>/`：`plan.json`、`ready.json`、`commit.json`、`cancel.json`、`result.json`、`restored.json`、`workspace.json`。
  - `last-handoff.json`：最近事务指针。
- 检查保留 GitHub Releases 契约：稳定渠道、版本点分段比较、拒绝降级、同版本不提示、精确资产名/架构/版本/官方 URL 前缀、`digest` 优先、release body checksum 回退、REST 403/429 有界回退。外部托管渠道（Scoop/未知标记）不参与自更新。
- 启动检查为后台注册表一次性任务，首窗与首个会话安顿后延迟联网；先水合缓存与失败证据，再按 `app.updates.auto_check` 决定是否联网。应用页手动检查立即执行。
- 旧 `update_state.json`、旧 `updates/` 不读；首次新版本不继承旧提示抑制。

### 下载与校验

- 下载器不信任 UI 数据：再次校验资产名、版本、官方 URL、大小、摘要格式；无 SHA-256 拒绝下载。
- 流式写入 `.part`；长度、`MZ` 头、SHA-256、512 MiB 上限全部通过后原子替换为安装包；中断或错误响应只留 `.part` 或删除。
- 代次作废链保留：取消或换版本后旧任务进度/完成写入失配；重复点击不叠加任务。
- 自动下载只预取，不授予安装权限；下载状态经应用页与更新提示步消费。

### 安装交接

- 两阶段文件机保留：`plan`/`ready` 表示 helper 就绪，`commit.json` 是唯一安装授权；未 commit 的 `PreparedUpdate` Drop 写 `cancel.json` 并终止 helper；启动时孤儿事务按 cancel 收尾。
- 工作区快照改为目标态：
  ```rust
  struct HandoffWorkspaceV2 {
      schema_version: u32,
      active_project_id: Option<ProjectId>,
      projects: Vec<HandoffProject>,   // id、root、display name、layout
      window: WindowGeometry,
      sidebar: SidebarSnapshot,
  }
  ```
  commit 前先冲刷设置/布局并原子写 `workspace.json`，再写 `commit.json`。
- 安装目录必须是安装器管理的安装（`unins000.exe` 在场）；便携版拒绝自动安装，提示下载页手动替换；提权隔离会话拒绝安装。
- helper 仍由 PowerShell 隐藏窗口启动，物化自 `handoff.ps1`；参与者身份用 PID + 原生创建时间，防 PID 回收冒名；恢复 ticket 版本匹配、尝试次数上限、`restored.json` 防重放。
- 更新交接恢复优先于普通会话恢复：成功或回滚后的首个启动按 ticket 还原工作区；恢复失败留待下次尝试，超限放弃。

### 发布链

- 正式发布只走 `scripts/release.ps1 -Version <X.Y.Z>`：共享构建 → Inno 安装器 → 便携 zip → 钉测试 → `verify-release`；正式路径禁止跳过构建/允许陈旧产物。
- 包内清单唯一真值源为 `build-windows-product.ps1` 的 `PackageEntries`；staging/zip 全等校验与 `installer.iss` 的 `[Files]` 同步。新增/删除包内文件必须三处同改。
- 本次目标态包清单变更：
  - 加入 `assets/runtime-api-v2.schema.json`、`assets/runtime-control-api-v2.md`、更新后的 `assets/skills/slterm-runtime/`。
  - 移除 v1 schema、v1 文档与旧 skill 文案。
  - 保留 app 图标、CLI logo、许可证与第三方 notices。
- 产物命名与更新链资产合同保持：`slTerminal-v<version>-windows-x64-setup.exe`、`...-windows-x64.zip`、`SHA256SUMS`；版本线继续，旧于首个 Inno 安装基的安装只提示手动重装。
- 安装器 AppId 保持不变，保证续装/升级/卸载连续；新状态根不改变安装目录与卸载数据边界。
- 体积预算：release profile 的冷代码优化、LTO、strip 与热路径钉包保留；预算按实测 + 15% 重新标定，钉在 `scripts/tests/package-release.tests.ps1`；收紧合法，放宽须在提交说明原因。
- 合规四落点：仓库 `LICENSE`、workspace/成员 `license` 元数据、安装器 `LicenseFile`、应用内关于页；`licenses/` 与 `THIRD-PARTY-NOTICES` 随包。真实凭据不得进入任何打包输入。
- 发布脚本统一 `#Requires -Version 7`；发布前按 `scripts/release-checklist.md` 人工核验，脚本成功不等于核验完成。
- 安装器卸载钩子清理：新 CLI 需要提供 `setup-ai --remove` 的真实实现，或由安装器移除该调用；不得保留已知 no-op 的卸载步骤。

### 应用页更新入口

- 显示当前版本、检查更新、自动检查/下载开关、下载页、下载进度、安装待重启状态、上次安装失败原因。
- “立即安装并退出”先取工作区快照并冲刷数据，再后台 prepare/commit；成功进入正常退出链，helper 在进程退出后覆盖安装；失败保持窗口并显示错误。
- “下次启动安装”只写预约标记，不启动 helper；下次启动预处理消费预约并退出，由 helper 完成安装。
- 外部托管渠道显示由包管理器更新，禁用自动下载/安装。

### 依赖里程碑

- 持久化协调器与目标态工作区快照。
- 单实例与窗口服务、应用页。
- 通知服务（更新提示与失败通知）。
- 发布链脚本与包清单。

### 迁移顺序

1. 保持旧更新引擎编译但停止从新壳调用；新状态路径与提示状态先落地。
2. 应用页接入手动检查、自动检查/下载开关与下载状态。
3. 工作区快照改为 `HandoffWorkspaceV2`；恢复 ticket 与版本化启动恢复接线。
4. 包清单、schema/docs/skill、体积预算与合规落点更新。
5. 跑假 release 服务器全链彩排与钉测试；再做真实安装器人工核验。
6. 删除旧更新弹窗、旧 `update_state.json` 路径与旧会话快照适配。

### 测试与验收

- 版本比较、稳定渠道过滤、资产精确匹配、digest 规范化和 body 回退、无摘要拒绝、降级拒绝。
- 下载：代理解析、重定向、长度/MZ/SHA-256/上限校验、`.part` 不留可执行文件、取消与换版本代次失配。
- 交接：plan/ready/commit/cancel 状态机、未 commit Drop 取消、孤儿恢复、参与者身份、安装目录归属、restore ticket 尝试与防重放。
- 快照：`HandoffWorkspaceV2` 往返、项目布局与活跃项目恢复、恢复后不抢焦。
- 应用页：检查中/成功/失败/无更新、自动下载开关、安装授权两阶段、外部渠道禁用。
- 发布链：钉测试、清单全等、新鲜度、体积预算、`verify-release` 资产名/PE/SHA256SUMS、假服务器全链彩排。

### 不可自动化面

- 真实 `setup.exe` 覆盖安装、卸载残留、提权隔离与安全软件拦截。
- 更新 helper 在真实进程退出后的安装时序、系统重启/关机路径。
- GitHub Release 网络、限流、CDN 重定向与代理环境。
- 安装向导外观、安装目录权限、干净环境首发与人工发布核验。
- 真实用户数据跨版本升级后的首次启动观感。
