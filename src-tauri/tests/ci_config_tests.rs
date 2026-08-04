//! CI 配置不变量守卫（GIT-11 迁移）
//!
//! 原 `git/mod.rs` 测试区的 `ci_l1_uses_single_test_thread` 属于工程配置
//! 守卫（非 git 域），GIT-11 迁移到独立文件——git 域测试文件只留 git 用例。

/// T3: CI L1 step 必须 --test-threads=1（ConPTY 并发 spawn 死锁防护）。配置不变量守卫。
#[test]
fn ci_l1_uses_single_test_thread() {
    let ci = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.github/workflows/ci.yml"
    ))
    .unwrap();
    assert!(
        ci.contains("--test-threads=1"),
        "CI L1 step 必须 --test-threads=1（ConPTY 并发 spawn 死锁防护）"
    );
}
