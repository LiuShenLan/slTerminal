//! 隔离记录 ZIP 导出：有界、原子、路径无越界。
//!
//! 归档只含不可变清单原文（路径按 UTF-16 无损）与逐项副本；条目名由
//! 顺序号与安全扩展名生成，不把任意来源名带进 ZIP 路径。

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Component, Path};

use super::persist::{QuarantineBatchRecord, load_batch_material, validate_manifest_name};
use crate::audit::{QuarantineAuditAction, history_quarantine as audit_quarantine};

/// 导出上限：条目数与解压后总字节；超出即零写入拒绝。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExportLimits {
    pub(crate) max_bytes: u64,
    pub(crate) max_entries: usize,
}

pub(crate) const DEFAULT_EXPORT_LIMITS: ExportLimits = ExportLimits {
    max_bytes: 512 * 1024 * 1024,
    max_entries: 10_000,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct QuarantineExportReport {
    pub(crate) entries: usize,
    pub(crate) bytes: u64,
}

/// 导出批次：先完成全部路径与大小预检，再写临时归档并原子替换目标。
pub(crate) fn export_batch(
    root: &Path,
    record: &QuarantineBatchRecord,
    destination: &Path,
    limits: ExportLimits,
) -> io::Result<QuarantineExportReport> {
    let material = load_batch_material(root, record)?;
    validate_manifest_name(&record.manifest_name)?;
    let manifest_path = root.join(&record.manifest_name);
    let manifest_bytes = fs::read(&manifest_path)?;
    let mut total = manifest_bytes.len() as u64;
    let mut items = Vec::with_capacity(material.entries.len());
    for entry in &material.entries {
        validate_quarantine_copy(root, &entry.copy)?;
        let metadata = fs::metadata(&entry.copy)?;
        total = total
            .checked_add(metadata.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::FileTooLarge, "导出体积溢出"))?;
        items.push((
            zip_entry_name(entry.order, &entry.source),
            entry.copy.clone(),
        ));
    }
    if items.len() + 1 > limits.max_entries || total > limits.max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "导出超过有界上限",
        ));
    }
    let report = QuarantineExportReport {
        entries: items.len() + 1,
        bytes: total,
    };
    let result = write_zip(destination, &manifest_bytes, &items);
    let ok = result.is_ok();
    audit_quarantine(QuarantineAuditAction::Export, &manifest_path, ok);
    for (_, path) in &items {
        audit_quarantine(QuarantineAuditAction::Export, path, ok);
    }
    result?;
    Ok(report)
}

/// 副本必须是隔离根的直接普通文件；拒绝 `..`、越界父目录与 reparse 父链。
pub(crate) fn validate_quarantine_copy(root: &Path, copy: &Path) -> io::Result<()> {
    if !root.is_absolute() || !copy.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "隔离路径必须为绝对路径",
        ));
    }
    if copy
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "隔离副本路径不得包含父目录",
        ));
    }
    let root_metadata = fs::symlink_metadata(root)?;
    if !root_metadata.is_dir() || is_reparse_point(&root_metadata) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "隔离根不是普通目录",
        ));
    }
    let parent = copy
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "隔离副本缺少父目录"))?;
    // 副本由隔离函数直接写入隔离根；任何其他词法父路径都属于篡改或别名。
    if parent != root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "隔离副本不在隔离根内",
        ));
    }
    let copy_metadata = fs::symlink_metadata(copy)?;
    if !copy_metadata.is_file() || is_reparse_point(&copy_metadata) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "隔离副本不是普通文件",
        ));
    }
    Ok(())
}

fn is_reparse_point(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

/// ZIP 条目名只由顺序号与白名单扩展名组成，来源名不参与路径。
fn zip_entry_name(order: usize, source: &Path) -> String {
    let extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| {
            !extension.is_empty()
                && extension.len() <= 8
                && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
        .unwrap_or("bin");
    format!("items/{order:04}.{extension}")
}

struct CentralEntry {
    name: String,
    crc: u32,
    size: u32,
    offset: u32,
}

fn write_zip(
    destination: &Path,
    manifest_bytes: &[u8],
    items: &[(String, std::path::PathBuf)],
) -> io::Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "导出目标缺少父目录"))?;
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "导出目标文件名无效"))?;
    let temporary = parent.join(format!(
        ".{file_name}.slterm-quarantine-{}",
        uuid::Uuid::now_v7()
    ));
    let result = write_zip_inner(&temporary, manifest_bytes, items);
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        return result;
    }
    crate::atomic_file::replace(&temporary, destination)
}

