//! 更新链本地 HTTP 夹具(仅测试编译)。
//!
//! 夹具扮演 HTTP 代理:ureq 经同一套系统代理解析进入本服务器,因此
//! 任意主机名(api.github.com / CDN / 重定向目标)都在本地应答,测试绝不
//! 触碰真实网络。每个响应按顺序消费;请求行逐条留档供断言。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) struct Server {
    pub(crate) address: SocketAddr,
    worker: Option<JoinHandle<Vec<String>>>,
}

impl Server {
    pub(crate) fn start(responses: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let worker = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let deadline = Instant::now() + Duration::from_secs(5);
                let stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "fixture received no connection");
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("fixture accept failed: {error}"),
                    }
                };
                // Windows 上 accept 出的 socket 继承 listener 的非阻塞模式。
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut reader = BufReader::new(stream);
                let first = read_headers(&mut reader);
                let request = if first.starts_with("CONNECT ") {
                    reader
                        .get_mut()
                        .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                        .unwrap();
                    // CONNECT 之后同一条连接上是真正的请求;留档第一行。
                    read_headers(&mut reader)
                } else {
                    first
                };
                requests.push(request);
                reader.get_mut().write_all(response.as_bytes()).unwrap();
            }
            requests
        });
        Self {
            address,
            worker: Some(worker),
        }
    }

    /// 经夹具代理访问任意 URL 的客户端;排除项模拟 `NO_PROXY`。
    pub(crate) fn agent(&self, exclusions: &[&str]) -> ureq::Agent {
        let server = self.address.to_string();
        // 走与生产同一套系统代理解析与回退策略。
        let url = super::parse_windows_proxy_server(&server, "http").unwrap();
        let proxy = super::resolve_with_system(
            Some((
                url,
                exclusions.iter().map(|entry| (*entry).to_owned()).collect(),
            )),
            || panic!("system proxy should win"),
        );
        ureq::config::Config::builder()
            .proxy(proxy)
            .timeout_global(Some(Duration::from_secs(4)))
            .max_redirects(5)
            .build()
            .new_agent()
    }

    /// 等待夹具线程收尾并返回每条请求行。
    pub(crate) fn finish(mut self) -> Vec<String> {
        self.worker
            .take()
            .unwrap()
            .join()
            .expect("HTTP fixture failed")
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            // 失败用例也必须回收有界夹具线程。
            let _ = worker.join();
        }
    }
}

fn read_headers(reader: &mut BufReader<TcpStream>) -> String {
    let mut headers = String::new();
    let mut first_line = String::new();
    loop {
        let mut line = String::new();
        assert!(
            reader.read_line(&mut line).unwrap() > 0,
            "unexpected fixture EOF"
        );
        if line == "\r\n" {
            // 未读完正文就在 Windows 上关闭连接会触发 TCP reset。
            let length = headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .map(|(_, value)| value.trim().parse::<u64>().unwrap())
                .unwrap_or(0);
            assert!(length <= 128 * 1024, "fixture request body is too large");
            let read = std::io::copy(&mut reader.take(length), &mut std::io::sink()).unwrap();
            assert_eq!(read, length, "unexpected fixture body EOF");
            return first_line;
        }
        if first_line.is_empty() {
            first_line = line.trim_end().to_owned();
        }
        headers.push_str(&line);
        assert!(headers.len() < 16 * 1024);
    }
}

/// 构造完整 HTTP 响应(Content-Length + Connection: close)。
pub(crate) fn response(status: &str, headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
        body.len()
    )
}
