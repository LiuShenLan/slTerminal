//! 通用 ZIP 解析器分配元数据之前的整包有界预检。
//!
//! 预检只看目录区:条目数、目录大小、Zip64/分卷/注释全部拒绝;随后在
//! 目录检查完成前向 ZIP 读取层隐藏资源字节,防伪造目录记录把元数据
//! 计数骗过上限。
use super::*;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// 元数据阶段隐藏资源字节的 ZIP 读取包装。
pub(super) struct PackageFile {
    file: File,
    directory_start: u64,
    metadata_only: Arc<AtomicBool>,
}

pub(super) fn reader(mut file: File) -> Result<(PackageFile, Arc<AtomicBool>, usize)> {
    let (directory_start, entries) = preflight(&mut file)?;
    let metadata_only = Arc::new(AtomicBool::new(true));
    Ok((
        PackageFile {
            file,
            directory_start,
            metadata_only: metadata_only.clone(),
        },
        metadata_only,
        entries,
    ))
}

impl Read for PackageFile {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.metadata_only.load(Ordering::Relaxed) {
            let position = self.file.stream_position()?;
            if position < self.directory_start {
                // 元数据损坏后 ZIP 读取器可能回退重试更早的结束记录。
                // 本阶段隐藏资源字节,内嵌记录不能替换预检过的目录。
                let count = buffer.len().min((self.directory_start - position) as usize);
                buffer[..count].fill(0);
                self.file.seek(SeekFrom::Current(count as i64))?;
                return Ok(count);
            }
        }
        self.file.read(buffer)
    }
}

impl Seek for PackageFile {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.file.seek(from)
    }
}

fn preflight(file: &mut File) -> Result<(u64, usize)> {
    let size = file.metadata()?.len();
    require(
        (22..=MAX_ARCHIVE_BYTES).contains(&size),
        "theme ZIP must be at most 48 MiB",
    )?;
    let tail_size = size.min(65_557);
    file.seek(SeekFrom::End(-(tail_size as i64)))?;
    let mut tail = vec![0u8; tail_size as usize];
    file.read_exact(&mut tail)?;
    let end = (0..=tail.len() - 22)
        .rev()
        .find(|&index| {
            tail[index..index + 4] == [0x50, 0x4b, 0x05, 0x06]
                && index + 22 + u16::from_le_bytes([tail[index + 20], tail[index + 21]]) as usize
                    == tail.len()
        })
        .ok_or_else(|| PackageError("missing ZIP end record".into()))?;
    let u16_at = |offset| u16::from_le_bytes([tail[end + offset], tail[end + offset + 1]]);
    let u32_at =
        |offset| u32::from_le_bytes(tail[end + offset..end + offset + 4].try_into().unwrap());
    require(
        u16_at(4) == 0 && u16_at(6) == 0 && u16_at(8) == u16_at(10),
        "split ZIP archives are not supported",
    )?;
    require(
        u16_at(10) as usize <= MAX_ENTRIES && u16_at(10) >= 2,
        "ZIP entry count exceeds package limit",
    )?;
    require(u16_at(20) == 0, "ZIP archive comments are not supported")?;
    require(
        u32_at(12) <= 256 * 1024,
        "ZIP directory exceeds metadata limit",
    )?;
    let directory_start = u32_at(16) as u64;
    let directory_size = u32_at(12) as usize;
    require(
        directory_start + directory_size as u64 == size - tail_size + end as u64,
        "ZIP directory must end at the ordinary end record; Zip64 is not supported",
    )?;
    // 先检查有界目录区,再让 ZIP 库接触伪造计数或 Zip64 扩展字段,
    // 防止元数据分配失控。
    file.seek(SeekFrom::Start(directory_start))?;
    let mut directory = vec![0; directory_size];
    file.read_exact(&mut directory)?;
    require(
        !directory.windows(4).any(|bytes| {
            matches!(
                bytes,
                [0x50, 0x4b, 0x05, 0x06] | [0x50, 0x4b, 0x06, 0x06] | [0x50, 0x4b, 0x06, 0x07]
            )
        }),
        "embedded ZIP end records are not supported in directory metadata",
    )?;
    let mut offset = 0usize;
    for _ in 0..u16_at(10) {
        require(offset + 46 <= directory.len(), "truncated ZIP directory")?;
        let header = &directory[offset..offset + 46];
        require(
            header[..4] == [0x50, 0x4b, 0x01, 0x02],
            "invalid ZIP directory entry",
        )?;
        let word = |i| u16::from_le_bytes([header[i], header[i + 1]]) as usize;
        let dword = |i| u32::from_le_bytes(header[i..i + 4].try_into().unwrap());
        require(word(34) == 0, "split ZIP entry")?;
        require(
            [20, 24, 42].iter().all(|&i| dword(i) != u32::MAX),
            "Zip64 entry",
        )?;
        let extra_start = offset + 46 + word(28);
        let extra_end = extra_start + word(30);
        let next = extra_end + word(32);
        require(
            next <= directory.len(),
            "ZIP entry metadata exceeds directory",
        )?;
        let mut extra_offset = extra_start;
        while extra_offset < extra_end {
            require(extra_offset + 4 <= extra_end, "truncated ZIP extra field")?;
            let id = u16::from_le_bytes(
                directory[extra_offset..extra_offset + 2]
                    .try_into()
                    .unwrap(),
            );
            let length = u16::from_le_bytes(
                directory[extra_offset + 2..extra_offset + 4]
                    .try_into()
                    .unwrap(),
            ) as usize;
            require(id != 1, "Zip64 extra field")?;
            extra_offset += 4 + length;
            require(extra_offset <= extra_end, "ZIP extra field exceeds entry")?;
        }
        offset = next;
    }
    require(
        offset == directory.len(),
        "ZIP directory count or trailing data differs",
    )?;
    file.seek(SeekFrom::Start(0))?;
    Ok((directory_start, usize::from(u16_at(10))))
}
