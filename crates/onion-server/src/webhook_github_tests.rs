//! `webhook_github` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 覆盖验签（含 RFC 4231 已知答案）、settled 规则与重投去重；HTTP 路由经真实
//! 进程冒烟验证（签名 202 / 未签名 401 / ping 200）。

use serde_json::json;

use super::*;

/// RFC 4231 Test Case 1：key = 0x0b × 20，data = "Hi There"。
#[test]
fn hmac_sha256_matches_rfc4231_known_answer() {
    let mac = hmac_sha256(&[0x0b; 20], b"Hi There");
    let hex: String = mac.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(
        hex,
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
    );
}

#[test]
fn verify_signature_accepts_valid_and_rejects_invalid() {
    let secret = "s3cret";
    let body = br#"{"action":"opened"}"#;
    let mac = hmac_sha256(secret.as_bytes(), body);
    let hex: String = mac.iter().map(|byte| format!("{byte:02x}")).collect();
    assert!(verify_signature(secret, body, Some(&format!("sha256={hex}"))));
    // 错密钥 / 错 body / 缺前缀 / 非法十六进制 / 缺头 一律拒绝。
    assert!(!verify_signature("other", body, Some(&format!("sha256={hex}"))));
    assert!(!verify_signature(secret, b"{}", Some(&format!("sha256={hex}"))));
    assert!(!verify_signature(secret, body, Some(&hex)));
    assert!(!verify_signature(secret, body, Some("sha256=zz")));
    assert!(!verify_signature(secret, body, None));
}

#[test]
fn settled_prompt_dispatches_only_opened_issues_and_pull_requests() {
    let issue = json!({
        "action": "opened",
        "repository": { "full_name": "acme/repo" },
        "issue": {
            "number": 7,
            "title": "Crash on start",
            "body": "Steps to reproduce …",
            "html_url": "https://github.com/acme/repo/issues/7"
        }
    });
    let prompt = settled_prompt("issues", &issue).unwrap();
    assert!(prompt.contains("GitHub issue was opened in acme/repo (#7): Crash on start"));
    assert!(prompt.contains("Steps to reproduce"));
    assert!(prompt.contains("https://github.com/acme/repo/issues/7"));

    let pull_request = json!({
        "action": "opened",
        "repository": { "full_name": "acme/repo" },
        "pull_request": { "number": 9, "title": "Fix", "body": "…", "html_url": "u" }
    });
    assert!(settled_prompt("pull_request", &pull_request)
        .unwrap()
        .contains("GitHub pull request was opened"));

    // 非 opened / 非目标事件 / 缺主体 一律不派发。
    let edited = json!({ "action": "edited", "issue": { "title": "t" } });
    assert!(settled_prompt("issues", &edited).is_none());
    assert!(settled_prompt("push", &issue).is_none());
    assert!(settled_prompt("issues", &json!({ "action": "opened" })).is_none());
}

#[test]
fn truncate_prompt_marks_overflow() {
    let short = "hello".to_string();
    assert_eq!(truncate_prompt(short), "hello");
    let long = "中".repeat(MAX_PROMPT_CHARS + 10);
    let truncated = truncate_prompt(long);
    assert!(truncated.ends_with("[truncated]"));
    assert!(truncated.chars().count() > MAX_PROMPT_CHARS);
}

#[test]
fn duplicate_deliveries_are_rejected_within_window() {
    let delivery = format!("test-delivery-{}", std::process::id());
    assert!(mark_delivery(&delivery));
    assert!(!mark_delivery(&delivery));
}
