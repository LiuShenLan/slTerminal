//! GPUI 前的单实例启动分支：首实例持锁，或第二实例完成有界移交。

use std::io;
use std::path::Path;

use slterm_runtime_protocol::{ControlScope, HandoverParams};

use super::{client, credentials, discovery};
use crate::platform::instance::{self, InstanceGuard};
use crate::secrets::{CredentialStore, WindowsCredentialStore};

/// 只有取得 OS 锁才返回首实例所有权；移交成功不携带任何窗口创建凭证。
pub(crate) enum InstanceStartup {
    Primary(InstanceGuard),
    HandedOver,
}

/// 生产启动入口；所有发现、凭据读取与阻塞移交均发生在 GPUI 创建前。
pub(crate) fn start_instance(working_directory: Option<&Path>) -> io::Result<InstanceStartup> {
    let store = WindowsCredentialStore;
    start_with(
        instance::acquire,
        &discovery::endpoint_path(),
        crate::platform::dirs::local_dir(),
        working_directory,
        &store,
    )
}

#[cfg(test)]
pub(super) fn start_instance_at(
    marker: &Path,
    discovery_path: &Path,
    local_root: &Path,
    working_directory: Option<&Path>,
    store: &dyn CredentialStore,
) -> io::Result<InstanceStartup> {
    start_with(
        || instance::acquire_at(marker),
        discovery_path,
        local_root,
        working_directory,
        store,
    )
}

fn start_with(
    acquire: impl FnOnce() -> io::Result<Option<InstanceGuard>>,
    discovery_path: &Path,
    local_root: &Path,
    working_directory: Option<&Path>,
    store: &dyn CredentialStore,
) -> io::Result<InstanceStartup> {
    match acquire() {
        Ok(Some(guard)) => Ok(InstanceStartup::Primary(guard)),
        Ok(None) => handover_existing(discovery_path, local_root, working_directory, store),
        Err(error) => Err(error),
    }
}

fn handover_existing(
    discovery_path: &Path,
    local_root: &Path,
    working_directory: Option<&Path>,
    store: &dyn CredentialStore,
) -> io::Result<InstanceStartup> {
    let params = handover_params(working_directory)?;
    let record = discovery::read_discovery_at(discovery_path)?;
    let credential = credentials::load_credential_at(ControlScope::Handover, local_root, store)?;
    client::handover(&record, credential, params).map_err(io::Error::other)?;
    Ok(InstanceStartup::HandedOver)
}

/// 显式目录只在锁冲突时转为移交意图，不改变首实例的坏目录回退语义。
fn handover_params(working_directory: Option<&Path>) -> io::Result<HandoverParams> {
    let Some(path) = working_directory else {
        return Ok(HandoverParams::Focus);
    };
    if !path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "移交项目目录必须真实存在且为目录",
        ));
    }
    let path = std::path::absolute(path)?;
    let project_root = path.into_os_string().into_string().map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidData, "移交项目目录必须是有效 Unicode")
    })?;
    Ok(HandoverParams::OpenProject { project_root })
}
