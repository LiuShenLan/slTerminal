//! 固定来源套餐查询客户端：专用凭据、固定 HTTPS 路径、有界响应。
//!
//! 请求组装是唯一接触 token 的代码；错误与结果都不携带认证材料。

use std::io::Read as _;
use std::time::Duration;

use super::{PlanBalanceCredentials, PlanBalanceInfo, PlanBalanceSource};

/// 响应正文上限；与设计中的 1 MiB 预算一致。
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
/// 请求总超时；不因慢响应无限占用后台执行体。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

/// 单来源查询失败；分类不携带 token、路径或响应正文。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlanBalanceError {
    /// 专用凭据目标缺失或不可用。
    MissingCredential,
    /// 凭据后端读取失败。
    Credential,
    /// 连接、写入或读取失败。
    Request,
    /// 非 200 状态；不跟随重定向。
    Status(u16),
    /// 响应超过预算。
    ResponseTooLarge,
    /// 响应不是合法 JSON 或字段不符合来源语义。
    Parse,
}

impl PlanBalanceSource {
    /// 固定 HTTPS 端点：每来源唯一，不接受设置或 CLI 覆盖。
    pub(crate) const fn endpoint(self) -> &'static str {
        match self {
            Self::DeepSeek => "https://api.deepseek.com/user/balance",
            Self::Kimi => "https://api.kimi.com/coding/v1/usages",
        }
    }
}

/// 固定来源查询客户端：凭据域读取 token，一次请求，零重定向。
pub(crate) struct PlanBalanceClient {
    credentials: PlanBalanceCredentials,
    agent: ureq::Agent,
    /// 测试注入的回环端点；生产构建不存在该字段。
    #[cfg(test)]
    endpoint_override: Option<String>,
}

impl PlanBalanceClient {
    /// 生产构造：8 秒总超时、零重定向、不随请求改写。
    pub(crate) fn new(credentials: PlanBalanceCredentials) -> Self {
        let agent = ureq::config::Config::builder()
            .max_redirects(0)
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build()
            .new_agent();
        Self {
            credentials,
            agent,
            #[cfg(test)]
            endpoint_override: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        credentials: PlanBalanceCredentials,
        endpoint: &str,
        timeout: Duration,
    ) -> Self {
        let agent = ureq::config::Config::builder()
            .proxy(None)
            .max_redirects(0)
            .timeout_global(Some(timeout))
            .build()
            .new_agent();
        Self {
            credentials,
            agent,
            endpoint_override: Some(endpoint.to_owned()),
        }
    }

    fn endpoint(&self, source: PlanBalanceSource) -> String {
        #[cfg(test)]
        if let Some(endpoint) = &self.endpoint_override {
            return endpoint.clone();
        }
        source.endpoint().to_owned()
    }

    /// 查询单来源套餐；token 只在请求头组装点使用。
    pub(crate) fn fetch(
        &self,
        source: PlanBalanceSource,
    ) -> Result<PlanBalanceInfo, PlanBalanceError> {
        let secret = self
            .credentials
            .load(source)
            .map_err(|_| PlanBalanceError::Credential)?;
        let Some(secret) = secret else {
            return Err(PlanBalanceError::MissingCredential);
        };
        let token = secret
            .into_string()
            .ok_or(PlanBalanceError::MissingCredential)?;
        let mut response = self
            .agent
            .get(&self.endpoint(source))
            .header("Authorization", &format!("Bearer {}", token.as_str()))
            .call()
            .map_err(|error| match error {
                ureq::Error::StatusCode(status) => PlanBalanceError::Status(status),
                _ => PlanBalanceError::Request,
            })?;
        if response.status() != 200 {
            return Err(PlanBalanceError::Status(response.status().as_u16()));
        }
        let mut body = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(MAX_RESPONSE_BYTES as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|_| PlanBalanceError::Request)?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(PlanBalanceError::ResponseTooLarge);
        }
        let value: serde_json::Value =
            serde_json::from_slice(&body).map_err(|_| PlanBalanceError::Parse)?;
        match source {
            PlanBalanceSource::DeepSeek => super::deepseek::parse_deepseek_balance(&value),
            PlanBalanceSource::Kimi => super::kimi::parse_kimi_usages(&value),
        }
        .map_err(|_| PlanBalanceError::Parse)
    }
}

