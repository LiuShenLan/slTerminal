//! 本机控制面与单实例移交服务。

mod client;
#[cfg(test)]
mod client_tests;
mod credentials;
mod discovery;
mod server;
#[cfg(test)]
mod server_tests;

#[allow(unused_imports)]
pub(crate) use client::{HandoverError, handover};
#[allow(unused_imports)]
pub(crate) use discovery::{DiscoveryGuard, publish, read_discovery};
#[allow(unused_imports)]
pub(crate) use server::LocalControlServer;
