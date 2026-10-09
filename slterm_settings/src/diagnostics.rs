//! 配置诊断薄层——serde 校验/诊断公开面的零依赖收缩形态。
//!
//! 上游配置栈(toml/Lua)已整体裁撤,保留的是「反序列化期间收集诊断、
//! 由调用方决定如何呈现」的机制:捕获作用域内 report_* 不直接输出,而是
//! 汇入调用方的诊断向量;作用域外(无捕获)退化为 stderr 一行,保证
//! 「失败可感知」底线。proc-macro derive 不迁,`SerdeReplace` 按需手实现。

use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use crate::json::Value;

/// 未知字段的策略:告警(继续)或拒绝(诊断标记为错误)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownFieldPolicy {
    Warn,
    Deny,
}

/// 诊断类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    UnknownField,
    DeprecatedField,
    InvalidValue,
}

/// 一条配置诊断。`error` 由类别与捕获时的策略共同决定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDiagnostic {
    pub kind: DiagnosticKind,
    pub field: Option<String>,
    pub message: String,
    pub error: bool,
}

impl ConfigDiagnostic {
    pub fn is_error(&self) -> bool {
        self.error
    }
}

struct DiagnosticScope {
    unknown_fields: UnknownFieldPolicy,
    diagnostics: Vec<ConfigDiagnostic>,
}

thread_local! {
    static DIAGNOSTIC_SCOPES: RefCell<Vec<DiagnosticScope>> = const { RefCell::new(Vec::new()) };
}

struct DiagnosticGuard {
    active: bool,
}

impl DiagnosticGuard {
    fn finish(mut self) -> Vec<ConfigDiagnostic> {
        self.active = false;
        DIAGNOSTIC_SCOPES.with(|scopes| {
            scopes
                .borrow_mut()
                .pop()
                .map(|scope| scope.diagnostics)
                .unwrap_or_default()
        })
    }
}

impl Drop for DiagnosticGuard {
    fn drop(&mut self) {
        if self.active {
            DIAGNOSTIC_SCOPES.with(|scopes| {
                scopes.borrow_mut().pop();
            });
        }
    }
}

/// 在捕获作用域内运行 `operation`,返回其结果与期间收集的诊断。
pub fn capture_diagnostics<T>(
    policy: UnknownFieldPolicy,
    operation: impl FnOnce() -> T,
) -> (T, Vec<ConfigDiagnostic>) {
    DIAGNOSTIC_SCOPES.with(|scopes| {
        scopes.borrow_mut().push(DiagnosticScope {
            unknown_fields: policy,
            diagnostics: Vec::new(),
        });
    });
    let guard = DiagnosticGuard { active: true };
    let result = operation();
    let diagnostics = guard.finish();
    (result, diagnostics)
}

fn capture(diagnostic: ConfigDiagnostic) -> bool {
    DIAGNOSTIC_SCOPES.with(|scopes| {
        let mut scopes = scopes.borrow_mut();
        let Some(scope) = scopes.last_mut() else {
            return false;
        };
        scope.diagnostics.push(diagnostic);
        true
    })
}

pub fn report_unknown_field(target: &'static str, field: &str) {
    let message = format!("Unused config key: {field}");
    let error = DIAGNOSTIC_SCOPES.with(|scopes| {
        scopes
            .borrow()
            .last()
            .is_some_and(|scope| scope.unknown_fields == UnknownFieldPolicy::Deny)
    });
    let captured = capture(ConfigDiagnostic {
        kind: DiagnosticKind::UnknownField,
        field: Some(field.to_owned()),
        message: message.clone(),
        error,
    });
    if !captured {
        eprintln!("{target}: {message}");
    }
}

pub fn report_deprecated_field(target: &'static str, field: &str, message: &str) {
    let captured = capture(ConfigDiagnostic {
        kind: DiagnosticKind::DeprecatedField,
        field: Some(field.to_owned()),
        message: message.to_owned(),
        error: false,
    });
    if !captured {
        eprintln!("{target}: {message}");
    }
}

pub fn report_invalid_value(target: &'static str, field: &str, detail: &str) {
    let message = format!("Config error: {field}: {}", detail.trim());
    let captured = capture(ConfigDiagnostic {
        kind: DiagnosticKind::InvalidValue,
        field: Some(field.to_owned()),
        message: message.clone(),
        error: true,
    });
    if !captured {
        eprintln!("{target}: {message}");
    }
}

/// 「替换式更新」合同:字段级就地替换而非整结构覆盖。值层类型不符是
/// 配置错误(Err),与宽容读取层(类型不符回默认)刻意不对称。
pub trait SerdeReplace {
    fn replace(&mut self, value: &Value) -> Result<(), Box<dyn Error>>;
}

