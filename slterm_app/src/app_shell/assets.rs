//! 内嵌资产桥(`gpui::AssetSource` 实现,照上游 assets.rs 裁剪:图标族不迁,
//! 随各域点亮按需回填——本仓图标先查、未命中回落组件库的两段式结构保留在
//! 注释里,回填时恢复)。
//!
//! 内嵌 Maple 字体常驻于此(上游 font_install.rs `REQUIRED_FONT_BYTES` 裁剪形态):
//! 系统等宽字体无 NF 图标码点,内嵌字形同源是跨机器截图/UI 测试可比对的前提。
//! 用户导入字体目录(`imported_font_files`)归设置体系。

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// 内嵌字体的族名(配置默认字体族即取此值)。
pub const REQUIRED_FONT_FAMILY: &str = "Maple Mono Normal NF CN";

/// 内嵌 Maple 字节的唯一静态面:壳内注册统一取这份,防多份字库漂移。
pub static REQUIRED_FONT_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/MapleMonoNormal-NF-CN-Regular.ttf");

/// 组件库资产桥:M3.1 为纯转发;自有图标回填时在此先查本仓、再回落组件库。
pub struct SltermAssets;

impl AssetSource for SltermAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_component_assets::Assets.list(path)
    }
}

#[cfg(test)]
mod assets_tests {
    use super::*;

    #[test]
    fn bundled_font_bytes_are_a_nonempty_truetype_face() {
        // 真型字体魔数(0x00010000)钉住内嵌链路,防路径改写后静默断供。
        assert!(REQUIRED_FONT_BYTES.len() > 1024);
        assert_eq!(&REQUIRED_FONT_BYTES[..4], &[0x00, 0x01, 0x00, 0x00]);
        assert_eq!(REQUIRED_FONT_FAMILY, "Maple Mono Normal NF CN");
    }

    #[test]
    fn asset_bridge_delegates_to_the_component_assets() {
        let source = SltermAssets;
        // 组件库自带图标可达;不存在路径按组件库约定报错(非静默 None)。
        assert!(source.load("icons/arrow-down.svg").unwrap().is_some());
        assert!(source.load("icons/definitely-not-here.svg").is_err());
        assert!(
            source
                .list("icons/")
                .unwrap()
                .iter()
                .any(|name| name.as_ref() == "icons/arrow-down.svg")
        );
    }
}
