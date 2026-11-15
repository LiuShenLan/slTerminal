# Document 崩溃快照

> 自描述正文信封、快照目录与独立写线程的唯一所有者。

## 决策与红线

- **文件权威与配额**：只接收脏的可编辑 Rope；正文使用 UTF-8 自描述信封，严格核对版本、长度、散列、DocumentId、路径、编码、基线与 TextStateId，单正文上限 64 MiB，总正文配额 256 MiB。配额压力只按旧到新驱逐不在打开脏保护集中的快照，不足时不删除任何既有文件；损坏文件保留给启动隔离，不清脏直删。原因：文件名不是正文身份，配额不得牺牲仍打开的编辑现场。边界：快照正文不进入 SQLite，文件经当前用户 ACL 与原子替换写入。
- **删除与写线程**：保存完成或撤销回 saved_state 可删除冗余快照；最后引用关闭必须携带包含该 Document 且已越过 StateCommitted 的关闭意图。`DocumentSnapshotWriter` 使用独立有界队列与 250 毫秒去抖，同 Document 只保留最新待写；flush/shutdown 只在后台等待，失败保留待写集合供原 owner 重试，断连与 Drop 最终冲刷。原因：恢复材料不得早于必要状态提交销毁，前台不得等待磁盘。边界：启动扫描、隔离与有主／无主恢复由真实快照恢复器执行，不在写线程猜测处理。

## 验收与测试

- 真实临时文件覆盖信封损坏、UTF-8 与状态元数据、打开脏保护、配额重试、保存／撤销清脏、关闭提交证明、写失败重试与断连冲刷。
- `snapshot_delete_commit_boundary_crash` 以真实子进程在 SQLite 提交后的删除前后分别崩溃并重开核验。

## 操作

- 定向测试：`cargo test -p slterm --test lib_tests persistence::document_snapshot -- --test-threads=1`。
