//! runtime skill 资产投放(04 篇 M7.4,安装器家族 03 篇形态)。
//!
//! 资产 = `docs/skills/slterm-runtime/SKILL.md` + `agents/openai.yaml`,编译期
//! `include_str!` 内嵌。归属判定沿用安装器家族纪律:
//!
//! - 指纹 = SKILL.md 与 openai.yaml 的 SHA-256 联合摘要;marker 文件
//!   `.slterm-managed` 记录该指纹。
//! - 空目录 / 自有(内容全等或指纹对账)/ 才可写;用户编辑过(指纹 ≠ marker)
//!   或同名未托管目录 → `Conflict`,保留不动,绝不按目录名覆盖。
//! - marker 只在两份内容都原子写完后落下——崩溃不会把半套文件误认成我们所有。

use std::io;
use std::path::{Path, PathBuf};

const RUNTIME_SKILL_MD: &str = include_str!("../../../../docs/skills/slterm-runtime/SKILL.md");
const RUNTIME_SKILL_OPENAI_YAML: &str =
    include_str!("../../../../docs/skills/slterm-runtime/agents/openai.yaml");
/// 目录内 marker:内容 = 联合指纹。
const RUNTIME_SKILL_MARKER: &str = ".slterm-managed";
/// skill 目录名(B-4 改名映射)。
const RUNTIME_SKILL_DIR: &str = "slterm-runtime";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ManagedSkillInstall {
    Installed,
    Current,
    Conflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ManagedSkillRemoval {
    Removed,
    Absent,
    Conflict,
}

/// 两家 AI CLI 的 skill 目录候选:codex → `<home>/.agents/skills`;claude →
/// `<claude 配置目录>/skills`(`CLAUDE_CONFIG_DIR` 可覆盖)。
pub(super) fn runtime_skill_candidates() -> Vec<(&'static str, PathBuf)> {
    let mut targets = Vec::new();
    if let Some(profile) = crate::platform::dirs::home_dir() {
        targets.push((
            "codex",
            profile
                .join(".agents")
                .join("skills")
                .join(RUNTIME_SKILL_DIR),
        ));
    }
    if let Some(claude) = super::claude_config_dir() {
        targets.push(("claude", claude.join("skills").join(RUNTIME_SKILL_DIR)));
    }
    targets
}

/// 自愈入口:只为「已经存在配置目录」的 agent 投放,不为不用它的人造足迹。
/// 逐项返回结果,调用方决定日志/展示;单个失败不阻断另一家。
pub(super) fn ensure_runtime_skills()
-> Vec<(&'static str, PathBuf, io::Result<ManagedSkillInstall>)> {
    runtime_skill_candidates()
        .into_iter()
        .filter(|(agent, _)| match *agent {
            "codex" => super::codex_notify::codex_config_dir().is_some_and(|dir| dir.exists()),
            "claude" => super::claude_config_dir().is_some_and(|dir| dir.exists()),
            _ => false,
        })
        .map(|(agent, path)| {
            let result = ensure_runtime_skill(&path);
            (agent, path, result)
        })
        .collect()
}

/// 两份资产的联合指纹:任一文件变化都会改变归属对账结果。
fn skill_fingerprint(skill: &[u8], metadata: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    use std::fmt::Write as _;

    let mut digest = Sha256::new();
    digest.update(skill);
    digest.update([0]);
    digest.update(metadata);
    let mut fingerprint = String::with_capacity(64);
    for byte in digest.finalize() {
        let _ = write!(&mut fingerprint, "{byte:02x}");
    }
    fingerprint
}

fn read_skill_fingerprint(dir: &Path) -> Option<String> {
    let skill = std::fs::read(dir.join("SKILL.md")).ok()?;
    let metadata = std::fs::read(dir.join("agents").join("openai.yaml")).ok()?;
    Some(skill_fingerprint(&skill, &metadata))
}

fn read_skill_marker(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join(RUNTIME_SKILL_MARKER))
        .ok()
        .map(|value| value.trim().to_owned())
}

fn ensure_runtime_skill(dir: &Path) -> io::Result<ManagedSkillInstall> {
    let skill_path = dir.join("SKILL.md");
    let metadata_path = dir.join("agents").join("openai.yaml");
    let marker_path = dir.join(RUNTIME_SKILL_MARKER);
    let expected = skill_fingerprint(
        RUNTIME_SKILL_MD.as_bytes(),
        RUNTIME_SKILL_OPENAI_YAML.as_bytes(),
    );
    let current = read_skill_fingerprint(dir);
    let marker = read_skill_marker(dir);
    let exact_skill =
        std::fs::read(&skill_path).is_ok_and(|contents| contents == RUNTIME_SKILL_MD.as_bytes());
    let metadata_compatible = !metadata_path.exists()
        || std::fs::read(&metadata_path)
            .is_ok_and(|contents| contents == RUNTIME_SKILL_OPENAI_YAML.as_bytes());
    let empty = !skill_path.exists() && !metadata_path.exists();
    let owned = current
        .as_ref()
        .zip(marker.as_ref())
        .is_some_and(|(actual, marker)| actual == marker);

    if !(empty || owned || (exact_skill && metadata_compatible)) {
        return Ok(ManagedSkillInstall::Conflict);
    }
    if current.as_deref() == Some(expected.as_str())
        && std::fs::read_to_string(&marker_path).is_ok_and(|value| value.trim() == expected)
    {
        return Ok(ManagedSkillInstall::Current);
    }
    crate::atomic_file::write(&skill_path, RUNTIME_SKILL_MD.as_bytes())?;
    crate::atomic_file::write(&metadata_path, RUNTIME_SKILL_OPENAI_YAML.as_bytes())?;
    crate::atomic_file::write(&marker_path, format!("{expected}\n").as_bytes())?;
    Ok(ManagedSkillInstall::Installed)
}

