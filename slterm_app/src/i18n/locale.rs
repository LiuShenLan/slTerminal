//! system_locale 的 i18n 侧再导出:原生探测留在平台适配层,独立合同
//! workspace 复编同一份实现(不引入应用壳)。

#[path = "../platform/locale.rs"]
mod platform_locale;

pub(crate) use platform_locale::system_locale;