/// JSON 值 → 标量的手写转换(proc-macro derive 不迁后的等价物)。
pub trait FromJson: Sized {
    fn from_json(value: &Value) -> Result<Self, Box<dyn Error>>;
}

/// 配置类型错误,载荷为静态说明。
#[derive(Debug)]
pub struct ConfigTypeError(&'static str);

impl fmt::Display for ConfigTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl Error for ConfigTypeError {}

fn type_error(message: &'static str) -> Box<dyn Error> {
    Box::new(ConfigTypeError(message))
}

macro_rules! impl_from_json_integer {
    ($($ty:ty => $name:literal),* $(,)?) => {
        $(
            impl FromJson for $ty {
                fn from_json(value: &Value) -> Result<Self, Box<dyn Error>> {
                    let number = value.as_f64().ok_or_else(|| type_error(concat!("expected number for ", $name)))?;
                    if !number.is_finite() || number.fract() != 0.0
                        || number < <$ty>::MIN as f64 || number > <$ty>::MAX as f64
                    {
                        return Err(type_error(concat!("number out of range for ", $name)));
                    }
                    Ok(number as $ty)
                }
            }
        )*
    };
}

impl_from_json_integer! {
    usize => "usize", u8 => "u8", u16 => "u16", u32 => "u32", u64 => "u64",
    isize => "isize", i8 => "i8", i16 => "i16", i32 => "i32", i64 => "i64",
}

impl FromJson for f32 {
    fn from_json(value: &Value) -> Result<Self, Box<dyn Error>> {
        let number = value
            .as_f64()
            .ok_or_else(|| type_error("expected number for f32"))?;
        if !number.is_finite() {
            return Err(type_error("non-finite number for f32"));
        }
        Ok(number as f32)
    }
}

impl FromJson for f64 {
    fn from_json(value: &Value) -> Result<Self, Box<dyn Error>> {
        let number = value
            .as_f64()
            .ok_or_else(|| type_error("expected number for f64"))?;
        if !number.is_finite() {
            return Err(type_error("non-finite number for f64"));
        }
        Ok(number)
    }
}

impl FromJson for bool {
    fn from_json(value: &Value) -> Result<Self, Box<dyn Error>> {
        value.as_bool().ok_or_else(|| type_error("expected bool"))
    }
}

impl FromJson for char {
    fn from_json(value: &Value) -> Result<Self, Box<dyn Error>> {
        let text = value
            .as_str()
            .ok_or_else(|| type_error("expected single-char string"))?;
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (Some(character), None) => Ok(character),
            _ => Err(type_error("expected single-char string")),
        }
    }
}

impl FromJson for String {
    fn from_json(value: &Value) -> Result<Self, Box<dyn Error>> {
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| type_error("expected string"))
    }
}

impl FromJson for PathBuf {
    fn from_json(value: &Value) -> Result<Self, Box<dyn Error>> {
        value
            .as_str()
            .map(PathBuf::from)
            .ok_or_else(|| type_error("expected path string"))
    }
}

macro_rules! impl_replace {
    ($($ty:ty),* $(,)?) => {
        $(
            impl SerdeReplace for $ty {
                fn replace(&mut self, value: &Value) -> Result<(), Box<dyn Error>> {
                    *self = <$ty as FromJson>::from_json(value)?;
                    Ok(())
                }
            }
        )*
    };
}

#[rustfmt::skip]
impl_replace!(
    usize, u8, u16, u32, u64,
    isize, i8, i16, i32, i64,
    f32, f64,
    bool,
    char,
    String,
    PathBuf,
);

impl<T: FromJson> SerdeReplace for Vec<T> {
    fn replace(&mut self, value: &Value) -> Result<(), Box<dyn Error>> {
        let Value::Array(items) = value else {
            return Err(type_error("expected array"));
        };
        *self = items
            .iter()
            .map(T::from_json)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(())
    }
}

impl<T: SerdeReplace + FromJson> SerdeReplace for Option<T> {
    fn replace(&mut self, value: &Value) -> Result<(), Box<dyn Error>> {
        match self {
            // Some 内层就地替换:结构字段逐个更新,未提及字段保留。
            Some(inner) => inner.replace(value),
            None => {
                *self = Some(T::from_json(value)?);
                Ok(())
            }
        }
    }
}

impl<T: FromJson> SerdeReplace for HashMap<String, T> {
    fn replace(&mut self, value: &Value) -> Result<(), Box<dyn Error>> {
        let Value::Object(members) = value else {
            return Err(type_error("expected object"));
        };
        // 合并语义:替换已有键,追加新键,未提及键保留。
        for (key, item) in members {
            self.insert(key.clone(), T::from_json(item)?);
        }
        Ok(())
    }
}

