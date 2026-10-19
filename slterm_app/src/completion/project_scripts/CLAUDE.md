# 项目 scripts 静态发现

> 从项目 manifest 与 workspace 配置静态发现脚本来源；只读、有界、绝不执行项目代码。

## 决策与红线

- **只读声明，绝不执行项目代码**：不调用包管理器、不运行 `package.json` 的 scripts、不导入 workspace 配置。`must-not-run` 测试用例是这条纪律的回归锚。
- **有界读取**：单个 manifest 1 MiB；向上找最近项目即停（损坏/缺 scripts 不借用父项目）；workspace 发现 500ms + 4 MiB + 4096 目录 + 512 包上限。原因：不可信项目目录不得拖垮补全或启动。边界：超限即降级为空结果，不猜、不重试。
- **缓存 2 秒 TTL + 代次失效**：与 Git 快照同纪律；旧请求不得把失效前的快照回填。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 必测面：`must-not-run`、单文件上限、最近项目停止规则、workspace 预算、TTL 与代次失效。

## 操作

- 新增 manifest 形态：只做静态读取；同步补有界与不执行测试。
- 调整预算：属契约数值变更，须评审并补超限降级用例。
