//! 会话历史查询 5 工具：
//! `session_search` / `session_event_search` / `session_trace` /
//! `session_event_trace` / `session_event_read`。
//!
//! 核心语义：
//! - 全部只读（ReadOnly）、非 deferred；
//! - `session_search`：跨会话全文搜索，工作区限定（仅当前 cwd 的 `sessions/`），
//!   始终排除调用会话，返回每会话最佳匹配；停在 `maxSearchResults`；
//! - `session_event_search`：单会话内全文搜索；当前会话停在调用步骤之前；
//! - `session_trace` / `session_event_trace` / `session_event_read`：精确读取。
//!
//! 授权保守：目标会话必须位于调用方 cwd 的 `sessions/<normalized-cwd>/` 下
//! （经 `sessions_dir` 定位天然限定，cwd 精确相等）。

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::filters::{
    corpus_descriptor, decode_cursor, encode_cursor, stable_digest, validate_cursor, SearchCursor,
    TextFilter, CURSOR_VERSION,
};
use super::{format_time, list_session_files, read_session_events, snippet, snippet_text, EventRecord};
use crate::session::{SessionEvent, SessionLog};
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// 单次搜索结果上限（缺省 100）。
const DEFAULT_MAX_SEARCH_RESULTS: usize = 100;
/// 命中摘录最大字符数。
const SNIPPET_MAX_CHARS: usize = 240;
/// 相邻事件摘要最大字符数。
const NEIGHBOR_MAX_CHARS: usize = 400;

/// 会话查询工具配置持有者：搜索上限 + 会话接线槽。**非 ToolPort**——实际工具
/// 由 [`define_all`](Self::define_all) 生成 5 个独立句柄，共享本持有的接线槽。
pub struct SessionQueryTool {
    /// 单次搜索返回的最大命中数（部署决策）。
    pub max_search_results: usize,
    /// 会话接线槽（Runtime 装配后 wire；用于读取调用会话 id 与计数）。
    session: Arc<OnceLock<Arc<SessionLog>>>,
}

impl SessionQueryTool {
    pub fn new(max_search_results: usize) -> Self {
        Self {
            max_search_results,
            session: Arc::new(OnceLock::new()),
        }
    }

    /// 延迟接线会话（宿主装配完成后调用）。
    pub fn wire_session(&self, session: Arc<SessionLog>) {
        let _ = self.session.set(session);
    }

    fn caller(&self) -> Option<CallerContext> {
        self.session.get().map(|session| CallerContext {
            id: session.session_id().to_string(),
            recorded_events: session.snapshot().len() as u64,
        })
    }

    /// 生成 5 个独立工具句柄（共享同一接线槽）。
    pub fn define_all(&self) -> Vec<Arc<dyn ToolPort>> {
        let session = self.session.clone();
        let max_results = self.max_search_results;
        vec![
            Arc::new(SearchTool {
                name: "session_search",
                max_results,
                session: session.clone(),
                event_scoped: false,
            }),
            Arc::new(SearchTool {
                name: "session_event_search",
                max_results,
                session: session.clone(),
                event_scoped: true,
            }),
            Arc::new(ReadTool { name: "session_trace", session: session.clone(), kind: ReadKind::Trace }),
            Arc::new(ReadTool { name: "session_event_trace", session: session.clone(), kind: ReadKind::EventTrace }),
            Arc::new(ReadTool { name: "session_event_read", session, kind: ReadKind::EventRead }),
        ]
    }
}

/// 调用方上下文：会话 id + counts（用于「排除调用会话」与「停在调用步骤之前」）。
struct CallerContext {
    id: String,
    /// 调用时已记录的事件数（`session_event_search` 对当前会话以此作为
    /// seq 上界：`lastStepStartSeq - 1`）。
    recorded_events: u64,
}

/// 进程 cwd 作为工作区根（onion 会话按 cwd 隔离；查询同源）。
fn cwd_path() -> &'static Path {
    static CWD: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    CWD.get_or_init(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
        .as_path()
}

fn str_arg(call: &ToolCall, key: &str) -> Option<String> {
    call.input.get(key).and_then(serde_json::Value::as_str).map(str::to_string)
}

fn u64_arg(call: &ToolCall, key: &str) -> Result<Option<u64>, String> {
    match call.input.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a non-negative integer")),
    }
}

