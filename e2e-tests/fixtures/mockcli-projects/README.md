# mockcli-projects fixture（CP-041 mockcli 历史会话数据）

E2E（L4，`e2e-tests/mockcli.e2e.ts` 第三 describe「mockcli 历史链路」）使用的
mockcli 历史会话假数据。**每次运行由 `run-wdio.cjs` 复制到
`e2e-tests/.tmp-mockcli-projects/` 副本**（fixture 通道同 claude-projects；
任何用例只动副本，不触碰真实用户目录）。

## 目录结构（编码目录名，禁止反解码）

```
mockcli-projects/
└── C--Users-e2e-fixture-mock/                 # fixture 目录（模拟一个 cwd 编码目录）
    └── 11111111-2222-4333-8444-555555555601.jsonl   # 展示 + 双击恢复目标（summary 首行 + cwd 占位符）
```

- **一级目录名 = cwd 的有损编码**（如 `C:\Users\e2e` → `C--Users-e2e`）。与后端
  `agent_history` 一致：**禁止反解码**——扫描器不做解码，仅按目录遍历。
- **会话文件名 = `<uuidv4>.jsonl`，文件名主干即 sessionId**。
- **占位符 `__E2E_PROJECT_DIR__`**：复制时被 `run-wdio.cjs` 替换为 E2E 临时项目目录
  真实绝对路径（JSON 转义 `\\`），保证 `cwdExists=true` + 归属 E2E 项目（导航树
  历史节点只显示归属会话）。占位符以外的反斜杠保持 `\\` 转义（JSON 合法）。
- mock 会话解析与 claude 同构：jsonl 格式复用 claude 解析助手（summary 首行 →
  标题回退落 summary），无 claude 私有字段语义（custom-title/ai-title 不要求）。

## 用例内 UUID 常量

`mockcli.e2e.ts` 第三 describe 顶部常量与这里逐字对应，新增/删除样本必须同步
修改该文件，否则行数与断言全挂。

## 修改后验证

改 fixture 后跑：

```bash
npm run e2e   # = build:e2e + wdio（重建副本 + 全量 L4）
```

副本 `.tmp-mockcli-projects/` 为构建产物，不入库（.gitignore）。
