# Changelog

本文件随安装器与便携包内置;发布前必须完成对应版本小节。发布说明只写
有构建或真实运行证据的用户可感知变化,中英双语表达同一事实。

## 0.3.0 (未发布 / Unreleased)

### 新增 / Added

- Windows 原生 GPUI 单进程终端:单窗口单实例、仅暗色模式、GPU 加速渲染。
- Runtime API 控制面:`slterm ctl`、`slterm env` 与资源动词,供 AI CLI
  与脚本查询/驱动终端。
- AI CLI 集成:hook 安装、Agent 状态投影与 runtime skill 自动投放。
- GitHub Releases 更新检查:资产四重校验、SHA-256 校验与两阶段安装
  handoff。

### 修复 / Fixed

- 首发小节在 M11.3 文档重写时补全;本文件当前只保证打包链内置版本号
  可核验。
