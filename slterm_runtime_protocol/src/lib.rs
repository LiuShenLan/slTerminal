//! slTerminal Runtime v2 线缆协议。
//!
//! 本 crate 只承载跨进程传输的借用字符串载荷与纯 Rust 校验：零生产依赖，
//! 不含序列化实现、平台路径校验或 UI 类型。反序列化归调用方，字段与校验
//! 规则的机器可读真值源是 [`methods::handover::HANDOFF_SCHEMA_JSON`]。

pub mod methods;

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::methods::handover::{
        HANDOFF_SCHEMA_JSON, HANDOFF_SCHEMA_VERSION, HandoffOp, HandoffReject, HandoffReply,
        HandoffRequest, HandoffValidationError,
    };

    fn request(op: HandoffOp<'_>) -> HandoffRequest<'_> {
        HandoffRequest {
            schema: HANDOFF_SCHEMA_VERSION,
            request_id: "req-1",
            op,
        }
    }

    #[test]
    fn schema_declares_ops_and_replies() {
        let schema: Value =
            serde_json::from_str(HANDOFF_SCHEMA_JSON).expect("handover schema must be valid JSON");
        assert_eq!(
            schema["$schema"], "https://json-schema.org/draft/2020-12/schema",
            "schema must target draft 2020-12"
        );
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["properties"]["schema"]["const"],
            HANDOFF_SCHEMA_VERSION
        );
        assert_eq!(schema["properties"]["request_id"]["minLength"], 1);

        let variants = schema["$defs"]["op"]["oneOf"]
            .as_array()
            .expect("op must be a oneOf of four variants");
        assert_eq!(variants.len(), 4);
        let mut kinds = Vec::new();
        for variant in variants {
            assert_eq!(variant["additionalProperties"], false);
            kinds.push(
                variant["properties"]["kind"]["const"]
                    .as_str()
                    .expect("each op variant must pin its kind"),
            );
        }
        kinds.sort_unstable();
        assert_eq!(
            kinds,
            ["focus", "new_terminal", "open_file", "open_project"]
        );

        let replies = schema["$defs"]["reply"]["oneOf"]
            .as_array()
            .expect("reply must be a oneOf of three variants");
        assert_eq!(replies.len(), 3);
        let mut reply_kinds = Vec::new();
        for variant in replies {
            assert_eq!(variant["additionalProperties"], false);
            reply_kinds.push(
                variant["properties"]["kind"]["const"]
                    .as_str()
                    .expect("each reply variant must pin its kind"),
            );
        }
        reply_kinds.sort_unstable();
        assert_eq!(reply_kinds, ["accepted", "busy", "rejected"]);

        let mut reasons: Vec<&str> = schema["$defs"]["reject"]["enum"]
            .as_array()
            .expect("reject must enumerate its reasons")
            .iter()
            .map(|reason| reason.as_str().expect("reject reasons must be strings"))
            .collect();
        reasons.sort_unstable();
        assert_eq!(
            reasons,
            [
                "invalid_path",
                "project_not_found",
                "project_not_ready",
                "pty_limit",
                "unknown_schema",
                "unsupported"
            ]
        );
    }

    #[test]
    fn validate_accepts_legal_requests() {
        assert_eq!(request(HandoffOp::Focus).validate(), Ok(()));
        assert_eq!(
            request(HandoffOp::OpenProject {
                project_root: r"C:\work\demo",
            })
            .validate(),
            Ok(())
        );
        assert_eq!(
            request(HandoffOp::OpenFile {
                project_root: Some("C:/work/demo"),
                path: "src/main.rs",
            })
            .validate(),
            Ok(())
        );
        assert_eq!(
            request(HandoffOp::NewTerminal {
                project_root: None,
                cwd: Some(r"C:\work\demo\src"),
            })
            .validate(),
            Ok(())
        );
        // 两个可选项都缺省合法:接收端回退活跃项目与默认工作目录。
        assert_eq!(
            request(HandoffOp::NewTerminal {
                project_root: None,
                cwd: None,
            })
            .validate(),
            Ok(())
        );
        // 畸形 Windows 路径归平台层判定,协议层不拒绝。
        assert_eq!(
            request(HandoffOp::OpenProject {
                project_root: r"C:\bad|name\..\\",
            })
            .validate(),
            Ok(())
        );
    }

    #[test]
    fn validate_rejects_unknown_schema() {
        let mut wrong = request(HandoffOp::Focus);
        wrong.schema = HANDOFF_SCHEMA_VERSION + 1;
        assert_eq!(wrong.validate(), Err(HandoffValidationError::UnknownSchema));
        wrong.schema = 0;
        assert_eq!(wrong.validate(), Err(HandoffValidationError::UnknownSchema));
    }

    #[test]
    fn validate_rejects_empty_request_id() {
        let mut wrong = request(HandoffOp::Focus);
        wrong.request_id = "";
        assert_eq!(
            wrong.validate(),
            Err(HandoffValidationError::EmptyRequestId)
        );
    }

    #[test]
    fn validate_rejects_internal_ids_in_path_fields() {
        for internal_id in ["project:1", "group:2", "panel:3"] {
            assert_eq!(
                request(HandoffOp::OpenProject {
                    project_root: internal_id,
                })
                .validate(),
                Err(HandoffValidationError::InternalId)
            );
            assert_eq!(
                request(HandoffOp::OpenFile {
                    project_root: Some(internal_id),
                    path: "src/lib.rs",
                })
                .validate(),
                Err(HandoffValidationError::InternalId)
            );
            assert_eq!(
                request(HandoffOp::OpenFile {
                    project_root: None,
                    path: internal_id,
                })
                .validate(),
                Err(HandoffValidationError::InternalId)
            );
            assert_eq!(
                request(HandoffOp::NewTerminal {
                    project_root: Some(internal_id),
                    cwd: None,
                })
                .validate(),
                Err(HandoffValidationError::InternalId)
            );
            assert_eq!(
                request(HandoffOp::NewTerminal {
                    project_root: None,
                    cwd: Some(internal_id),
                })
                .validate(),
                Err(HandoffValidationError::InternalId)
            );
        }
    }

    #[test]
    fn validate_rejects_empty_path_fields() {
        assert_eq!(
            request(HandoffOp::OpenProject { project_root: "" }).validate(),
            Err(HandoffValidationError::EmptyProjectRoot)
        );
        assert_eq!(
            request(HandoffOp::OpenFile {
                project_root: Some(""),
                path: "src/lib.rs",
            })
            .validate(),
            Err(HandoffValidationError::EmptyProjectRoot)
        );
        assert_eq!(
            request(HandoffOp::OpenFile {
                project_root: None,
                path: "",
            })
            .validate(),
            Err(HandoffValidationError::EmptyPath)
        );
        assert_eq!(
            request(HandoffOp::NewTerminal {
                project_root: None,
                cwd: Some(""),
            })
            .validate(),
            Err(HandoffValidationError::EmptyCwd)
        );
    }

    #[test]
    fn ops_carry_borrowed_paths() {
        let project_root = String::from(r"C:\work\demo");
        let path = String::from("src/main.rs");
        let cwd = String::from(r"C:\work\demo\src");

        let focus = request(HandoffOp::Focus);
        assert!(matches!(focus.op, HandoffOp::Focus));

        let open_project = request(HandoffOp::OpenProject {
            project_root: &project_root,
        });
        match open_project.op {
            HandoffOp::OpenProject { project_root: root } => {
                assert_eq!(root, project_root.as_str())
            }
            _ => panic!("expected OpenProject"),
        }

        let open_file = request(HandoffOp::OpenFile {
            project_root: Some(&project_root),
            path: &path,
        });
        match open_file.op {
            HandoffOp::OpenFile {
                project_root: root,
                path: file,
            } => {
                assert_eq!(root, Some(project_root.as_str()));
                assert_eq!(file, path.as_str());
            }
            _ => panic!("expected OpenFile"),
        }

        let new_terminal = request(HandoffOp::NewTerminal {
            project_root: None,
            cwd: Some(&cwd),
        });
        match new_terminal.op {
            HandoffOp::NewTerminal {
                project_root: root,
                cwd: workdir,
            } => {
                assert_eq!(root, None);
                assert_eq!(workdir, Some(cwd.as_str()));
            }
            _ => panic!("expected NewTerminal"),
        }
    }

    #[test]
    fn replies_carry_borrowed_request_id_and_reason() {
        let request_id = String::from("req-9");

        match (HandoffReply::Accepted {
            request_id: &request_id,
        }) {
            HandoffReply::Accepted { request_id: id } => assert_eq!(id, request_id.as_str()),
            _ => panic!("expected Accepted"),
        }

        match (HandoffReply::Busy {
            request_id: &request_id,
        }) {
            HandoffReply::Busy { request_id: id } => assert_eq!(id, request_id.as_str()),
            _ => panic!("expected Busy"),
        }

        let rejects = [
            HandoffReject::UnknownSchema,
            HandoffReject::InvalidPath,
            HandoffReject::ProjectNotFound,
            HandoffReject::ProjectNotReady,
            HandoffReject::PtyLimit,
            HandoffReject::Unsupported,
        ];
        for reason in rejects {
            match (HandoffReply::Rejected {
                request_id: &request_id,
                reason,
            }) {
                HandoffReply::Rejected {
                    request_id: id,
                    reason: actual,
                } => {
                    assert_eq!(id, request_id.as_str());
                    assert_eq!(actual, reason);
                }
                _ => panic!("expected Rejected"),
            }
        }
    }
}