#[cfg(test)]
mod client_tests {
    use std::io::{BufRead as _, BufReader, Write as _};
    use std::net::{SocketAddr, TcpListener};
    use std::sync::Arc;
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    use super::*;
    use crate::plan_balance::PlanBalanceSource;
    use crate::secrets::{CredentialStore, SecretBytes};

    #[derive(Default)]
    struct RecordingStore {
        token: Option<Vec<u8>>,
    }

    impl CredentialStore for RecordingStore {
        fn load(&self, _target: &str) -> std::io::Result<Option<SecretBytes>> {
            Ok(self.token.clone().map(SecretBytes::new))
        }

        fn store(&self, _target: &str, _secret: &[u8]) -> std::io::Result<()> {
            Ok(())
        }

        fn delete(&self, _target: &str) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct HttpFixture {
        address: SocketAddr,
        worker: JoinHandle<Vec<String>>,
    }

    impl HttpFixture {
        fn start(responses: Vec<Vec<u8>>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let worker = thread::spawn(move || {
                let mut requests = Vec::new();
                'responses: for response in responses {
                    let deadline = Instant::now() + Duration::from_millis(500);
                    let stream = loop {
                        match listener.accept() {
                            Ok((stream, _)) => break stream,
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                if Instant::now() >= deadline {
                                    break 'responses;
                                }
                                thread::sleep(Duration::from_millis(2));
                            }
                            Err(error) => panic!("HTTP fixture accept failed: {error}"),
                        }
                    };
                    stream.set_nonblocking(false).unwrap();
                    let mut reader = BufReader::new(stream);
                    let mut request = String::new();
                    loop {
                        let mut line = String::new();
                        reader.read_line(&mut line).unwrap();
                        if line == "\r\n" {
                            break;
                        }
                        request.push_str(&line);
                    }
                    requests.push(request);
                    reader.get_mut().write_all(&response).unwrap();
                }
                requests
            });
            Self { address, worker }
        }

        fn url(&self, path: &str) -> String {
            format!("http://{}{path}", self.address)
        }