fn write_zip_inner(
    temporary: &Path,
    manifest_bytes: &[u8],
    items: &[(String, std::path::PathBuf)],
) -> io::Result<()> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)?;
    let mut writer = BufWriter::new(file);
    let mut central = Vec::with_capacity(items.len() + 1);
    let mut position = 0u64;
    let mut push_entry = |writer: &mut BufWriter<File>,
                          position: &mut u64,
                          name: &str,
                          crc: u32,
                          size: u32,
                          data: Option<&[u8]>,
                          data_path: Option<&Path>|
     -> io::Result<()> {
        let name_bytes = name.as_bytes();
        let offset = u32::try_from(*position)
            .map_err(|_| io::Error::new(io::ErrorKind::FileTooLarge, "ZIP 偏移溢出"))?;
        let mut header = Vec::with_capacity(30 + name_bytes.len());
        header.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        header.extend_from_slice(&20u16.to_le_bytes());
        header.extend_from_slice(&0x0800u16.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(&0x0021u16.to_le_bytes());
        header.extend_from_slice(&crc.to_le_bytes());
        header.extend_from_slice(&size.to_le_bytes());
        header.extend_from_slice(&size.to_le_bytes());
        header.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(name_bytes);
        writer.write_all(&header)?;
        *position += header.len() as u64;
        if let Some(data) = data {
            writer.write_all(data)?;
            *position += data.len() as u64;
        } else if let Some(path) = data_path {
            let mut reader = File::open(path)?;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let read = reader.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                writer.write_all(&buffer[..read])?;
                *position += read as u64;
            }
        }
        central.push(CentralEntry {
            name: name.to_owned(),
            crc,
            size,
            offset,
        });
        Ok(())
    };

    push_entry(
        &mut writer,
        &mut position,
        "manifest.json",
        crc32(manifest_bytes),
        u32::try_from(manifest_bytes.len())
            .map_err(|_| io::Error::new(io::ErrorKind::FileTooLarge, "清单过大"))?,
        Some(manifest_bytes),
        None,
    )?;
    for (name, path) in items {
        let size = fs::metadata(path)?.len();
        let size = u32::try_from(size)
            .map_err(|_| io::Error::new(io::ErrorKind::FileTooLarge, "条目过大"))?;
        let crc = crc32_file(path)?;
        push_entry(
            &mut writer,
            &mut position,
            name,
            crc,
            size,
            None,
            Some(path),
        )?;
    }

    let central_offset = u32::try_from(position)
        .map_err(|_| io::Error::new(io::ErrorKind::FileTooLarge, "ZIP 偏移溢出"))?;
    for entry in &central {
        let name_bytes = entry.name.as_bytes();
        let mut header = Vec::with_capacity(46 + name_bytes.len());
        header.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        header.extend_from_slice(&20u16.to_le_bytes());
        header.extend_from_slice(&20u16.to_le_bytes());
        header.extend_from_slice(&0x0800u16.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(&0x0021u16.to_le_bytes());
        header.extend_from_slice(&entry.crc.to_le_bytes());
        header.extend_from_slice(&entry.size.to_le_bytes());
        header.extend_from_slice(&entry.size.to_le_bytes());
        header.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(&0u32.to_le_bytes());
        header.extend_from_slice(&entry.offset.to_le_bytes());
        header.extend_from_slice(name_bytes);
        writer.write_all(&header)?;
        position += header.len() as u64;
    }
    let central_size = u32::try_from(position - u64::from(central_offset))
        .map_err(|_| io::Error::new(io::ErrorKind::FileTooLarge, "ZIP 中央目录溢出"))?;
    let entries = u16::try_from(central.len())
        .map_err(|_| io::Error::new(io::ErrorKind::FileTooLarge, "ZIP 条目过多"))?;
    let mut eocd = Vec::with_capacity(22);
    eocd.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    eocd.extend_from_slice(&0u16.to_le_bytes());
    eocd.extend_from_slice(&0u16.to_le_bytes());
    eocd.extend_from_slice(&entries.to_le_bytes());
    eocd.extend_from_slice(&entries.to_le_bytes());
    eocd.extend_from_slice(&central_size.to_le_bytes());
    eocd.extend_from_slice(&central_offset.to_le_bytes());
    eocd.extend_from_slice(&0u16.to_le_bytes());
    writer.write_all(&eocd)?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok(())
}

fn crc32_file(path: &Path) -> io::Result<u32> {
    let mut file = File::open(path)?;
    let mut buffer = [0u8; 64 * 1024];
    let mut crc = Crc32::new();
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        crc.update(&buffer[..read]);
    }
    Ok(crc.finish())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = Crc32::new();
    crc.update(bytes);
    crc.finish()
}

struct Crc32 {
    value: u32,
}

impl Crc32 {
    fn new() -> Self {
        Self { value: 0xffff_ffff }
    }

    fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            let index = ((self.value ^ u32::from(*byte)) & 0xff) as usize;
            self.value = (self.value >> 8) ^ CRC_TABLE[index];
        }
    }

    fn finish(self) -> u32 {
        self.value ^ 0xffff_ffff
    }
}

const CRC_TABLE: [u32; 256] = build_crc_table();

const fn build_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut index = 0usize;
    while index < 256 {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 != 0 {
                (value >> 1) ^ 0xedb8_8320
            } else {
                value >> 1
            };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

#[cfg(test)]
mod export_tests {
    use super::*;

    #[test]
    fn crc32_matches_known_vector() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn zip_entry_names_are_order_based_and_safe() {
        assert_eq!(
            zip_entry_name(3, Path::new("C:/history/../evil.jsonl")),
            "items/0003.jsonl"
        );
        assert_eq!(
            zip_entry_name(4, Path::new("C:/history/无扩展")),
            "items/0004.bin"
        );
    }
}
