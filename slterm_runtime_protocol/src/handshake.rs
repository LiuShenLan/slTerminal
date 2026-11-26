//! 握手、能力摘要与方法描述 DTO；方法集合只从注册表派生。

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::ControlScope;

/// `runtime.describe` 的单个方法合同：与注册表共用同一事实。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MethodDescriptor {
    pub method: String,
    pub scopes: Vec<ControlScope>,
    pub kind: crate::MethodKind,
    pub idempotent: bool,
}

/// `runtime.describe` 返回方法注册与当前合同散列。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DescribeResponse {
    pub protocol_sha256: String,
    pub schema_sha256: String,
    pub methods: Vec<MethodDescriptor>,
}

impl DescribeResponse {
    /// 只从方法注册表派生；散列由握手同一来源传入。
    pub fn current(protocol_sha256: impl Into<String>, schema_sha256: impl Into<String>) -> Self {
        let methods = crate::METHODS
            .iter()
            .map(|spec| MethodDescriptor {
                method: spec.method.as_str().to_owned(),
                scopes: spec.scopes.to_vec(),
                kind: spec.kind,
                idempotent: spec.idempotent,
            })
            .collect();
        Self {
            protocol_sha256: protocol_sha256.into(),
            schema_sha256: schema_sha256.into(),
            methods,
        }
    }

    /// 按连接作用域过滤 describe；注册表仍是唯一来源。
    pub fn for_scope(
        protocol_sha256: impl Into<String>,
        schema_sha256: impl Into<String>,
        scope: ControlScope,
    ) -> Self {
        let mut response = Self::current(protocol_sha256, schema_sha256);
        response.methods.retain(|method| {
            crate::lookup(&method.method).is_some_and(|spec| spec.scopes.contains(&scope))
        });
        response
    }
}

/// 当前客户端可调用的方法集合。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub methods: BTreeSet<String>,
}

impl Capabilities {
    /// 能力摘要只从方法注册表派生，禁止第二份方法名单。
    pub fn current() -> Self {
        Self {
            methods: crate::method_names().map(str::to_owned).collect(),
        }
    }

    /// 按连接作用域过滤能力摘要；客户端只应看到本连接可调用的方法。
    pub fn for_scope(scope: ControlScope) -> Self {
        Self {
            methods: crate::methods_for(scope)
                .map(|spec| spec.method.as_str().to_owned())
                .collect(),
        }
    }
}

/// hello 握手返回的世代、事件起点与合同散列。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HelloResponse {
    pub store_epoch: String,
    pub stream_epoch: String,
    pub revision: u64,
    pub protocol_sha256: String,
    pub schema_sha256: String,
    pub capabilities: Capabilities,
}

impl HelloResponse {
    /// 按已认证作用域收窄能力摘要；合同散列与世代保持不变。
    pub fn for_scope(mut self, scope: ControlScope) -> Self {
        self.capabilities = Capabilities::for_scope(scope);
        self
    }

    pub fn new(
        store_epoch: impl Into<String>,
        stream_epoch: impl Into<String>,
        revision: u64,
        protocol_sha256: impl Into<String>,
        schema_sha256: impl Into<String>,
    ) -> Self {
        Self {
            store_epoch: store_epoch.into(),
            stream_epoch: stream_epoch.into(),
            revision,
            protocol_sha256: protocol_sha256.into(),
            schema_sha256: schema_sha256.into(),
            capabilities: Capabilities::current(),
        }
    }
}