/// 规范化查询：去空白、合并空白；空 → Err（`normalizeQuery` 语义）。
fn normalize_query(value: &str) -> Result<String, String> {
    let query = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if query.is_empty() {
        return Err("session search query must contain non-whitespace text".to_string());
    }
    if query.contains('\0') {
        return Err("session search query must not contain NUL".to_string());
    }
    Ok(query)
}

fn target_id(call: &ToolCall, caller: &CallerContext) -> String {
    str_arg(call, "session_id").unwrap_or_else(|| caller.id.clone())
}

/// 加载指定会话的事件（定位 `<dir>/<encoded-id>/session.vN.jsonl`）。
fn load_session(dir: &Path, session_id: &str) -> Result<Vec<EventRecord>, String> {
    load_session_at(
        dir,
        &crate::paths::subagent_logs_dir(&crate::paths::data_home()),
        session_id,
    )
}

/// 同 [`load_session`]，但子代理日志根显式传入（装配读面共用、测试可注入）。
///
/// 子代理会话与父会话**并列**存放（`<root>/logs/subagents/<id>/`，不按 cwd 分区），
/// 因此父路径读不到时回落该根。回落的必要性不只是「能读到」：一旦报
/// 「not readable」，模型会把它当成会话不存在而反复换关键词重试，每次重试
/// 都是一整轮模型往返（真机实测单次可浪费约 30 s）。
fn load_session_at(
    dir: &Path,
    subagent_root: &Path,
    session_id: &str,
) -> Result<Vec<EventRecord>, String> {
    let file = crate::paths::session_log_file(&crate::paths::session_dir(dir, session_id));
    match read_session_events(&file) {
        Ok(records) => Ok(records),
        Err(primary) => {
            let subagent = crate::paths::session_log_file(
                &subagent_root.join(crate::paths::encode_session_id(session_id)),
            );
            read_session_events(&subagent)
                .map_err(|_| format!("session {session_id} is not readable: {primary}"))
        }
    }
}

/// 解析 ISO 8601 时间（带 `Z` 或 `±HH:MM` 偏移）为 epoch 毫秒；不合法返回 Err。
fn parse_iso_time(value: &str) -> Result<u64, String> {
    // 支持 `YYYY-MM-DDTHH:MM:SS(.fff)?(Z|±HH:MM)` 与省略秒/时分的宽松形态。
    let value = value.trim();
    let (body, offset_secs) = {
        let (b, tail) = if let Some(rest) = value.strip_suffix('Z') {
            (rest, 0i64)
        } else if let Some(plus) = value.rfind('+') {
            (&value[..plus], parse_offset(&value[plus + 1..])?)
        } else if let Some(minus) = value.rfind('-') {
            // 排除日期部分本身的连字符：偏移前的 `-` 位于时间部分后。
            let after_t = value.find('T').unwrap_or(0);
            if minus < after_t {
                (value, 0i64)
            } else {
                (&value[..minus], -parse_offset(&value[minus + 1..])?)
            }
        } else {
            (value, 0i64)
        };
        (b, tail)
    };
    let ts = parse_utc_body(body)?;
    Ok((ts as i64 - offset_secs * 1000) as u64)
}

/// 解析 `HH:MM(:SS)?` 偏移为秒数。
fn parse_offset(s: &str) -> Result<i64, String> {
    let parts: Vec<&str> = s.split(':').collect();
    let hour: i64 = parts
        .first()
        .map(|h| h.parse().map_err(|_| "invalid offset hour".to_string()))
        .ok_or_else(|| "invalid offset".to_string())??;
    let minute: i64 = parts
        .get(1)
        .map(|m| m.parse().map_err(|_| "invalid offset minute".to_string()))
        .unwrap_or(Ok(0))?;
    let second: i64 = parts
        .get(2)
        .map(|s2| s2.parse().map_err(|_| "invalid offset second".to_string()))
        .unwrap_or(Ok(0))?;
    Ok(hour * 3600 + minute * 60 + second)
}

