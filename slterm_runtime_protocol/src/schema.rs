//! 由实际客户端 DTO 生成 Runtime v2 JSON Schema。

/// 当前客户端请求的 Draft 2020-12 schema。
pub fn runtime_api_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::ControlMessage)
}

/// 确定性的美化 JSON，末尾保留一个换行以便逐字节对账。
pub fn runtime_api_schema_json() -> String {
    let mut encoded =
        serde_json::to_string_pretty(&runtime_api_schema()).expect("Runtime schema 必须可序列化");
    encoded.push('\n');
    encoded
}
