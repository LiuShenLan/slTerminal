# 项目 scripts 静态发现

## 决策与红线

- **只读声明,绝不执行项目代码**:不调用包管理器、不运行 `package.json`
  的 scripts、不导入 workspace 配置。`must-not-run` 测试用例是这条纪律的
  回归锚。
- **有界读取**:单个 manifest 1MB;向上找最近项目即停(损坏/缺 scripts 不
  借用父项目);workspace 发现 500ms + 4MB + 4096 目录 + 512 包上限。
- **缓存 2s TTL + 代次失效**:与 Git 快照同纪律;旧请求不得把失效前的
  快照回填。

## 步骤接缝

- M9.1 只落来源;设置页/文件树对 workspace 列表的消费归后续篇目。
