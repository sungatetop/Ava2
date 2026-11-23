//! workflow 程序模型：声明式 JSON 程序的解析校验 + `{{...}}` 模板替换。
//!
//! 程序形态（模型可见 schema 见 `tools/workflow.rs`）：
//!
//! ```json
//! {
//!   "meta": { "name": "review", "description": "…", "when_to_use": "…",
//!             "phases": [{ "title": "scan", "detail": "…" }] },
//!   "steps": [
//!     { "id": "list", "phase": "scan", "prompt": "List files under {{args.root}}" },
//!     { "id": "read", "phase": "scan", "prompt": "Read {{item}}",
//!       "for_each": "files", "label": "read file" },
//!     { "id": "report", "prompt": "Summarize: {{steps.list}}" }
//!   ],
//!   "concurrency": 4
//! }
//! ```
//!
//! 校验是 fail-closed 的：meta.name / meta.description 必填，steps 非空且 id 唯一，
//! 显式 `phase` 必须在 `meta.phases` 内声明，`for_each` 必须是 `args` 内的数组路径，
//! 模板中的未知/未解析变量直接报错（不静默留下原文，避免子代理拿到畸形 prompt）。

use std::collections::BTreeMap;

use serde_json::Value;

/// 未显式声明并发度时的缺省值。
pub const DEFAULT_CONCURRENCY: usize = 4;
/// 并发度上限（防单步 fan-out 打爆宿主连接数）。
pub const MAX_CONCURRENCY: usize = 16;

/// 阶段声明（仅进度分组，不改变执行顺序）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Phase {
    pub title: String,
    pub detail: Option<String>,
}

/// 程序元信息（`meta`）：模型侧据此理解适用场景。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramMeta {
    pub name: String,
    pub description: String,
    pub when_to_use: Option<String>,
    pub phases: Vec<Phase>,
}

/// 单个步骤：一次子代理派发（或一次 fan-out）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub id: String,
    /// 进度分组标签（`meta.phases` 已声明时须命中其中之一）。
    pub phase: Option<String>,
    /// 子代理 prompt 模板。
    pub prompt: String,
    /// `args` 内的数组路径；声明后逐项并发派发，该步骤产出数组。
    pub for_each: Option<String>,
    /// 展示用标签（缺省回退到 `id`）。
    pub label: Option<String>,
}

/// 声明式 workflow 程序。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub meta: ProgramMeta,
    pub steps: Vec<Step>,
    pub concurrency: usize,
}

impl Program {
    /// 解析并校验程序（错误串直接回给模型，含具体字段定位）。
    pub fn parse(value: &Value) -> Result<Self, String> {
        if !value.is_object() {
            return Err("program must be a JSON object".to_string());
        }
        let meta_value = value
            .get("meta")
            .ok_or_else(|| "program: `meta` is required".to_string())?;
        let meta = Self::parse_meta(meta_value)?;
        let steps_value = value
            .get("steps")
            .and_then(Value::as_array)
            .ok_or_else(|| "program: `steps` must be a non-empty array".to_string())?;
        if steps_value.is_empty() {
            return Err("program: `steps` must be a non-empty array".to_string());
        }
        let mut steps: Vec<Step> = Vec::with_capacity(steps_value.len());
        for (index, step_value) in steps_value.iter().enumerate() {
            let step = Self::parse_step(step_value, index)?;
            if steps.iter().any(|existing| existing.id == step.id) {
                return Err(format!("program: duplicate step id `{}`", step.id));
            }
            if let Some(phase) = &step.phase {
                let declared = meta.phases.iter().any(|p| &p.title == phase);
                if !meta.phases.is_empty() && !declared {
                    return Err(format!(
                        "program: step `{}` uses phase `{phase}` which is not declared in meta.phases",
                        step.id
                    ));
                }
            }
            steps.push(step);
        }
        let concurrency = Self::parse_concurrency(value.get("concurrency"))?;
        Ok(Self { meta, steps, concurrency })
    }

    fn parse_meta(value: &Value) -> Result<ProgramMeta, String> {
        if !value.is_object() {
            return Err("program: `meta` must be an object".to_string());
        }
        let name = required_string(value, "name", "program.meta")?;
        let description = required_string(value, "description", "program.meta")?;
        let when_to_use = optional_string(value, "when_to_use");
        let mut phases = Vec::new();
        if let Some(list) = value.get("phases") {
            let list = list
                .as_array()
                .ok_or_else(|| "program.meta: `phases` must be an array".to_string())?;
            for (index, phase) in list.iter().enumerate() {
                if !phase.is_object() {
                    return Err(format!("program.meta.phases[{index}]: must be an object"));
                }
                let title = required_string(phase, "title", &format!("program.meta.phases[{index}]"))?;
                phases.push(Phase { title, detail: optional_string(phase, "detail") });
            }
        }
        Ok(ProgramMeta { name, description, when_to_use, phases })
    }

