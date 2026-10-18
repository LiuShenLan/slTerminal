//! 合同 workspace 的构建脚本:复编应用侧生成器,产出 translations.rs。

#[path = "../../slterm_app/build/i18n.rs"]
mod i18n;

fn main() {
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let output = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../slterm_app/build/i18n.rs");
    println!("cargo:rerun-if-changed=../../slterm_app/build/i18n/catalog.rs");
    i18n::generate(
        &root.join("../../slterm_app/i18n"),
        &output.join("translations.rs"),
    )
    .unwrap_or_else(|error| panic!("translation catalog validation failed: {error}"));
}
