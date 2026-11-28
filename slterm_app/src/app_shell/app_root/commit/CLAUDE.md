# commit

> Commit 破坏性写确认面：只渲染决定与分派既有写入口。

## 决策与红线

- **默认取消**：确认面显示目标完整路径与丢弃范围，默认动作是取消；无脏 Document 时提供显式执行按钮。原因：回滚与删除不可逆，焦点或回车不得直接落盘。
- **脏正文唯一决定面**：目标 Document 脏时同一面提供保存／丢弃／取消，保存成功回执后才继续原写意图；取消保持磁盘与正文不变。原因：不得复制第二套关闭决定面或静默丢弃正文。边界：Runtime 非交互路径返回结构化 `decision_required`，不在本层伪造回执。
- **渲染无副作用**：本目录只构造确认面元素并分派 AppRoot 方法，不探测 Git、不修改状态；颜色只引用主题语义 token。原因：确认展示不等于授权。

## 验收与测试

- `commit_delete_requires_confirmation_and_cancel_keeps_file`、`commit_untracked_permanent_delete_requires_confirmation` 覆盖完整路径确认与默认取消。
- `commit_rollback_dirty_document_can_continue_without_saving`、`commit_rollback_dirty_document_saves_before_execution` 覆盖脏正文丢弃与保存后继续写。
- `git_write_rejects_dirty_document_without_decision` 覆盖 Runtime 结构化拒绝。
