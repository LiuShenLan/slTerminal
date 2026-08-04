# claude-projects fixture（E2E 历史会话数据）

E2E（L4，`e2e-tests/history.e2e.ts`）使用的 Claude 历史会话假数据。**每次运行由 `run-wdio.cjs` 复制到 `e2e-tests/.tmp-claude-projects/` 副本**（SEC-02 安全红线：任何用例只动副本，绝不触碰用户真实 `~/.claude/projects/`）。

## 目录结构（编码目录名，禁止反解码）

```
claude-projects/
├── C--Users-e2e-fixture-a/          # fixture 目录 A（模拟一个 cwd 编码目录）
│   ├── 11111111-2222-4333-8444-555555555501.jsonl   # 形态1 custom-title
│   ├── 11111111-2222-4333-8444-555555555502.jsonl   # 形态2 ai-title
│   ├── 11111111-2222-4333-8444-555555555503.jsonl   # 形态3 回退首条 prompt
│   ├── 11111111-2222-4333-8444-555555555504/        # 形态7 subagents 子目录（排除）
│   │   └── subagents/agent-child.jsonl
│   ├── 11111111-2222-4333-8444-555555555507.jsonl   # 恢复编排目标（cwd → __E2E_PROJECT_DIR__ 占位符）
│   ├── agent-misc.jsonl             # 形态6 agent-* 平铺（排除）
│   └── not-a-uuid.jsonl             # 非 UUID 文件名（排除）
└── C--Users-e2e-fixture-b/          # fixture 目录 B
    ├── 11111111-2222-4333-8444-555555555505.jsonl   # 形态4 无 cwd
    └── 11111111-2222-4333-8444-555555555506.jsonl   # 形态5 孤儿（cwd 指向不存在路径）
```

- **一级目录名 = cwd 的有损编码**（如 `C:\Users\e2e` → `C--Users-e2e`）。与后端 `claude_history` 一致：**禁止反解码**——扫描器不做解码，仅按目录遍历。
- **会话文件名 = `<uuidv4>.jsonl`，文件名主干即 sessionId**。

## 与 claude_history 排除规则的同步关系（E2E-13③）

修改本 fixture 前必须先读 `src-tauri/src/claude_history/CLAUDE.md` 的排除规则，二者必须同步：

| 排除规则（scan.rs） | 本 fixture 的对应样本 | 同步约束 |
|---------------------|----------------------|---------|
| `agent-*.jsonl` 平铺形态 | `agent-misc.jsonl` | 新增排除形态样本时，前端 `history.e2e.ts` 用例 1 的「排除文件内容不出现」断言需同步 |
| 文件名主干非 UUID | `not-a-uuid.jsonl` | UUID 形态必须满足 `is_uuid_filename`（36 长度 + 连字符位置 + ascii hex） |
| 不递归子目录（`<id>/subagents/` 天然不命中） | `504/subagents/agent-child.jsonl` | 新样子目录同样放在 UUID 目录下 |
| `cwd_exists`（cwd 非 null 且目录存在） | 505 无 cwd / 506 孤儿 | 孤儿样本的 cwd 必须指向**真实不存在**的路径（占位符替换后仍不存在） |

**占位符 `__E2E_PROJECT_DIR__`**：复制时被 `run-wdio.cjs` 替换为 E2E 临时项目目录真实绝对路径（JSON 转义 `\\`），保证 `cwdExists=true`。新增恢复编排样本时沿用该占位符；样本 JSON 中占位符以外的反斜杠保持 `\\` 转义（JSON 合法）。

**用例内 UUID 常量**：`history.e2e.ts` 顶部的 `UUID_*` 常量与这里逐字对应，新增/删除样本必须同步修改该文件，否则行数与断言全挂。

## 修改后验证

改 fixture 后跑：

```bash
npm run e2e   # = build:e2e + wdio（重建副本 + 全量 L4）
```

副本 `.tmp-claude-projects/` 为构建产物，不入库（.gitignore）。
