//! 构建脚本:i18n 静态生成(06 篇)与版本串注入。

use std::env;
use std::path::Path;

#[path = "build/i18n.rs"]
mod i18n;

fn main() {
    // 生成器自身也是输入:模块文件变更同样触发重跑。
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build/i18n.rs");
    println!("cargo:rerun-if-changed=build/i18n/catalog.rs");
    // 翻译目录是生成器的直接输入:漏掉本行会让新增键在增量构建里留下
    // 不存在的 Message 变体(必须 clean 才可见)。
    println!("cargo:rerun-if-changed=i18n");
    i18n::generate(
        &Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("i18n"),
        &Path::new(&env::var("OUT_DIR").unwrap()).join("translations.rs"),
    )
    .unwrap_or_else(|error| panic!("translation catalog validation failed: {error}"));
}
