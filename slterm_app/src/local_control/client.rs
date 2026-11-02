//! 第二实例启动前使用的一次性同步 handover 客户端。

use std::fmt;
use std::io::{self, Read as _, Write as _};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde::de::DeserializeOwned;
use slterm_runtime_protocol::{
    ControlMethod, ControlScope, Credential, DiscoveryRecord, DomainErrorCode, HandoverMethod,
    HandoverParams, HandoverRequest, HandoverResponse, HelloMethod, HelloParams, HelloRequest,
    HelloResponse, JsonRpcError, JsonRpcId, JsonRpcResponse,
};
use zeroize::Zeroizing;

use crate::secrets::SecretBytes;

const CLIENT_ID: &str = "slterm-handover";
const TOTAL_TIMEOUT: Duration = Duration::from_secs(5);
const RESPONSE_BUDGET: usize = 1024 * 1024;
const HELLO_ID: JsonRpcId = JsonRpcId::Number(1);
const HANDOVER_ID: JsonRpcId = JsonRpcId::Number(2);

/// 客户端错误不持有请求行、原始响应、凭据或服务端错误文本。
#[derive(Debug)]
#[allow(dead_code)] // 消费点 = 第二实例启动接线。
pub(crate) enum HandoverError {
    Io(io::Error),
    InvalidDiscovery,
    InvalidCredential,
    InvalidResponse,
    ResponseTooLarge,
    MissingHandoverCapability,
    HandoverRejected,
    Server {
        code: i32,
        domain: Option<DomainErrorCode>,
    },
}

impl fmt::Display for HandoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "控制面连接或传输失败: {error}"),
            Self::InvalidDiscovery => formatter.write_str("控制面发现记录无效"),
            Self::InvalidCredential => formatter.write_str("控制面凭据不是有效 UTF-8"),
            Self::InvalidResponse => formatter.write_str("控制面响应无效"),
            Self::ResponseTooLarge => formatter.write_str("控制面响应超过大小上限"),
            Self::MissingHandoverCapability => {
                formatter.write_str("控制面未声明 instance.handover 能力")
            }
            Self::HandoverRejected => formatter.write_str("控制面未接受移交"),
            Self::Server { code, domain } => {
                write!(
                    formatter,
                    "控制面返回结构化错误: code={code}, domain={domain:?}"
                )
            }
        }
    }
}

impl std::error::Error for HandoverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for HandoverError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// 只连接发现记录指定的 loopback 地址，完成一次 hello 与 handover。
#[allow(dead_code)] // 消费点 = 创建 GPUI 前的第二实例启动接线。
pub(crate) fn handover(
    record: &DiscoveryRecord,
    credential: SecretBytes,
    params: HandoverParams,
) -> Result<(), HandoverError> {
    record
        .validate()
        .map_err(|_| HandoverError::InvalidDiscovery)?;
    let credential = credential
        .into_string()
        .ok_or(HandoverError::InvalidCredential)?;
    let hello = HelloRequest::new(
        HELLO_ID,
        HelloMethod::RuntimeHello,
        HelloParams {
            protocol_version: 2,
            client_id: CLIENT_ID.to_owned(),
            credential: Credential::new(credential.as_str()),
            scope: ControlScope::Handover,
        },
    );
    drop(credential);

    let deadline = Instant::now() + TOTAL_TIMEOUT;
    let mut stream = TcpStream::connect_timeout(&record.endpoint, remaining(deadline)?)?;
    let mut buffer = Vec::new();
    write_json(&mut stream, &hello, deadline)?;
    drop(hello);
    let response: JsonRpcResponse<HelloResponse> =
        read_response(&mut stream, &mut buffer, deadline, &HELLO_ID)?;
    let hello = match response {
        JsonRpcResponse::Result { result, .. } => result,
        JsonRpcResponse::Error { error, .. } => return Err(server_error(error)),
    };
    if !hello
        .capabilities
        .methods
        .contains(ControlMethod::InstanceHandover.as_str())
    {
        return Err(HandoverError::MissingHandoverCapability);
    }

    let request = HandoverRequest::new(HANDOVER_ID, HandoverMethod::InstanceHandover, params);
    write_json(&mut stream, &request, deadline)?;
    let response: JsonRpcResponse<HandoverResponse> =
        read_response(&mut stream, &mut buffer, deadline, &HANDOVER_ID)?;
    match response {
        JsonRpcResponse::Result { result, .. } if result.is_accepted() => Ok(()),
        JsonRpcResponse::Result { .. } => Err(HandoverError::HandoverRejected),
        JsonRpcResponse::Error { error, .. } => Err(server_error(error)),
    }
}