    fn parse_step(value: &Value, index: usize) -> Result<Step, String> {
        let at = format!("program.steps[{index}]");
        if !value.is_object() {
            return Err(format!("{at}: must be an object"));
        }
        let id = required_string(value, "id", &at)?;
        if !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(format!(
                "{at}: `id` may only contain letters, digits, underscores and hyphens"
            ));
        }
        let prompt = required_string(value, "prompt", &at)?;
        let phase = optional_string(value, "phase");
        let for_each = optional_string(value, "for_each");
        if let Some(path) = &for_each {
            if path.split('.').any(str::is_empty) {
                return Err(format!("{at}: `for_each` path `{path}` is malformed"));
            }
        }
        Ok(Step { id, phase, prompt, for_each, label: optional_string(value, "label") })
    }

    fn parse_concurrency(value: Option<&Value>) -> Result<usize, String> {
        let Some(value) = value else {
            return Ok(DEFAULT_CONCURRENCY);
        };
        let raw = value
            .as_u64()
            .ok_or_else(|| "program: `concurrency` must be a positive integer".to_string())?;
        if raw == 0 {
            return Err("program: `concurrency` must be a positive integer".to_string());
        }
        // 超上限按上限收敛（模型常给大值；拒绝会让整次编排白跑）。
        Ok((raw as usize).min(MAX_CONCURRENCY))
    }
}

/// 必填非空字符串（去空白后）。
fn required_string(value: &Value, key: &str, at: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{at}: `{key}` must be a non-empty string"))
}

/// 可选非空字符串（空串视为未提供）。
fn optional_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// 模板渲染上下文（一次步骤派发的变量来源）。
pub struct TemplateContext<'a> {
    pub args: &'a Value,
    /// 已完成步骤的输出（`steps.<id>` 数据源）。
    pub outputs: &'a BTreeMap<String, Value>,
    /// `for_each` 当前项（`item` 数据源；非 fan-out 步骤为 None）。
    pub item: Option<&'a Value>,
    /// `for_each` 当前下标（`index` 数据源）。
    pub index: Option<usize>,
}

/// 渲染步骤 prompt（错误串带步骤定位）。
pub fn render_prompt(step: &Step, context: &TemplateContext<'_>) -> Result<String, String> {
    render(&step.prompt, context).map_err(|error| format!("step `{}`: {error}", step.id))
}

/// 渲染一段模板：替换全部 `{{...}}` 占位符。
///
/// 未闭合的 `{{` / 空表达式 / 未知变量 / 未解析路径一律报错（fail-closed）。
pub fn render(template: &str, context: &TemplateContext<'_>) -> Result<String, String> {
    let mut rendered = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        rendered.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            return Err("unterminated template expression (`{{` without `}}`)".to_string());
        };
        let expression = after[..end].trim();
        rendered.push_str(&display_value(&lookup(expression, context)?));
        rest = &after[end + 2..];
    }
    rendered.push_str(rest);
    Ok(rendered)
}

/// 解析单个占位符表达式为 JSON 值。
fn lookup(expression: &str, context: &TemplateContext<'_>) -> Result<Value, String> {
    if expression.is_empty() {
        return Err("empty template expression `{{}}`".to_string());
    }
    if expression == "index" {
        return context
            .index
            .map(Value::from)
            .ok_or_else(|| "`{{index}}` is only available inside a `for_each` step".to_string());
    }
    let mut segments = expression.split('.');
    let head = segments.next().unwrap_or_default();
    let tail: Vec<&str> = segments.collect();
    match head {
        "args" => {
            if tail.is_empty() {
                return Err(
                    "`{{args}}` requires a path, e.g. `{{args.files}}`".to_string(),
                );
            }
            dotted(context.args, &tail).ok_or_else(|| {
                format!("template variable `{{{{{expression}}}}}` does not resolve in `args`")
            })
        }
        "steps" => {
            let Some(step_id) = tail.first().copied() else {
                return Err(
                    "`{{steps}}` requires a step id, e.g. `{{steps.scan}}`".to_string(),
                );
            };
            let output = context.outputs.get(step_id).ok_or_else(|| {
                format!(
                    "template variable `{{{{{expression}}}}}` refers to step `{step_id}` that has \
                     not produced output yet (steps run in order)"
                )
            })?;
            if tail.len() == 1 {
                return Ok(output.clone());
            }
            dotted(output, &tail[1..]).ok_or_else(|| {
                format!("template variable `{{{{{expression}}}}}` does not resolve in step `{step_id}` output")
            })
        }
        "item" => {
            let item = context.item.ok_or_else(|| {
                format!("template variable `{{{{{expression}}}}}` is only available inside a `for_each` step")
            })?;
            if tail.is_empty() {
                return Ok(item.clone());
            }
            dotted(item, &tail).ok_or_else(|| {
                format!("template variable `{{{{{expression}}}}}` does not resolve in the current item")
            })
        }
        _ => Err(format!(
            "unknown template variable `{{{{{expression}}}}}` (use args.*, steps.*, item or index)"
        )),
    }
}

/// 按点路径取值（对象按键、数组按数字下标）。
fn dotted(value: &Value, segments: &[&str]) -> Option<Value> {
    let mut current = value;
    for segment in segments {
        current = match current {
            Value::Object(map) => map.get(*segment)?,
            Value::Array(items) => items.get(segment.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current.clone())
}

/// 值 → 模板文本（字符串原样，其余 JSON 紧凑序列化）。
fn display_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// 解析 `for_each` 路径为数组（路径相对 `args`；允许带 `args.` 前缀）。
pub fn resolve_for_each(path: &str, args: &Value) -> Result<Vec<Value>, String> {
    let segments: Vec<&str> = path.split('.').collect();
    let segments = match segments.first() {
        Some(&"args") => segments[1..].to_vec(),
        _ => segments,
    };
    if segments.is_empty() {
        return Err("`for_each` requires an array path inside `args`".to_string());
    }
    let value = dotted(args, &segments)
        .ok_or_else(|| format!("`for_each` path `{path}` does not resolve in `args`"))?;
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(format!("`for_each` path `{path}` must resolve to an array")),
    }
}
