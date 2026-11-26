//! 已解析主题定义的进程内预载缓存。
//!
//! 热应用路径（任何设置字段提交后的全局快照重建）只允许查缓存，不得在
//! GPUI 前台读取或解析主题文件。冷路径（启动、显式主题编辑、库写事件）
//! 负责解析并安装；主题库写成功后主动失效，避免旧定义继续生效。

use std::sync::{Arc, Mutex, OnceLock};

use slterm_settings::ThemeDefinition;

#[derive(Clone)]
struct CachedTheme {
    id: String,
    definition: Arc<ThemeDefinition>,
}

fn cache() -> &'static Mutex<Option<CachedTheme>> {
    static CACHE: OnceLock<Mutex<Option<CachedTheme>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// 安装冷路径解析出的定义；替换旧条目（同一 id 的新 revision 同样覆盖）。
pub(crate) fn install(id: &str, definition: ThemeDefinition) {
    let mut guard = cache().lock().unwrap_or_else(|poison| poison.into_inner());
    *guard = Some(CachedTheme {
        id: id.to_owned(),
        definition: Arc::new(definition),
    });
}

/// 热应用查询：只命中同一身份，未预载时不读盘。
pub(crate) fn get(id: &str) -> Option<Arc<ThemeDefinition>> {
    let guard = cache().lock().unwrap_or_else(|poison| poison.into_inner());
    guard
        .as_ref()
        .filter(|cached| cached.id == id)
        .map(|cached| Arc::clone(&cached.definition))
}

/// 主题库写入或删除后失效；下一次冷路径会重新解析。
pub(crate) fn invalidate() {
    let mut guard = cache().lock().unwrap_or_else(|poison| poison.into_inner());
    *guard = None;
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use slterm_settings::ThemeName;

    #[test]
    fn cache_hits_only_the_installed_identity_and_clears_on_invalidate() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
        let other = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c";
        let definition = ThemeDefinition::from_builtin(ThemeName::Nord);
        install(id, definition.clone());
        assert_eq!(
            get(id).as_deref().map(|cached| &cached.name),
            Some(&definition.name)
        );
        assert!(get(other).is_none(), "不同身份不得命中同一缓存");
        invalidate();
        assert!(get(id).is_none(), "失效后必须重新走冷路径");
    }
}
