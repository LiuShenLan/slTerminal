//! 本机控制面与单实例移交服务。

mod server;
#[cfg(test)]
mod server_tests;

#[allow(unused_imports)]
pub(crate) use server::LocalControlServer;
