# 验证策略

## 工具矩阵

| 工具 | 适用场景 | 限制 |
|------|---------|------|
| WebSearch | 初始搜索、发现来源 URL | 摘要可能过时/不完整 |
| WebFetch | 读取公开文档页面全文 | 无法处理需要登录/JS 渲染的页面 |
| chrome-devtools MCP | 直接访问 GitHub issue/npm registry/需要 JS 渲染的页面 | 需要浏览器实例 |
| Context7 MCP | 查询特定库/框架的官方文档 | 仅限已索引的库 |

## 验证方法

### 方法 1: 交叉引用验证

对两条来自不同方向的相同声称 → 比对来源 URL 和日期。

```
if (两个方向的声称一致 && 来源不同) → 高置信度
if (两个方向的声称一致 && 来自同一来源) → 中等置信度
if (两个方向的声称矛盾) → 冲突，需裁决
```

### 方法 2: 直接页面访问

当 WebSearch 片段不足时，用 chrome-devtools 直接导航到来源页面：

```typescript
// 流程（确定性）
function directVerify(url: string, claim: string): VerificationResult {
  // 1. navigate_page → url
  // 2. take_snapshot → 获取页面文本
  // 3. 在页面文本中搜索 claim 中的关键词
  // 4. 找到 → 确认；找不到 → 标记为"不可验证"
}
```

**必须用 chrome-devtools 的场景**:
- GitHub issue 状态（Open/Closed/标签）
- npm registry 包版本号
- GitLab MR 状态（merged/closed/open）
- KDE Bugzilla/其他 bug tracker 的状态
- 未被 WebFetch 正确渲染的页面（大量 JS）

### 方法 3: 冲突裁决（ADJUDICATION）

当多个来源矛盾时，生成裁决文件。

**裁决优先级（确定性规则）**:
1. 官方文档 > 社区总结
2. 源代码 > 博客文章
3. 较新的 > 较旧的
4. 有直接引用 > 无引用

**裁决文件格式** (`ADJUDICATION.md`)：
```markdown
### 冲突 N: {字段名}
| 方向 | 声称 |
|------|------|
| D1 §X | ... |
| D2 §Y | ... |

**裁决**: {结论}。{理由}。
```

## 分阶段验证

| 阶段 | 验证范围 | 方法 | 触发条件 |
|------|---------|------|---------|
| Round 1 | 所有源文件 | WebSearch + WebFetch 交叉验证 | 检索完成后 |
| Round 2 | Round 1 无法验证的条目 | chrome-devtools 直接访问 | Round 1 有未验证项 |
| Round N | 前 N-1 轮修改引入的新错误 | 全文抽查 | 有修改 |

## 错误数阈值

```
错误总数 ≥ 10 → spawn 5 个并行验证子代理（每方向 1 个）
错误总数 < 10 → AI 直接验证并修正
```
