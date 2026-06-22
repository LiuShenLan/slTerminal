/// 集成测试 — 验证 slterminal_lib crate 核心类型可链接且行为正确
use slterminal_lib::AppState;
use slterminal_lib::AppError;
use slterminal_lib::PtyState;

/// 验证 crate 可链接，AppState::new() 创建成功
#[test]
fn test_app_state_creation() {
    let state = AppState::new();
    assert!(
        state.pty.sessions.read().unwrap().is_empty(),
        "新建 AppState 的 sessions 应为空"
    );
}

/// 验证 PtyState::new() 创建成功
#[test]
fn test_pty_state_creation() {
    let pty = PtyState::new();
    assert!(
        pty.sessions.read().unwrap().is_empty(),
        "新建 PtyState 的 sessions 应为空"
    );
}

/// 验证 AppError 所有变体的 Display 和 Debug 实现不 panic
#[test]
fn test_app_error_display_no_panic() {
    let errors: Vec<AppError> = vec![
        AppError::Io("io 测试".to_string()),
        AppError::Pty("pty 测试".to_string()),
        AppError::Git("git 测试".to_string()),
        AppError::Serde("serde 测试".to_string()),
        AppError::Unknown("unknown 测试".to_string()),
        AppError::SessionNotFound("session-1".to_string()),
        AppError::ChannelSend("channel 测试".to_string()),
    ];
    for err in &errors {
        // Display 不应 panic
        let display = format!("{err}");
        assert!(!display.is_empty(), "Display 输出不应为空");

        // Debug 不应 panic
        let debug = format!("{err:?}");
        assert!(!debug.is_empty(), "Debug 输出不应为空");
    }
}
