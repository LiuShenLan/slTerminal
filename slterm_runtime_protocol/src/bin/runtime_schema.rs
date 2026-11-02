//! 生成并原子替换 Runtime v2 schema 资产。

use std::path::PathBuf;

fn main() {
    let destination = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/runtime-api-v2.schema.json"));
    let temporary = destination.with_extension("schema.tmp");
    std::fs::write(
        &temporary,
        slterm_runtime_protocol::schema::runtime_api_schema_json(),
    )
    .expect("写入临时 Runtime schema 失败");
    if let Err(error) = std::fs::rename(&temporary, &destination) {
        let _ = std::fs::remove_file(&temporary);
        panic!("替换 Runtime schema 资产失败: {error}");
    }
}
