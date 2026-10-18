//! 系统 locale 探测:界面语言 System 档的输入面。
//!
//! Windows 走 `GetUserDefaultLocaleName` 一手结果;失败再退 POSIX 环境
//! 变量链(LC_ALL → LC_MESSAGES → LANG),便于测试注入与无 API 环境。
//! 探测与渲染解耦:结果只产出 locale 字符串,语言协商归调用方
//! (`LanguagePref::from_locale`)。

/// Windows locale 名缓冲区长度上限(平台常量,非设置值)。
const LOCALE_NAME_MAX_LENGTH: usize = 85;

/// 当前系统 locale(BCP-47 形态,如 `zh-CN`);探测不出一律 None。
pub(crate) fn system_locale() -> Option<String> {
    windows_locale().or_else(|| locale_from_variables(|name| std::env::var(name).ok()))
}

/// POSIX 环境变量回退链的注入式内核:LC_ALL 最优先,LANG 兜底;
/// 空串视为未设置,测试不触碰真环境。
fn locale_from_variables(mut variable: impl FnMut(&str) -> Option<String>) -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(&mut variable)
        .find(|value| !value.trim().is_empty())
}

/// Windows 一手 locale;API 返回长度 <= 1 视为失败(含空串)。
fn windows_locale() -> Option<String> {
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;

    let mut buffer = [0u16; LOCALE_NAME_MAX_LENGTH];
    // SAFETY: 缓冲区长度与指针一致;API 只写入不超过该长度的 UTF-16。
    let written = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
    let written = usize::try_from(written).ok()?;
    if written <= 1 {
        return None;
    }
    // 返回长度含结尾 NUL。
    Some(String::from_utf16_lossy(&buffer[..written - 1]))
}

#[cfg(test)]
mod locale_tests {
    use super::*;

    #[test]
    fn posix_environment_fallback_prefers_lc_all_then_lc_messages_then_lang() {
        let both = |name: &str| match name {
            "LC_ALL" => Some("zh-CN".to_owned()),
            "LANG" => Some("en-US".to_owned()),
            _ => None,
        };
        assert_eq!(locale_from_variables(both), Some("zh-CN".to_owned()));

        let lang_only = |name: &str| (name == "LANG").then(|| "en-US".to_owned());
        assert_eq!(locale_from_variables(lang_only), Some("en-US".to_owned()));

        // 空串视为未设置 → 继续下一条。
        let empty_lc_all = |name: &str| match name {
            "LC_ALL" => Some(String::new()),
            "LC_MESSAGES" => Some("zh-CN.UTF-8".to_owned()),
            _ => None,
        };
        assert_eq!(
            locale_from_variables(empty_lc_all),
            Some("zh-CN.UTF-8".to_owned())
        );

        assert_eq!(locale_from_variables(|_| None), None);
    }
}