/// 解析 `YYYY-MM-DDTHH:MM:SS(.fff)?` 为 epoch 毫秒（视为 UTC）。
fn parse_utc_body(body: &str) -> Result<u64, String> {
    let (date_part, time_part) = match body.split_once('T') {
        Some((d, t)) => (d, t),
        None => (body, ""),
    };
    let date: Vec<&str> = date_part.split('-').collect();
    if date.len() != 3 {
        return Err(format!("invalid ISO 8601 timestamp: {body}"));
    }
    let year: i64 = date[0].parse().map_err(|_| "invalid year".to_string())?;
    let month: i64 = date[1].parse().map_err(|_| "invalid month".to_string())?;
    let day: i64 = date[2].parse().map_err(|_| "invalid day".to_string())?;
    let (hour, minute, second, millis) = if time_part.is_empty() {
        (0, 0, 0, 0)
    } else {
        let time = time_part.trim_end_matches('Z');
        let mut parts = time.split(':');
        let hour: i64 = parts.next().map(|h| h.parse().map_err(|_| "invalid hour".to_string())).unwrap_or(Ok(0))?;
        let minute: i64 = parts.next().map(|m| m.parse().map_err(|_| "invalid minute".to_string())).unwrap_or(Ok(0))?;
        let sec_part = parts.next().unwrap_or("0");
        let (second, millis) = if let Some((s, f)) = sec_part.split_once('.') {
            let s: i64 = s.parse().map_err(|_| "invalid second".to_string())?;
            let frac: String = format!("{:0<3}", &f[..f.len().min(3)]);
            let millis: i64 = frac.parse().unwrap_or(0);
            (s, millis)
        } else {
            (sec_part.parse().map_err(|_| "invalid second".to_string())?, 0)
        };
        (hour, minute, second, millis)
    };
    let days = days_from_civil(year, month, day)?;
    let secs = days * 86_400 + hour * 3600 + minute * 60 + second;
    Ok((secs as u64) * 1000 + millis as u64)
}

/// 民用日期 → 1970-01-01 起的天数（Howard Hinnant 算法反向）。
fn days_from_civil(y: i64, m: i64, d: i64) -> Result<i64, String> {
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err("invalid calendar date".to_string());
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok(era * 146_097 + doe - 719_468)
}

/// 解析时间过滤参数（`<key>` 字符串 ISO 8601），返回 epoch 毫秒。
fn time_arg(call: &ToolCall, key: &str) -> Result<Option<u64>, String> {
    match call.input.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => {
            let s = value.as_str().ok_or_else(|| format!("{key} must be an ISO 8601 string"))?;
            parse_iso_time(s).map(Some)
        }
    }
}

/// 会话查询过滤参数集合（`session_search` / `session_event_search` 共用）。
/// onion 无 lineage/availability/surfaces 数据模型，仅封装可兑现过滤。
#[derive(Default)]
struct SearchFilter {
    session_ids: Option<Vec<String>>,
    seq_from: Option<u64>,
    seq_to: Option<u64>,
    time_from: Option<u64>,
    time_to: Option<u64>,
    event_types: Option<Vec<String>>,
    /// 会话创建时间下界（epoch 毫秒；取目标会话首个 `SessionStarted` 的 ts）。
    created_at_from: Option<u64>,
    /// 会话创建时间上界（epoch 毫秒；取目标会话首个 `SessionStarted` 的 ts）。
    created_at_to: Option<u64>,
}

/// 解析字符串数组过滤参数（如 `event_types` / `session_ids`）。
fn strings_arg(call: &ToolCall, key: &str) -> Result<Option<Vec<String>>, String> {
    match call.input.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => {
            let arr = value.as_array().ok_or_else(|| format!("{key} must be an array of strings"))?;
            let mut out = Vec::new();
            for item in arr {
                let s = item.as_str().ok_or_else(|| format!("{key} must be an array of strings"))?;
                out.push(s.to_string());
            }
            Ok(Some(out))
        }
    }
}

/// created_at 是否落在 [from, to]（任一缺失即无界）。
fn created_at_in_range(created_at: u64, filter: &SearchFilter) -> bool {
    filter.created_at_from.is_none_or(|from| created_at >= from)
        && filter.created_at_to.is_none_or(|to| created_at <= to)
}

/// 会话创建时间：首个 `SessionStarted` 事件的 ts；缺失视为 0（未知）。
fn session_created_at(records: &[EventRecord]) -> u64 {
    records
        .iter()
        .find(|r| matches!(r.event, crate::session::SessionEvent::SessionStarted { .. }))
        .map(|r| r.ts)
        .unwrap_or(0)
}

