//! 由实际客户端 DTO 生成 Runtime v2 JSON Schema。

use serde_json::{Map, Value, json};

use crate::ControlScope;

/// 当前客户端请求的 Draft 2020-12 schema，并附带逐方法 params／result 合同。
pub fn runtime_api_schema() -> Value {
    let mut root = serde_json::to_value(schemars::schema_for!(crate::protocol::ControlMessage))
        .expect("Runtime schema 必须可序列化");
    let mut contracts = Map::new();
    let mut titled_definitions = Vec::new();
    for spec in crate::METHODS {
        let method = spec.method.as_str();
        let params = merge_subschema(
            &mut root,
            serde_json::to_value((spec.params_schema)()).expect("参数 schema 必须可序列化"),
        );
        let result = merge_subschema(
            &mut root,
            serde_json::to_value((spec.result_schema)()).expect("结果 schema 必须可序列化"),
        );
        collect_titled_definitions(&params, &mut titled_definitions);
        collect_titled_definitions(&result, &mut titled_definitions);
        contracts.insert(
            method.to_owned(),
            json!({
                "description": method,
                "type": "object",
                "properties": {
                    "params": params,
                    "result": result,
                },
                "required": ["params", "result"],
                "additionalProperties": false,
            }),
        );
    }
    let required_methods: Vec<String> = contracts.keys().cloned().collect();
    let root_defs = root
        .get_mut("$defs")
        .and_then(Value::as_object_mut)
        .expect("根 schema 必须包含 $defs");
    for (name, definition) in titled_definitions {
        root_defs.entry(name).or_insert(definition);
    }
    root_defs.insert(
        "MethodContracts".to_owned(),
        json!({
            "description": "方法名到参数与结果 schema 的封闭映射；新增方法必须同步注册表。",
            "type": "object",
            "properties": contracts,
            "required": required_methods,
            "additionalProperties": false,
        }),
    );
    if let Some(request) = root_defs
        .get_mut("RuntimeRequest")
        .and_then(Value::as_object_mut)
    {
        if let Some(properties) = request.get_mut("properties").and_then(Value::as_object_mut) {
            // 参数由逐方法 oneOf 约束，通用 RuntimeParams 不再是唯一入口。
            properties.insert("params".to_owned(), json!({}));
        }
        let branches = crate::METHODS
            .iter()
            .filter(|spec| spec.scopes.contains(&ControlScope::Runtime))
            .map(|spec| {
                let method = spec.method.as_str();
                json!({
                    "properties": {
                        "method": {"const": method},
                        "params": {
                            "$ref": format!(
                                "#/$defs/MethodContracts/properties/{method}/properties/params"
                            )
                        }
                    }
                })
            })
            .collect::<Vec<_>>();
        request.insert("oneOf".to_owned(), Value::Array(branches));
    }
    root
}

/// 收集内联子 schema 的类型标题，作为根 `$defs` 的具名定义。
fn collect_titled_definitions(value: &Value, definitions: &mut Vec<(String, Value)>) {
    if let Some(object) = value.as_object() {
        if let Some(title) = object.get("title").and_then(Value::as_str)
            && title != "MutationParams"
        {
            let mut definition = value.clone();
            if let Some(definition) = definition.as_object_mut() {
                definition.remove("$schema");
            }
            definitions.push((title.to_owned(), definition));
        }
        for nested in object.values() {
            collect_titled_definitions(nested, definitions);
        }
    } else if let Some(items) = value.as_array() {
        for nested in items {
            collect_titled_definitions(nested, definitions);
        }
    }
}

/// 把子 schema 的 `$defs` 并入根，避免方法合同内出现无法解析的局部引用。
fn merge_subschema(root: &mut Value, mut schema: Value) -> Value {
    if let Some(defs) = schema.get_mut("$defs").and_then(Value::as_object_mut) {
        let defs = std::mem::take(defs);
        let root_defs = root
            .get_mut("$defs")
            .and_then(Value::as_object_mut)
            .expect("根 schema 必须包含 $defs");
        for (name, definition) in defs {
            root_defs.entry(name).or_insert(definition);
        }
        if let Some(object) = schema.as_object_mut() {
            object.remove("$defs");
        }
    }
    schema
}

/// 确定性的美化 JSON，末尾保留一个换行以便逐字节对账。
pub fn runtime_api_schema_json() -> String {
    let mut encoded =
        serde_json::to_string_pretty(&runtime_api_schema()).expect("Runtime schema 必须可序列化");
    encoded.push('\n');
    encoded
}
