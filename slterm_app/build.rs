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
    i18n::generate(
        &Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("i18n"),
        &Path::new(&env::var("OUT_DIR").unwrap()).join("translations.rs"),
    )
    .unwrap_or_else(|error| panic!("translation catalog validation failed: {error}"));
}