/// 单条候选命中：跨会话 / 单会话搜索共用的排序与分页单元。
struct RankedHit {
    session_id: String,
    seq: u64,
    ts: u64,
    /// 词序列匹配次数（非重叠计数；首要排序键，降序）。
    count: usize,
    /// 文档长度（字符数；同分时短者优先——`document_length ASC`）。
    doc_len: usize,
    ty: String,
    text: String,
}

impl RankedHit {
    /// 从一条事件构造命中；词序列不匹配时返回 `None`。
    fn from_doc(session_id: &str, record: &EventRecord, doc: &str, words: &TextFilter) -> Option<Self> {
        let count = words.match_count(doc)?;
        Some(Self {
            session_id: session_id.to_string(),
            seq: record.seq,
            ts: record.ts,
            count,
            doc_len: doc.chars().count(),
            ty: super::event_type(&record.event),
            text: snippet_text(doc, SNIPPET_MAX_CHARS),
        })
    }

    /// 排序：match_count 降序 → doc_len 升序 → ts 降序 → seq 降序 → session_id 升序
    /// （`match_count DESC, document_length ASC, time DESC` 口径，组内 seq DESC、
    /// 跨会话 session_id ASC；全序确定，保证游标分页偏移稳定）。
    fn stronger(&self, other: &Self) -> std::cmp::Ordering {
        other
            .count
            .cmp(&self.count)
            .then_with(|| self.doc_len.cmp(&other.doc_len))
            .then_with(|| other.ts.cmp(&self.ts))
            .then_with(|| other.seq.cmp(&self.seq))
            .then_with(|| self.session_id.cmp(&other.session_id))
    }
}

/// 请求指纹：query + 过滤参数 + limit 的稳定摘要（跨页参数变化即判 Invalid cursor）。
fn search_fingerprint(query: &str, filter: &SearchFilter, max_results: usize) -> String {
    let canonical = format!(
        "q={query};ids={:?};seq={:?}-{:?};time={:?}-{:?};types={:?};created={:?}-{:?};limit={max_results}",
        filter.session_ids,
        filter.seq_from,
        filter.seq_to,
        filter.time_from,
        filter.time_to,
        filter.event_types,
        filter.created_at_from,
        filter.created_at_to,
    );
    stable_digest(&canonical)
}

/// 在结果尾部追加游标翻页提示（`end < total` 时调用）。
fn cursor_footer(lines: &mut Vec<String>, end: usize, fingerprint: &str, generation: &str) {
    lines.push(String::new());
    lines.push(
        "Result cap reached. Pass \"cursor\" with the token below (keep the other parameters unchanged) for the next page."
            .to_string(),
    );
    lines.push(format!(
        "Next cursor: {}",
        encode_cursor(&SearchCursor {
            v: CURSOR_VERSION,
            f: fingerprint.to_string(),
            g: generation.to_string(),
            o: end as u64,
        }),
    ));
}

/// `session_search`：跨会话全文搜索，工作区限定，排除调用会话，每会话最佳匹配；
/// 结果按相关性排序，超出 `max_results` 部分经游标翻页。
fn execute_session_search(
    dir: &Path,
    caller: &CallerContext,
    query: &str,
    max_results: usize,
    filter: &SearchFilter,
    cursor: Option<&SearchCursor>,
) -> Result<String, String> {
    let words = TextFilter::compile(query);
    // 候选会话：排除调用会话 + `session_ids` 过滤，按 id 排序保证确定性。
    let mut candidates: Vec<_> = list_session_files(dir)
        .into_iter()
        .filter(|file| file.id != caller.id)
        .filter(|file| filter.session_ids.as_ref().is_none_or(|ids| ids.iter().any(|id| id == &file.id)))
        .collect();
    candidates.sort_by(|a, b| a.id.cmp(&b.id));
    // 语料代际：候选集（id + 日志长度）的稳定摘要；集合或文件增改即判 Stale。
    let generation = stable_digest(&corpus_descriptor(&candidates));
    let fingerprint = search_fingerprint(query, filter, max_results);
    let offset = validate_cursor(cursor, &fingerprint, &generation)?;

    let mut ranked: Vec<RankedHit> = Vec::new();
    for file in &candidates {
        let Ok(records) = read_session_events(&file.path) else { continue };
        // 会话级 created_at 过滤（取首个 `SessionStarted` 的 ts；缺失视为 0）。
        if !created_at_in_range(session_created_at(&records), filter) {
            continue;
        }
        for record in &records {
            if !event_matches(record, filter) {
                continue;
            }
            let doc = super::event_json(record);
            if let Some(hit) = RankedHit::from_doc(&file.id, record, &doc, &words) {
                ranked.push(hit);
            }
        }
    }
    // 相关性排序后每会话仅保留最强命中（每会话最佳匹配）。
    ranked.sort_by(RankedHit::stronger);
    let mut seen: HashSet<String> = HashSet::new();
    ranked.retain(|hit| seen.insert(hit.session_id.clone()));

    let total = ranked.len();
    if total == 0 {
        return Ok("No prior session matches found.".to_string());
    }
    let start = offset.min(total as u64) as usize;
    let end = (start + max_results).min(total);
    let mut lines = vec![format!("Session search results ({}):", end - start)];
    for (index, hit) in ranked[start..end].iter().enumerate() {
        lines.push(String::new());
        lines.push(format!(
            "{}. Session {} — best match: seq {} | {} | {} | matches {}",
            start + index + 1,
            hit.session_id,
            hit.seq,
            hit.ty,
            format_time(hit.ts),
            hit.count,
        ));
        lines.push(format!("   Snippet: {}", hit.text));
    }
    if end < total {
        cursor_footer(&mut lines, end, &fingerprint, &generation);
    }
    Ok(lines.join("\n"))
}

