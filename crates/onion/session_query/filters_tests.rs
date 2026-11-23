//! `session_query::filters` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::{corpus_descriptor, decode_cursor, encode_cursor, stable_digest, validate_cursor, SearchCursor, TextFilter, CURSOR_VERSION};
use crate::session_query::SessionFile;

#[test]
fn text_filter_matches_words_with_flexible_whitespace() {
    let filter = TextFilter::compile("alpha beta");
    // 大小写不敏感 + 词间空白长度弹性。
    assert_eq!(filter.match_count("ALPHA   beta"), Some(1));
    assert_eq!(filter.match_count("x alpha\n\tbeta y"), Some(1));
    // 词序不对 / 无空白分隔 → 不匹配。
    assert_eq!(filter.match_count("alphabeta"), None);
    assert_eq!(filter.match_count("beta alpha"), None);
    // 非重叠计数。
    assert_eq!(filter.match_count("alpha beta alpha beta"), Some(2));
}

#[test]
fn text_filter_single_word_counts_all_occurrences() {
    let filter = TextFilter::compile("go");
    assert_eq!(filter.match_count("go go go"), Some(3));
    // 单词保持字面量子串语义（不要求词边界）。
    assert_eq!(filter.match_count("golang"), Some(1));
    assert_eq!(filter.match_count("gopher goes"), Some(2));
    assert_eq!(filter.match_count("nothing"), None);
}

#[test]
fn text_filter_cjk_matches_across_whitespace_only() {
    let filter = TextFilter::compile("你好 世界");
    assert_eq!(filter.match_count("前缀 你好   世界 后缀"), Some(1));
    assert_eq!(filter.match_count("你好世界"), None);
}

#[test]
fn cursor_roundtrip_and_validation() {
    let cursor = SearchCursor { v: CURSOR_VERSION, f: "fp".into(), g: "gen".into(), o: 7 };
    let decoded = decode_cursor(&encode_cursor(&cursor)).unwrap();
    assert_eq!(decoded.o, 7);
    assert_eq!(decoded.v, CURSOR_VERSION);
    assert_eq!(decoded.f, "fp");
    assert_eq!(decoded.g, "gen");

    // 校验三态：无游标 → 0；指纹 / 代际相符 → 偏移；不符 → Err。
    assert_eq!(validate_cursor(None, "fp", "gen").unwrap(), 0);
    assert_eq!(validate_cursor(Some(&cursor), "fp", "gen").unwrap(), 7);
    assert!(validate_cursor(Some(&cursor), "other", "gen").is_err());
    assert!(validate_cursor(Some(&cursor), "fp", "other").is_err());
    // 坏 token（非 base64url）→ Err。
    assert!(decode_cursor("!!!not-base64!!!").is_err());
}

#[test]
fn corpus_descriptor_is_order_independent_and_len_sensitive() {
    let dir = std::env::temp_dir().join(format!("onion_filters_corpus_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.jsonl"), "abc").unwrap();
    std::fs::write(dir.join("b.jsonl"), "xyz").unwrap();
    let forward = corpus_descriptor(&[
        SessionFile { id: "a".to_string(), path: dir.join("a.jsonl") },
        SessionFile { id: "b".to_string(), path: dir.join("b.jsonl") },
    ]);
    let backward = corpus_descriptor(&[
        SessionFile { id: "b".to_string(), path: dir.join("b.jsonl") },
        SessionFile { id: "a".to_string(), path: dir.join("a.jsonl") },
    ]);
    // 顺序无关（内部按 id 排序）。
    assert_eq!(forward, backward);
    // 文件长度变化（追加）→ 描述符不同（在途游标判 Stale）。
    std::fs::write(dir.join("a.jsonl"), "abcd").unwrap();
    assert_ne!(forward, corpus_descriptor(&[
        SessionFile { id: "a".to_string(), path: dir.join("a.jsonl") },
        SessionFile { id: "b".to_string(), path: dir.join("b.jsonl") },
    ]));
    // 缺失文件按长度 0 处理。
    let ghost = SessionFile { id: "ghost".to_string(), path: dir.join("ghost.jsonl") };
    assert_eq!(corpus_descriptor(&[ghost]), "ghost:0");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stable_digest_is_deterministic_hex() {
    let a = stable_digest("alpha");
    assert_eq!(a, stable_digest("alpha"));
    assert_eq!(a.len(), 32);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(a, stable_digest("beta"));
}