#[cfg(test)]
mod diagnostics_tests {
    use super::*;

    #[test]
    fn capture_collects_all_kinds_and_marks_policy_errors() {
        let (result, diagnostics) = capture_diagnostics(UnknownFieldPolicy::Deny, || {
            report_unknown_field("test-target", "unknown");
            report_deprecated_field("test-target", "old", "old is deprecated");
            report_invalid_value("test-target", "value", "not a number");
            42
        });
        assert_eq!(result, 42);
        assert_eq!(diagnostics.len(), 3);
        let unknown = &diagnostics[0];
        assert_eq!(unknown.kind, DiagnosticKind::UnknownField);
        assert_eq!(unknown.field.as_deref(), Some("unknown"));
        assert!(unknown.is_error(), "Deny 策略下未知字段是错误");
        assert!(!diagnostics[1].is_error());
        assert!(diagnostics[2].is_error());
        assert_eq!(diagnostics[2].message, "Config error: value: not a number");
    }

    #[test]
    fn warn_policy_keeps_unknown_fields_non_fatal() {
        let (_, diagnostics) = capture_diagnostics(UnknownFieldPolicy::Warn, || {
            report_unknown_field("test-target", "stray");
        });
        assert!(!diagnostics[0].is_error());
    }

    #[test]
    fn nested_captures_are_isolated_scopes() {
        let (_, outer) = capture_diagnostics(UnknownFieldPolicy::Warn, || {
            report_unknown_field("outer", "a");
            let (_, inner) = capture_diagnostics(UnknownFieldPolicy::Deny, || {
                report_unknown_field("inner", "b");
            });
            assert_eq!(inner.len(), 1);
            assert!(inner[0].is_error());
            assert_eq!(inner[0].field.as_deref(), Some("b"));
        });
        assert_eq!(outer.len(), 1);
        assert_eq!(outer[0].field.as_deref(), Some("a"));
    }

    #[test]
    fn reporting_without_a_capture_scope_falls_back_to_stderr_without_panicking() {
        // 无捕获作用域:不得 panic,不得污染其他线程的诊断栈。
        report_unknown_field("stderr", "loose");
        report_deprecated_field("stderr", "loose", "message");
        report_invalid_value("stderr", "loose", "detail");
    }

    #[test]
    fn serde_replace_updates_scalars_and_rejects_type_mismatches() {
        let mut number: u32 = 1;
        number.replace(&Value::Number(7.0)).unwrap();
        assert_eq!(number, 7);
        assert!(number.replace(&Value::String("x".into())).is_err());
        assert!(number.replace(&Value::Number(-1.0)).is_err());
        assert!(number.replace(&Value::Number(1.5)).is_err());

        let mut flag = false;
        flag.replace(&Value::Bool(true)).unwrap();
        assert!(flag);
        assert!(
            flag.replace(&Value::Number(1.0)).is_err(),
            "布尔不兼容数字拼写"
        );

        let mut text = String::from("old");
        text.replace(&Value::String("new".into())).unwrap();
        assert_eq!(text, "new");

        let mut path = PathBuf::new();
        path.replace(&Value::String("C:/tmp".into())).unwrap();
        assert_eq!(path, PathBuf::from("C:/tmp"));
    }

    #[test]
    fn serde_replace_merges_containers_like_the_upstream_contract() {
        // Vec 整组替换。
        let mut list = vec![1u32, 2];
        list.replace(&Value::Array(vec![Value::Number(9.0)]))
            .unwrap();
        assert_eq!(list, vec![9]);

        // Option<Some> 就地替换内层,None 整体构造(显式限定:Option 固有
        // 的 replace 会遮蔽 trait 方法)。
        let mut maybe: Option<String> = Some("keep".into());
        SerdeReplace::replace(&mut maybe, &Value::String("replaced".into())).unwrap();
        assert_eq!(maybe.as_deref(), Some("replaced"));
        let mut empty: Option<String> = None;
        SerdeReplace::replace(&mut empty, &Value::String("built".into())).unwrap();
        assert_eq!(empty.as_deref(), Some("built"));

        // HashMap 合并:替换已有、追加新键、未提及键保留。
        let mut map = HashMap::from([("a".to_owned(), 1u32), ("b".to_owned(), 2u32)]);
        map.replace(&Value::Object(vec![
            ("b".to_owned(), Value::Number(20.0)),
            ("c".to_owned(), Value::Number(30.0)),
        ]))
        .unwrap();
        assert_eq!(map["a"], 1);
        assert_eq!(map["b"], 20);
        assert_eq!(map["c"], 30);
    }
}
