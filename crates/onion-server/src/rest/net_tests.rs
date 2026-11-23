//! `net` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::validate_public_https_url;

#[test]
fn accepts_public_https_hosts() {
    for url in [
        "https://api.skillhub.cn/api/v1/search?q=pdf",
        "https://example.com/",
        "https://8.8.8.8/dns-query",
    ] {
        assert!(validate_public_https_url(url).is_ok(), "应放行 {url}");
    }
}

#[test]
fn rejects_private_or_non_https_targets() {
    for url in [
        "http://example.com/",            // 非 https
        "ftp://example.com/file",         // 非 http(s) scheme
        "https://localhost/x",            // localhost
        "https://sub.localhost/x",        // localhost 变体
        "https://127.0.0.1/x",            // 环回 IP
        "https://10.1.2.3/x",             // 私网 IP
        "https://192.168.1.1/x",          // 私网 IP
        "https://169.254.169.254/x",      // 链路本地（云 metadata）
        "https://intranet/x",             // 无点单标签主机名
        "not-a-url",                      // 解析失败
    ] {
        assert!(validate_public_https_url(url).is_err(), "应拒绝 {url}");
    }
}