/// 事件是否匹配过滤条件（seq / 时间 / 类型）。
fn event_matches(record: &EventRecord, filter: &SearchFilter) -> bool {
    if let Some(f) = filter.seq_from {
        if record.seq < f {
            return false;
        }
    }
    if let Some(t) = filter.seq_to {
        if record.seq > t {
            return false;
        }
    }
    if let Some(f) = filter.time_from {
        if record.ts < f {
            return false;
        }
    }
    if let Some(t) = filter.time_to {
        if record.ts > t {
            return false;
        }
    }
    if let Some(types) = &filter.event_types {
        let ty = super::event_type(&record.event);
        if !types.iter().any(|t| t == &ty) {
            return false;
        }
    }
    true
}

/// `session_event_search`：单会话内全文搜索；当前会话停在调用步骤之前；
/// 结果按相关性排序，超出 `max_results` 部分经游标翻页。
fn execute_event_search(
    dir: &Path,
    caller: &CallerContext,
    call: &ToolCall,
    query: &str,
    max_results: usize,
    filter: &SearchFilter,
    cursor: Option<&SearchCursor>,
) -> Result<String, String> {
    let session_id = target_id(call, caller);
    let records = load_session(dir, &session_id)?;
    // 会话级 created_at 过滤（与 `session_search` 同语义；越界即无命中）。
    let zero = format!("Session {session_id} — event search results (0):");
    if !created_at_in_range(session_created_at(&records), filter) {
        return Ok(zero);
    }
    let words = TextFilter::compile(query);
    let mut ranked: Vec<RankedHit> = Vec::new();
    for record in &records {
        // 当前会话排除调用步骤之后的事件（seq > recorded_events 属「未来」）。
        if session_id == caller.id && record.seq > caller.recorded_events {
            continue;
        }
        if !event_matches(record, filter) {
            continue;
        }
        let doc = super::event_json(record);
        if let Some(hit) = RankedHit::from_doc(&session_id, record, &doc, &words) {
            ranked.push(hit);
        }
    }
    ranked.sort_by(RankedHit::stronger);

    // 单会话语料代际：事件数 + 最后事件 ts；追加事件即判 Stale。
    let last_ts = records.last().map(|r| r.ts).unwrap_or(0);
    let generation = stable_digest(&format!("session={session_id};events={};last_ts={last_ts}", records.len()));
    let fingerprint = search_fingerprint(query, filter, max_results);
    let offset = validate_cursor(cursor, &fingerprint, &generation)?;

    let total = ranked.len();
    let start = offset.min(total as u64) as usize;
    let end = (start + max_results).min(total);
    let mut lines = vec![format!("Session {session_id} — event search results ({}):", end - start)];
    for (index, hit) in ranked[start..end].iter().enumerate() {
        lines.push(format!(
            "{}. seq {} | {} | {} | matches {}",
            start + index + 1,
            hit.seq,
            hit.ty,
            format_time(hit.ts),
            hit.count,
        ));
        lines.push(format!("   Snippet: {}", hit.text));
    }
    if end < total {
        cursor_footer(&mut lines, end, &fingerprint, &generation);
    }
    Ok(lines.join("\n"))
}

