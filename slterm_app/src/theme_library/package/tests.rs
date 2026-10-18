//! 主题包有界检查用例:版本化 Manifest / 整包边界 / 不解可信以外字段。

use std::io::Write as _;

use super::*;
use crate::theme_library::{ThemeDocument, from_definition};
use slterm_settings::{ThemeDefinition, ThemeName};

/// 1×1 透明 PNG(真实签名,供镜像签名校验通过)。
const PNG_BYTES: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

fn author() -> Author {
    Author {
        name: "slTerminal".to_owned(),
        github: None,
    }
}

fn document_with(background: Option<&str>) -> ThemeDocument {
    let mut definition = ThemeDefinition::from_builtin(ThemeName::Nord);
    definition.name = "Packaged".to_owned();
    definition.effects.background_image = background.map(str::to_owned);
    from_definition(&definition).unwrap()
}

fn write_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
}

#[test]
fn manifest_is_version_locked_and_rejects_unknown_fields() {
    let manifest = Manifest {
        package_version: 2,
        name: "Theme".into(),
        author: author(),
        version: "1.0.0".into(),
        license: "MIT".into(),
        theme: "theme.slterm-theme.json".into(),
        resources: Vec::new(),
    };
    assert!(manifest.validate().is_err(), "版本号不匹配必须拒收");

    let valid = Manifest {
        package_version: 1,
        ..manifest
    };
    assert!(valid.validate().is_ok());

    let json = serde_json::json!({
        "package_version": 1,
        "name": "Theme",
        "author": {"name": "slTerminal"},
        "version": "1.0.0",
        "license": "MIT",
        "theme": "theme.slterm-theme.json",
        "resources": [],
        "unexpected": true,
    });
    let parsed: std::result::Result<Manifest, _> = serde_json::from_value(json);
    assert!(parsed.is_err(), "未知 Manifest 字段不得被静默接受");
}

#[test]
fn exported_package_round_trips_through_the_bounded_checker() {
    let directory = tempfile::tempdir().unwrap();
    let image_path = directory.path().join("background.png");
    std::fs::write(&image_path, PNG_BYTES).unwrap();
    let output = directory.path().join("theme.zip");
    let document = document_with(Some(image_path.to_str().unwrap()));
    export_package(
        &document,
        author(),
        "1.0.0".into(),
        "MIT".into(),
        &output,
        None,
    )
    .unwrap();
    assert!(output.exists());

    let checked = CheckedPackage::open(&output).unwrap();
    assert_eq!(checked.manifest.package_version, 1);
    assert_eq!(checked.manifest.name, "Packaged");
    assert_eq!(checked.manifest.resources.len(), 1);
    let definition = checked.document.definition().unwrap();
    assert_eq!(
        definition.effects.background_image.as_deref(),
        Some("assets/background.png")
    );
}

#[test]
fn undeclared_entries_and_bad_hashes_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let document = document_with(None);
    let manifest = Manifest {
        package_version: 1,
        name: document.name().to_owned(),
        author: author(),
        version: "1.0.0".into(),
        license: "MIT".into(),
        theme: "theme.slterm-theme.json".into(),
        resources: Vec::new(),
    };
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    let theme_bytes = document.to_json_bytes().unwrap();
    let path = directory.path().join("undeclared.zip");
    write_zip(
        &path,
        &[
            ("manifest.json", &manifest_bytes),
            ("theme.slterm-theme.json", &theme_bytes),
            ("assets/extra.png", PNG_BYTES),
        ],
    );
    assert!(CheckedPackage::open(&path).is_err(), "未声明条目必须拒收");

    // 声明了资源但清单哈希与真实字节不符。
    let bad_manifest = Manifest {
        resources: vec![Resource {
            path: "assets/background.png".to_owned(),
            kind: ResourceKind::Image,
            bytes: PNG_BYTES.len() as u64,
            sha256: "0".repeat(64),
        }],
        ..manifest
    };
    let bad_manifest_bytes = serde_json::to_vec(&bad_manifest).unwrap();
    let path = directory.path().join("bad-hash.zip");
    write_zip(
        &path,
        &[
            ("manifest.json", &bad_manifest_bytes),
            ("theme.slterm-theme.json", &theme_bytes),
            ("assets/background.png", PNG_BYTES),
        ],
    );
    assert!(CheckedPackage::open(&path).is_err(), "哈希不符必须拒收");
}

#[test]
fn archives_outside_the_size_and_entry_bounds_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    // 下限不足 22 字节:不是合法 ZIP 结束记录。
    let path = directory.path().join("tiny.zip");
    std::fs::write(&path, [0u8; 8]).unwrap();
    assert!(CheckedPackage::open(&path).is_err());

    // 条目数超过包上限。
    let path = directory.path().join("too-many.zip");
    let document = document_with(None);
    let manifest = Manifest {
        package_version: 1,
        name: document.name().to_owned(),
        author: author(),
        version: "1.0.0".into(),
        license: "MIT".into(),
        theme: "theme.slterm-theme.json".into(),
        resources: Vec::new(),
    };
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    let theme_bytes = document.to_json_bytes().unwrap();
    let mut entries: Vec<(String, Vec<u8>)> = vec![
        ("manifest.json".to_owned(), manifest_bytes),
        ("theme.slterm-theme.json".to_owned(), theme_bytes),
    ];
    for index in 0..MAX_ENTRIES {
        entries.push((format!("assets/{index}.png"), PNG_BYTES.to_vec()));
    }
    let borrowed: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    write_zip(&path, &borrowed);
    assert!(
        CheckedPackage::open(&path).is_err(),
        "条目数超过上限必须拒收"
    );
}
