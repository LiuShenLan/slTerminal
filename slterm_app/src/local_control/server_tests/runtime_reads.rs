//! Runtime 终端分页读取：快照复用、游标失效与响应预算。

use super::runtime::{
    hello_scoped, start_dual_scope_server_with_read, start_dual_scope_server_with_read_and_wait,
    terminal_read_request,
};
use super::*;

#[test]
fn runtime_terminal_read_pages_snapshot_and_rejects_stale_cursor() {
    let terminal = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d";
    let text = format!(
        "{}\ntail",
        "a".repeat(crate::local_control::reads::PAGE_BUDGET_BYTES)
    );
    let snapshot_state = Arc::new(Mutex::new((3u64, 5u64, text)));
    let read_state = Arc::clone(&snapshot_state);
    let (mut server, _calls) = start_dual_scope_server_with_read(move |target| {
        assert_eq!(target.terminal_id.to_string(), terminal);
        assert_eq!(target.generation, 3);
        let state = read_state.lock().expect("读取快照");
        Ok(crate::local_control::reads::TerminalTextSnapshot {
            generation: state.0,
            revision: state.1,
            text: state.2.as_str().into(),
        })
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(&mut stream, &terminal_read_request(terminal, 3, None));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(
        reply["result"]["data"].as_str().unwrap().len(),
        crate::local_control::reads::PAGE_BUDGET_BYTES
    );
    assert_eq!(reply["result"]["generation"], 3);
    assert_eq!(reply["result"]["revision"], 5);
    let cursor = reply["result"]["next_cursor"]
        .as_str()
        .expect("尾部仍有内容必须返回续读游标")
        .to_owned();

    write_json(
        &mut stream,
        &terminal_read_request(terminal, 3, Some(&cursor)),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["data"], "\ntail");
    assert!(reply["result"]["next_cursor"].is_null());

    // 输出 revision 变化后旧游标必须失效。
    snapshot_state.lock().expect("读取快照").1 = 6;
    write_json(
        &mut stream,
        &terminal_read_request(terminal, 3, Some(&cursor)),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "stale_cursor");

    write_json(
        &mut stream,
        &terminal_read_request("not-a-terminal-id", 3, None),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "invalid_request");
    server.shutdown().unwrap();
}

/// 续页复用不可变全文，不再向 worker 重新物化；revision 变化仍使旧游标失效。
#[test]
fn runtime_terminal_read_pages_reuse_the_cached_snapshot() {
    let terminal = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2e";
    let text = format!(
        "{}\ntail-page",
        "b".repeat(crate::local_control::reads::PAGE_BUDGET_BYTES)
    );
    let reads = Arc::new(AtomicUsize::new(0));
    let revision = Arc::new(AtomicU64::new(4));
    let reads_by_callback = Arc::clone(&reads);
    let revision_by_callback = Arc::clone(&revision);
    let revision_by_wait = Arc::clone(&revision);
    let (mut server, _calls) = start_dual_scope_server_with_read_and_wait(
        move |_| {
            reads_by_callback.fetch_add(1, Ordering::SeqCst);
            Ok(crate::local_control::reads::TerminalTextSnapshot {
                generation: 3,
                revision: revision_by_callback.load(Ordering::SeqCst),
                text: text.as_str().into(),
            })
        },
        move |_| Ok(revision_by_wait.load(Ordering::SeqCst)),
    );
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(&mut stream, &terminal_read_request(terminal, 3, None));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    let cursor = reply["result"]["next_cursor"]
        .as_str()
        .expect("续读游标")
        .to_owned();
    assert_eq!(reads.load(Ordering::SeqCst), 1);

    write_json(
        &mut stream,
        &terminal_read_request(terminal, 3, Some(&cursor)),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["data"], "\ntail-page");
    assert_eq!(
        reads.load(Ordering::SeqCst),
        1,
        "续页必须命中缓存，不得重新读取 worker"
    );

    revision.store(5, Ordering::SeqCst);
    write_json(
        &mut stream,
        &terminal_read_request(terminal, 3, Some(&cursor)),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "stale_cursor");
    server.shutdown().unwrap();
}
