//! release 资产精确匹配合同(改造节 7)。
//!
//! 名称/架构/版本/官方 URL 前缀四重校验的「名称」半边:发现侧只认
//! `slTerminal-v{version}-windows-x64-setup.exe` 单一合同;架构维度已
//! 固化在名称里,非 x64 构建不提供任何资产。下载侧还会再次校验 URL、
//! 大小与 SHA-256(下载器不信任 UI 数据)。

use super::{
    GitHubReleaseAsset, RELEASE_DOWNLOAD_PREFIX, UpdateAsset, checksum_from_release_body,
    normalize_sha256,
};

/// 当前架构可下载的官方资产名。slTerminal 只发布 Windows x64 安装器。
pub(crate) fn native_names(version: &str) -> Vec<String> {
    if std::env::consts::ARCH == "x86_64" {
        super::windows_x64_installer_names(version).to_vec()
    } else {
        Vec::new()
    }
}

/// 在 release 资产清单中按名称优先级选第一个精确命中项。
///
/// digest 字段优先;缺失时回退 release body 内嵌 checksum;两者都没有时
/// 仍返回资产(sha256 = None),由下载侧拒绝无摘要安装包。
pub(super) fn select(
    version: &str,
    body: &str,
    assets: &[GitHubReleaseAsset],
    names: &[String],
) -> Option<UpdateAsset> {
    let asset = names
        .iter()
        .find_map(|name| assets.iter().find(|asset| asset.name == *name))?;
    Some(UpdateAsset {
        version: version.into(),
        name: asset.name.clone(),
        url: asset.browser_download_url.clone(),
        size: (asset.size > 0).then_some(asset.size),
        digest: asset
            .digest
            .as_deref()
            .and_then(normalize_sha256)
            .or_else(|| checksum_from_release_body(body, &asset.name)),
    })
}

/// `SHA256SUMS` 资产解析:同一条目必须唯一且格式合法。
/// REST 限流回退时用它恢复受验证的下载元数据(无条目 = 只能手动下载)。
pub(super) fn from_checksums(version: &str, text: &str) -> Option<UpdateAsset> {
    native_names(version).iter().find_map(|name| {
        let hashes: Vec<_> = text
            .lines()
            .filter_map(|line| {
                let (hash, file) = line.split_once(char::is_whitespace)?;
                (file.trim().trim_start_matches('*') == name)
                    .then(|| normalize_sha256(&format!("sha256:{hash}")))
                    .flatten()
            })
            .collect();
        if hashes.len() != 1 {
            return None;
        }
        Some(UpdateAsset {
            version: version.into(),
            name: name.clone(),
            url: format!("{RELEASE_DOWNLOAD_PREFIX}v{version}/{name}"),
            size: None,
            digest: hashes.into_iter().next(),
        })
    })
}

#[cfg(test)]
mod assets_tests {
    use super::*;

    #[test]
    fn windows_architecture_is_exact_and_unique() {
        let version = "1.9.2";
        let x64 = native_names(version);
        if std::env::consts::ARCH == "x86_64" {
            assert_eq!(x64, ["slTerminal-v1.9.2-windows-x64-setup.exe"]);
        } else {
            assert!(x64.is_empty());
            return;
        }
        let other = "slTerminal-v1.9.2-windows-arm64-setup.exe".to_owned();
        let assets = [x64[0].clone(), other]
            .into_iter()
            .map(|name| GitHubReleaseAsset {
                name,
                browser_download_url: format!("{RELEASE_DOWNLOAD_PREFIX}v{version}/asset"),
                size: 42,
                digest: Some(format!("sha256:{}", "a".repeat(64))),
            })
            .collect::<Vec<_>>();
        assert_eq!(select(version, "", &assets, &x64).unwrap().name, x64[0]);
        assert!(select(version, "", &assets[..1], &["unknown".to_owned()]).is_none());
    }
}