/// `session_trace`：会话概览 + lineage（fork 血缘经 `ForkSeed` 播种标记呈现）。
fn execute_session_trace(dir: &Path, caller: &CallerContext, call: &ToolCall) -> Result<String, String> {
    let session_id = target_id(call, caller);
    let records = load_session(dir, &session_id)?;
    let created_at = session_created_at(&records);
    let mut lines = vec![
        format!("Session {session_id}"),
        format!("Created: {}", format_time(created_at)),
        format!("Events: {}", records.len()),
        String::new(),
    ];
    lines.push(ancestor_line(&records));
    lines.extend(descendant_lines(dir, &session_id));
    Ok(lines.join("\n"))
}

/// 祖先：目标日志中的 `ForkSeed` 播种标记（fork 子会话的源会话与继承边界）。
fn ancestor_line(records: &[EventRecord]) -> String {
    records
        .iter()
        .rev()
        .find_map(|record| match &record.event {
            SessionEvent::ForkSeed { parent_session_id, parent_boundary_seq, inherited_events } => {
                Some(format!(
                    "Ancestors: {parent_session_id} (boundary seq {parent_boundary_seq}, inherited {inherited_events} events)"
                ))
            }
            _ => None,
        })
        .unwrap_or_else(|| "Ancestors: none".to_string())
}

/// 后代：同容器全部会话日志中指向本会话的 `ForkSeed`（fork 子会话与源同容器落盘）。
fn descendant_lines(dir: &Path, session_id: &str) -> Vec<String> {
    let mut descendants: Vec<String> = Vec::new();
    for file in list_session_files(dir) {
        if file.id == session_id {
            continue;
        }
        let Ok(records) = read_session_events(&file.path) else { continue };
        let forked = records.iter().any(|record| match &record.event {
            SessionEvent::ForkSeed { parent_session_id, .. } => parent_session_id == session_id,
            _ => false,
        });
        if forked {
            descendants.push(file.id);
        }
    }
    descendants.sort();
    descendants.dedup();
    if descendants.is_empty() {
        vec!["Descendants: none".to_string()]
    } else {
        let mut lines = vec![format!("Descendants ({}):", descendants.len())];
        lines.extend(
            descendants
                .iter()
                .enumerate()
                .map(|(index, id)| format!("  {}. {id}", index + 1)),
        );
        lines
    }
}

/// `session_event_trace`：目标事件的位置关系（onion 无压缩替换/派生关系）。
fn execute_event_trace(dir: &Path, caller: &CallerContext, call: &ToolCall) -> Result<String, String> {
    let session_id = target_id(call, caller);
    let seq = u64_arg(call, "seq")?.ok_or_else(|| "session_event_trace: missing 'seq'".to_string())?;
    let records = load_session(dir, &session_id)?;
    let target = records
        .iter()
        .find(|r| r.seq == seq)
        .ok_or_else(|| format!("session {session_id} has no event at seq {seq}"))?;
    Ok([
        format!("Session {session_id}"),
        format!("Target: seq {} | {} | {}", target.seq, super::event_type(&target.event), format_time(target.ts)),
        "Replaced by: none".to_string(),
        "Replacement chain: none".to_string(),
        "Events replaced by target: none".to_string(),
        "Events cited directly as sources: none".to_string(),
        "Direct derived events: none".to_string(),
    ]
    .join("\n"))
}

/// `session_event_read`：完整未删节事件（JSON）+ 可选相邻事件摘要。
fn execute_event_read(dir: &Path, caller: &CallerContext, call: &ToolCall) -> Result<String, String> {
    let session_id = target_id(call, caller);
    let seq = u64_arg(call, "seq")?.ok_or_else(|| "session_event_read: missing 'seq'".to_string())?;
    let before = u64_arg(call, "before")?.unwrap_or(0);
    let after = u64_arg(call, "after")?.unwrap_or(0);
    let records = load_session(dir, &session_id)?;
    let target = records
        .iter()
        .find(|r| r.seq == seq)
        .ok_or_else(|| format!("session {session_id} has no event at seq {seq}"))?;
    let mut lines = vec![
        format!("Session {session_id}"),
        format!("Target event seq {seq}:"),
        "```json".to_string(),
        super::event_json(target),
        "```".to_string(),
    ];
    let before_records: Vec<_> = records.iter().filter(|r| r.seq < seq).rev().take(before as usize).collect();
    if !before_records.is_empty() {
        lines.push(String::new());
        lines.push("Before:".to_string());
        for record in before_records.iter().rev() {
            lines.push(format_neighbor(record));
        }
    }
    let after_records: Vec<_> = records.iter().filter(|r| r.seq > seq).take(after as usize).collect();
    if !after_records.is_empty() {
        lines.push(String::new());
        lines.push("After:".to_string());
        for record in after_records {
            lines.push(format_neighbor(record));
        }
    }
    Ok(lines.join("\n"))
}