/// 卸载只删除自己所有的那一套;用户编辑过 → `Conflict` 保留原文件。
#[allow(dead_code)] // 消费点 = 设置页卸载/setup-ai verb(03/06 篇后续接线)。
pub(super) fn remove_runtime_skill(dir: &Path) -> io::Result<ManagedSkillRemoval> {
    let Some(marker) = read_skill_marker(dir) else {
        return Ok(ManagedSkillRemoval::Absent);
    };
    if read_skill_fingerprint(dir).as_deref() != Some(marker.as_str()) {
        return Ok(ManagedSkillRemoval::Conflict);
    }
    for path in [
        dir.join("SKILL.md"),
        dir.join("agents").join("openai.yaml"),
        dir.join(RUNTIME_SKILL_MARKER),
    ] {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    let metadata_dir = dir.join("agents");
    if metadata_dir.is_dir() && std::fs::read_dir(&metadata_dir)?.next().is_none() {
        std::fs::remove_dir(metadata_dir)?;
    }
    if dir.is_dir() && std::fs::read_dir(dir)?.next().is_none() {
        std::fs::remove_dir(dir)?;
    }
    Ok(ManagedSkillRemoval::Removed)
}

#[cfg(test)]
mod runtime_skill_tests {
    use super::{
        ManagedSkillInstall, ManagedSkillRemoval, RUNTIME_SKILL_MARKER, RUNTIME_SKILL_MD,
        ensure_runtime_skill, remove_runtime_skill, skill_fingerprint,
    };

    #[test]
    fn managed_skill_installs_idempotently_and_removes_its_own_files() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("slterm-runtime");

        assert_eq!(
            ensure_runtime_skill(&dir).unwrap(),
            ManagedSkillInstall::Installed
        );
        assert_eq!(
            ensure_runtime_skill(&dir).unwrap(),
            ManagedSkillInstall::Current
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("SKILL.md")).unwrap(),
            RUNTIME_SKILL_MD
        );
        assert!(dir.join("agents/openai.yaml").is_file());
        assert!(dir.join(RUNTIME_SKILL_MARKER).is_file());
        assert_eq!(
            remove_runtime_skill(&dir).unwrap(),
            ManagedSkillRemoval::Removed
        );
        assert!(!dir.exists());
    }

    #[test]
    fn managed_skill_never_overwrites_an_unmanaged_same_name() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("slterm-runtime");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), "user-owned\n").unwrap();

        assert_eq!(
            ensure_runtime_skill(&dir).unwrap(),
            ManagedSkillInstall::Conflict
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("SKILL.md")).unwrap(),
            "user-owned\n"
        );
        assert_eq!(
            remove_runtime_skill(&dir).unwrap(),
            ManagedSkillRemoval::Absent
        );
    }

    #[test]
    fn managed_skill_preserves_user_edits_during_update_and_remove() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("slterm-runtime");
        assert_eq!(
            ensure_runtime_skill(&dir).unwrap(),
            ManagedSkillInstall::Installed
        );
        std::fs::write(dir.join("SKILL.md"), "edited after install\n").unwrap();

        assert_eq!(
            ensure_runtime_skill(&dir).unwrap(),
            ManagedSkillInstall::Conflict
        );
        assert_eq!(
            remove_runtime_skill(&dir).unwrap(),
            ManagedSkillRemoval::Conflict
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("SKILL.md")).unwrap(),
            "edited after install\n"
        );
    }

    #[test]
    fn editing_either_asset_changes_the_joint_fingerprint() {
        // 指纹必须覆盖两份资产:只改 metadata 同样不能被视为「Current」。
        let first = skill_fingerprint(b"skill", b"metadata");
        assert_ne!(first, skill_fingerprint(b"skill!", b"metadata"));
        assert_ne!(first, skill_fingerprint(b"skill", b"metadata!"));
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn metadata_edit_reports_conflict_instead_of_overwriting() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("slterm-runtime");
        ensure_runtime_skill(&dir).unwrap();
        std::fs::write(dir.join("agents/openai.yaml"), "user metadata\n").unwrap();
        assert_eq!(
            ensure_runtime_skill(&dir).unwrap(),
            ManagedSkillInstall::Conflict
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("agents/openai.yaml")).unwrap(),
            "user metadata\n"
        );
    }

    #[test]
    fn exact_content_without_marker_is_adopted_and_marked() {
        // 内容全等是归属的第二通道:从旧副本升级/手工拷贝时不需要旧 marker。
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("slterm-runtime");
        std::fs::create_dir_all(dir.join("agents")).unwrap();
        std::fs::write(dir.join("SKILL.md"), RUNTIME_SKILL_MD).unwrap();
        assert_eq!(
            ensure_runtime_skill(&dir).unwrap(),
            ManagedSkillInstall::Installed
        );
        assert!(dir.join(RUNTIME_SKILL_MARKER).is_file());
        assert_eq!(
            ensure_runtime_skill(&dir).unwrap(),
            ManagedSkillInstall::Current
        );
    }

    #[test]
    fn extra_user_files_survive_install_and_removal() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("slterm-runtime");
        ensure_runtime_skill(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), "user notes").unwrap();
        assert_eq!(
            ensure_runtime_skill(&dir).unwrap(),
            ManagedSkillInstall::Current
        );
        assert_eq!(
            remove_runtime_skill(&dir).unwrap(),
            ManagedSkillRemoval::Removed
        );
        // 额外用户文件不是我们所有:目录保留,只删托管资产。
        assert_eq!(
            std::fs::read_to_string(dir.join("notes.txt")).unwrap(),
            "user notes"
        );
        assert!(!dir.join("SKILL.md").exists());
    }
}