        fn finish(self) -> Vec<String> {
            self.worker.join().unwrap()
        }
    }

    fn response(status: &str, headers: &str, body: &[u8]) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        response
    }

    fn client(store: Arc<dyn CredentialStore>, endpoint: &str) -> PlanBalanceClient {
        PlanBalanceClient::for_test(
            crate::plan_balance::PlanBalanceCredentials::new(store),
            endpoint,
            Duration::from_secs(2),
        )
    }

    /// 生产端点固定为设计中的 HTTPS 路径。
    #[test]
    fn plan_endpoints_are_fixed_https_paths() {
        assert_eq!(
            PlanBalanceSource::DeepSeek.endpoint(),
            "https://api.deepseek.com/user/balance"
        );
        assert_eq!(
            PlanBalanceSource::Kimi.endpoint(),
            "https://api.kimi.com/coding/v1/usages"
        );
    }

    /// 请求只携带专用凭据目标的 token，并解析固定来源响应。
    #[test]
    fn plan_request_uses_dedicated_credential_and_parses_response() {
        let fixture = HttpFixture::start(vec![response(
            "200 OK",
            "Content-Type: application/json\r\n",
            br#"{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"12.34"}]}"#,
        )]);
        let store = Arc::new(RecordingStore {
            token: Some(b"sk-test-plan-client".to_vec()),
        });
        let client = client(store, &fixture.url("/user/balance"));

        let info = client
            .fetch(PlanBalanceSource::DeepSeek)
            .expect("必须解析成功");
        assert!(!info.frozen);
        assert_eq!(info.amount.expect("金额必须解析").value, "12.34");

        let requests = fixture.finish();
        assert_eq!(requests.len(), 1, "只允许一次请求");
        assert!(
            requests[0].starts_with("GET /user/balance HTTP/1.1"),
            "固定路径: {}",
            requests[0]
        );
        assert!(
            requests[0]
                .to_ascii_lowercase()
                .contains("authorization: bearer sk-test-plan-client"),
            "必须携带专用 token: {}",
            requests[0]
        );
    }

    /// Kimi 请求同样使用专用凭据并解析双窗用量。
    #[test]
    fn plan_kimi_request_parses_both_windows() {
        let fixture = HttpFixture::start(vec![response(
            "200 OK",
            "Content-Type: application/json\r\n",
            br#"{
                "usage": {"limit": "100", "used": "23", "remaining": "77", "resetTime": "2026-09-04T00:18:45Z"},
                "limits": [
                    {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                     "detail": {"limit": "100", "used": "1", "remaining": "99", "resetTime": "2026-08-28T22:18:45Z"}}
                ],
                "totalQuota": {"limit": "100", "remaining": "99"}
            }"#,
        )]);
        let store = Arc::new(RecordingStore {
            token: Some(b"sk-test-plan-kimi".to_vec()),
        });
        let client = client(store, &fixture.url("/coding/v1/usages"));

        let info = client.fetch(PlanBalanceSource::Kimi).expect("必须解析成功");
        let windows = info.windows.expect("双窗必须存在");
        assert_eq!(windows.five_hour.used_percent, 1);
        assert_eq!(windows.seven_day.used_percent, 23);

        let requests = fixture.finish();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("GET /coding/v1/usages HTTP/1.1"));
        assert!(
            requests[0]
                .to_ascii_lowercase()
                .contains("authorization: bearer sk-test-plan-kimi")
        );
    }

    /// 重定向不跟随：只有一个请求，返回状态错误。
    #[test]
    fn plan_request_does_not_follow_redirects() {
        let fixture = HttpFixture::start(vec![response(
            "302 Found",
            "Location: http://127.0.0.1:1/elsewhere\r\n",
            b"",
        )]);
        let store = Arc::new(RecordingStore {
            token: Some(b"sk-test-plan-redirect".to_vec()),
        });
        let client = client(store, &fixture.url("/user/balance"));

        let error = client
            .fetch(PlanBalanceSource::DeepSeek)
            .expect_err("重定向必须失败");
        assert_eq!(error, PlanBalanceError::Status(302));
        assert_eq!(fixture.finish().len(), 1, "不得发起第二次请求");
    }

    /// 超过 1 MiB 的响应明确拒绝，不解析截断内容。
    #[test]
    fn plan_response_over_budget_is_rejected() {
        let body = vec![b' '; MAX_RESPONSE_BYTES + 1];
        let fixture = HttpFixture::start(vec![response(
            "200 OK",
            "Content-Type: application/json\r\n",
            &body,
        )]);
        let store = Arc::new(RecordingStore {
            token: Some(b"sk-test-plan-large".to_vec()),
        });
        let client = client(store, &fixture.url("/user/balance"));

        let error = client
            .fetch(PlanBalanceSource::DeepSeek)
            .expect_err("超预算响应必须失败");
        assert_eq!(error, PlanBalanceError::ResponseTooLarge);
        fixture.finish();
    }

    /// 缺少专用凭据时不发请求；凭据后端错误不携带材料。
    #[test]
    fn plan_request_requires_a_dedicated_credential() {
        let fixture = HttpFixture::start(Vec::new());
        let store = Arc::new(RecordingStore { token: None });
        let client = client(store, &fixture.url("/user/balance"));

        let error = client
            .fetch(PlanBalanceSource::Kimi)
            .expect_err("缺少凭据必须失败");
        assert_eq!(error, PlanBalanceError::MissingCredential);
        assert_eq!(fixture.finish().len(), 0, "缺少凭据不得发起请求");
    }

    /// 集成锚：套餐凭据取自专用目标，CLI 假凭据不参与请求。
    #[test]
    fn integration_plan_credentials() {
        let fixture = HttpFixture::start(vec![response(
            "200 OK",
            "Content-Type: application/json\r\n",
            br#"{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"1.00"}]}"#,
        )]);
        // CLI 认证材料的替身文件：套餐请求不得读取它。
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("credentials.json"),
            br#"{"token":"sk-test-cli-credential"}"#,
        )
        .unwrap();
        let store = Arc::new(RecordingStore {
            token: Some(b"sk-test-dedicated-plan".to_vec()),
        });
        let client = client(store, &fixture.url("/user/balance"));
        client.fetch(PlanBalanceSource::DeepSeek).expect("解析成功");

        let requests = fixture.finish();
        assert_eq!(requests.len(), 1);
        let request = requests[0].to_ascii_lowercase();
        assert!(request.contains("bearer sk-test-dedicated-plan"));
        assert!(
            !request.contains("sk-test-cli-credential"),
            "不得使用 CLI 认证材料"
        );
    }
}