fn server_error(error: JsonRpcError) -> HandoverError {
    // message 可能是恶意回显；只保留数值码与封闭领域码。
    HandoverError::Server {
        code: error.code,
        domain: error.data.map(|data| data.code),
    }
}

fn read_response<T: DeserializeOwned>(
    stream: &mut TcpStream,
    buffer: &mut Vec<u8>,
    deadline: Instant,
    expected_id: &JsonRpcId,
) -> Result<JsonRpcResponse<T>, HandoverError> {
    let response: JsonRpcResponse<T> = read_json(stream, buffer, deadline)?;
    response
        .validate()
        .map_err(|_| HandoverError::InvalidResponse)?;
    let id = match &response {
        JsonRpcResponse::Result { id, .. } | JsonRpcResponse::Error { id, .. } => id,
    };
    if id != expected_id {
        return Err(HandoverError::InvalidResponse);
    }
    Ok(response)
}

fn write_json<T: Serialize>(
    stream: &mut TcpStream,
    value: &T,
    deadline: Instant,
) -> Result<(), HandoverError> {
    let mut encoded = Zeroizing::new(Vec::new());
    serde_json::to_writer(&mut *encoded, value)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "控制面请求编码失败"))?;
    encoded.push(b'\n');
    let mut written = 0;
    while written < encoded.len() {
        stream.set_write_timeout(Some(remaining(deadline)?))?;
        match stream.write(&encoded[written..]) {
            Ok(0) => {
                return Err(io::Error::new(io::ErrorKind::WriteZero, "控制面请求写入失败").into());
            }
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn read_json<T: DeserializeOwned>(
    stream: &mut TcpStream,
    buffer: &mut Vec<u8>,
    deadline: Instant,
) -> Result<T, HandoverError> {
    let frame = read_frame(stream, buffer, deadline)?;
    serde_json::from_slice(&frame).map_err(|_| HandoverError::InvalidResponse)
}

fn read_frame(
    stream: &mut TcpStream,
    buffer: &mut Vec<u8>,
    deadline: Instant,
) -> Result<Vec<u8>, HandoverError> {
    let mut chunk = [0_u8; 1024];
    let mut scanned = 0;
    loop {
        if let Some(newline) = find_newline_since(buffer, scanned) {
            // 服务端预算是换行前的 JSON 编码长度，换行本身占额外一个字节。
            if newline > RESPONSE_BUDGET {
                return Err(HandoverError::ResponseTooLarge);
            }
            return Ok(buffer.drain(..=newline).collect());
        }
        // 换行是单字节分隔符；下一轮只需扫描新追加的后缀。
        scanned = buffer.len();
        if buffer.len() > RESPONSE_BUDGET {
            return Err(HandoverError::ResponseTooLarge);
        }
        stream.set_read_timeout(Some(remaining(deadline)?))?;
        match stream.read(&mut chunk) {
            Ok(0) => {
                return Err(
                    io::Error::new(io::ErrorKind::UnexpectedEof, "控制面响应提前结束").into(),
                );
            }
            Ok(count) => buffer.extend_from_slice(&chunk[..count]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(error.into()),
        }
    }
}

pub(super) fn find_newline_since(buffer: &[u8], scanned: usize) -> Option<usize> {
    buffer
        .get(scanned..)?
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|newline| scanned + newline)
}

fn remaining(deadline: Instant) -> Result<Duration, HandoverError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        Err(io::Error::new(io::ErrorKind::TimedOut, "控制面交互超过总等待上限").into())
    } else {
        Ok(remaining)
    }
}
