//! 本机控制面与单实例移交服务。

#[cfg(test)]
mod boundary_tests;
mod client;
#[cfg(test)]
mod client_tests;
mod credentials;
mod discovery;
#[cfg(test)]
mod event_ring_tests;
mod events;
mod git;
#[cfg(test)]
mod git_tests;
mod mutations;
#[cfg(test)]
mod operation_dispatch_tests;
mod operations;
#[cfg(test)]
mod read_page_tests;
mod reads;
mod server;
#[cfg(test)]
mod server_tests;
mod service;
#[cfg(test)]
mod service_tests;
mod startup;
#[cfg(test)]
mod startup_tests;

#[allow(unused_imports)]
pub(crate) use client::{HandoverError, handover};
#[allow(unused_imports)]
pub(crate) use credentials::rotate_runtime_at;
#[allow(unused_imports)]
pub(crate) use discovery::{
    DiscoveryGuard, discover_json_at, publish, read_discovery, read_discovery_for,
};
#[allow(unused_imports)]
pub(crate) use mutations::{RuntimeMutation, RuntimeMutationRequest};
#[allow(unused_imports)]
pub(crate) use server::LocalControlServer;
#[allow(unused_imports)]
pub(crate) use service::{LocalControlService, RuntimeEventPublisher};
pub(crate) use startup::{InstanceStartup, start_instance};