fn format_neighbor(record: &EventRecord) -> String {
    let text = snippet(record, NEIGHBOR_MAX_CHARS);
    format!(
        "- seq {} | {} | {}\n  {}",
        record.seq,
        super::event_type(&record.event),
        format_time(record.ts),
        text.replace('\n', "\n  "),
    )
}

/// 搜索类工具（session_search / session_event_search）。
struct SearchTool {
    name: &'static str,
    max_results: usize,
    session: Arc<OnceLock<Arc<SessionLog>>>,
    event_scoped: bool,
}

impl ToolPort for SearchTool {
    fn definition(&self) -> ToolDefinition {
        let description = if self.event_scoped {
            "Search prior events in one authorized session; the current session excludes the step performing this call."
        } else {
            "Search prior sessions in the caller workspace and return the strongest matching event from each session."
        };
        let mut properties = json!({
            "query": { "type": "string", "description": "Literal full-text query over prior session history. Words must appear in order, separated by whitespace.", "required": true },
            "cursor": { "type": "string", "description": "Opaque pagination token from a previous \"Next cursor:\" line. Keep all other parameters unchanged when passing it." }
        });
        if self.event_scoped {
            properties["session_id"] =
                json!({ "type": "string", "description": "Target session id. Omit for the current session." });
            properties["seq_from"] = json!({ "type": "integer", "description": "Inclusive event sequence lower bound." });
            properties["seq_to"] = json!({ "type": "integer", "description": "Inclusive event sequence upper bound." });
            properties["time_from"] = json!({ "type": "string", "description": "Inclusive ISO 8601 event-time lower bound (e.g. 2026-01-01T00:00:00Z)." });
            properties["time_to"] = json!({ "type": "string", "description": "Inclusive ISO 8601 event-time upper bound (e.g. 2026-01-01T00:00:00Z)." });
            properties["event_types"] = json!({ "type": "array", "items": { "type": "string" }, "description": "Event types to include." });
            properties["created_at_from"] = json!({ "type": "string", "description": "Inclusive ISO 8601 session-creation lower bound (the session's first SessionStarted event time)." });
            properties["created_at_to"] = json!({ "type": "string", "description": "Inclusive ISO 8601 session-creation upper bound (the session's first SessionStarted event time)." });
        } else {
            properties["session_ids"] = json!({ "type": "array", "items": { "type": "string" }, "description": "Optional session ids to include." });
            properties["event_seq_from"] = json!({ "type": "integer", "description": "Inclusive event sequence lower bound." });
            properties["event_seq_to"] = json!({ "type": "integer", "description": "Inclusive event sequence upper bound." });
            properties["event_time_from"] = json!({ "type": "string", "description": "Inclusive ISO 8601 event-time lower bound (e.g. 2026-01-01T00:00:00Z)." });
            properties["event_time_to"] = json!({ "type": "string", "description": "Inclusive ISO 8601 event-time upper bound (e.g. 2026-01-01T00:00:00Z)." });
            properties["event_types"] = json!({ "type": "array", "items": { "type": "string" }, "description": "Event types to include." });
            properties["created_at_from"] = json!({ "type": "string", "description": "Inclusive ISO 8601 session-creation lower bound (the session's first SessionStarted event time)." });
            properties["created_at_to"] = json!({ "type": "string", "description": "Inclusive ISO 8601 session-creation upper bound (the session's first SessionStarted event time)." });
        }
        let parameters = json!({ "type": "object", "properties": properties, "required": ["query"] });
        ToolDefinition {
            name: self.name.to_string(),
            description: description.to_string(),
            parameters,
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let max_results = self.max_results;
        let event_scoped = self.event_scoped;
        let name = self.name.to_string();
        let caller = self.session.get().map(|session| CallerContext {
            id: session.session_id().to_string(),
            recorded_events: session.snapshot().len() as u64,
        });
        Box::pin(async move {
            let caller = caller.ok_or_else(|| format!("{name} requires an owning agent session"))?;
            let query = str_arg(&call, "query").ok_or_else(|| format!("{name}: missing 'query'"))?;
            let query = normalize_query(&query)?;
            let event_types = strings_arg(&call, "event_types")?;
            let cursor = match call.input.get("cursor") {
                None | Some(serde_json::Value::Null) => None,
                Some(value) => {
                    let token = value
                        .as_str()
                        .ok_or_else(|| "cursor must be a string token".to_string())?;
                    Some(decode_cursor(token)?)
                }
            };
            let dir = crate::paths::sessions_dir(&crate::paths::data_home(), cwd_path());
            let text = if event_scoped {
                let filter = SearchFilter {
                    event_types,
                    time_from: time_arg(&call, "time_from")?,
                    time_to: time_arg(&call, "time_to")?,
                    created_at_from: time_arg(&call, "created_at_from")?,
                    created_at_to: time_arg(&call, "created_at_to")?,
                    ..Default::default()
                };
                execute_event_search(&dir, &caller, &call, &query, max_results, &filter, cursor.as_ref())?
            } else {
                let filter = SearchFilter {
                    session_ids: strings_arg(&call, "session_ids")?,
                    seq_from: u64_arg(&call, "event_seq_from")?,
                    seq_to: u64_arg(&call, "event_seq_to")?,
                    time_from: time_arg(&call, "event_time_from")?,
                    time_to: time_arg(&call, "event_time_to")?,
                    event_types,
                    created_at_from: time_arg(&call, "created_at_from")?,
                    created_at_to: time_arg(&call, "created_at_to")?,
                };
                execute_session_search(&dir, &caller, &query, max_results, &filter, cursor.as_ref())?
            };
            Ok(vec![ContentBlock::Text { text }])
        })
    }
}

/// 精确读取类工具（session_trace / session_event_trace / session_event_read）。
enum ReadKind {
    Trace,
    EventTrace,
    EventRead,
}

struct ReadTool {
    name: &'static str,
    session: Arc<OnceLock<Arc<SessionLog>>>,
    kind: ReadKind,
}

impl ToolPort for ReadTool {
    fn definition(&self) -> ToolDefinition {
        let (description, parameters) = match self.kind {
            ReadKind::Trace => (
                "Read the authorized session lineage around one session, including complete visible ancestor and descendant relationships.",
                json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "Target session id. Omit for the current session." }
                    }
                }),
            ),
            ReadKind::EventTrace => (
                "Read every direct replacement and relationship to a cited source event for one event in an authorized session.",
                json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "Target session id. Omit for the current session." },
                        "seq": { "type": "integer", "description": "Target event sequence number." }
                    },
                    "required": ["seq"]
                }),
            ),
            ReadKind::EventRead => (
                "Read one full unabridged event and optional neighboring raw-event summaries from an authorized session.",
                json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "Target session id. Omit for the current session." },
                        "seq": { "type": "integer", "description": "Target event sequence number." },
                        "before": { "type": "integer", "description": "Number of preceding raw events to summarize. Omit for none." },
                        "after": { "type": "integer", "description": "Number of following raw events to summarize. Omit for none." }
                    },
                    "required": ["seq"]
                }),
            ),
        };
        ToolDefinition {
            name: self.name.to_string(),
            description: description.to_string(),
            parameters,
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let name = self.name.to_string();
        let caller = self.session.get().map(|session| CallerContext {
            id: session.session_id().to_string(),
            recorded_events: session.snapshot().len() as u64,
        });
        let kind = match self.kind {
            ReadKind::Trace => 0,
            ReadKind::EventTrace => 1,
            ReadKind::EventRead => 2,
        };
        Box::pin(async move {
            let caller = caller.ok_or_else(|| format!("{name} requires an owning agent session"))?;
            let dir = crate::paths::sessions_dir(&crate::paths::data_home(), cwd_path());
            let text = match kind {
                0 => execute_session_trace(&dir, &caller, &call)?,
                1 => execute_event_trace(&dir, &caller, &call)?,
                _ => execute_event_read(&dir, &caller, &call)?,
            };
            Ok(vec![ContentBlock::Text { text }])
        })
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
