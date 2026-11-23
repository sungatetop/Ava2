//! `workflow::program` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::program::*;

fn minimal() -> Value {
    json!({
        "meta": { "name": "demo", "description": "demo program" },
        "steps": [{ "id": "one", "prompt": "do it" }]
    })
}

#[test]
fn parse_accepts_minimal_program_with_default_concurrency() {
    let program = Program::parse(&minimal()).unwrap();
    assert_eq!(program.meta.name, "demo");
    assert_eq!(program.meta.when_to_use, None);
    assert!(program.meta.phases.is_empty());
    assert_eq!(program.concurrency, DEFAULT_CONCURRENCY);
    assert_eq!(
        program.steps,
        vec![Step {
            id: "one".to_string(),
            phase: None,
            prompt: "do it".to_string(),
            for_each: None,
            label: None,
        }]
    );
}

#[test]
fn parse_rejects_missing_meta_and_empty_steps() {
    assert!(Program::parse(&json!({})).unwrap_err().contains("`meta` is required"));
    let empty_steps = json!({
        "meta": { "name": "x", "description": "y" },
        "steps": []
    });
    assert!(Program::parse(&empty_steps).unwrap_err().contains("non-empty array"));
}

#[test]
fn parse_rejects_duplicate_step_ids() {
    let value = json!({
        "meta": { "name": "x", "description": "y" },
        "steps": [
            { "id": "dup", "prompt": "a" },
            { "id": "dup", "prompt": "b" }
        ]
    });
    assert!(Program::parse(&value).unwrap_err().contains("duplicate step id"));
}

#[test]
fn parse_rejects_undeclared_phase() {
    let value = json!({
        "meta": {
            "name": "x",
            "description": "y",
            "phases": [{ "title": "scan", "detail": "read everything" }]
        },
        "steps": [{ "id": "one", "phase": "write", "prompt": "a" }]
    });
    let error = Program::parse(&value).unwrap_err();
    assert!(error.contains("phase `write`"));
    assert!(error.contains("not declared"));
}

#[test]
fn parse_clamps_concurrency_to_ceiling() {
    let mut value = minimal();
    value["concurrency"] = json!(99);
    assert_eq!(Program::parse(&value).unwrap().concurrency, MAX_CONCURRENCY);
    value["concurrency"] = json!(0);
    assert!(Program::parse(&value).unwrap_err().contains("positive integer"));
}

#[test]
fn parse_rejects_bad_step_id_characters() {
    let value = json!({
        "meta": { "name": "x", "description": "y" },
        "steps": [{ "id": "bad id", "prompt": "a" }]
    });
    assert!(Program::parse(&value).unwrap_err().contains("only contain letters"));
}

#[test]
fn render_substitutes_args_steps_item_and_index() {
    let args = json!({ "root": "/tmp", "files": ["a.txt", "b.txt"] });
    let mut outputs = BTreeMap::new();
    outputs.insert("scan".to_string(), json!("scanned"));
    outputs.insert("rows".to_string(), json!([{ "name": "n0" }]));
    let item = json!({ "name": "alpha" });
    let context = TemplateContext {
        args: &args,
        outputs: &outputs,
        item: Some(&item),
        index: Some(1),
    };
    let rendered = render(
        "root={{args.root}} file={{args.files.0}} prev={{steps.scan}} row={{steps.rows.0.name}} \
         item={{item.name}} idx={{index}}",
        &context,
    )
    .unwrap();
    assert_eq!(
        rendered,
        "root=/tmp file=a.txt prev=scanned row=n0 item=alpha idx=1"
    );
}

#[test]
fn render_reports_unresolved_and_unknown_variables() {
    let args = json!({});
    let outputs = BTreeMap::new();
    let context = TemplateContext { args: &args, outputs: &outputs, item: None, index: None };
    assert!(render("{{args.missing}}", &context).unwrap_err().contains("does not resolve"));
    assert!(render("{{steps.later}}", &context).unwrap_err().contains("has not produced output yet"));
    assert!(render("{{item}}", &context).unwrap_err().contains("only available inside"));
    assert!(render("{{index}}", &context).unwrap_err().contains("only available inside"));
    assert!(render("{{bogus.x}}", &context).unwrap_err().contains("unknown template variable"));
    assert!(render("{{args}}", &context).unwrap_err().contains("requires a path"));
    assert!(render("trailing {{args.x", &context).unwrap_err().contains("unterminated"));
    assert!(render("{{}}", &context).unwrap_err().contains("empty template expression"));
}

#[test]
fn render_prompt_prefixes_step_id_on_error() {
    let args = json!({});
    let outputs = BTreeMap::new();
    let step = Step {
        id: "broken".to_string(),
        phase: None,
        prompt: "{{args.nope}}".to_string(),
        for_each: None,
        label: None,
    };
    let context = TemplateContext { args: &args, outputs: &outputs, item: None, index: None };
    assert!(render_prompt(&step, &context).unwrap_err().starts_with("step `broken`:"));
}

#[test]
fn resolve_for_each_reads_array_paths() {
    let args = json!({ "files": ["a", "b"], "nested": { "rows": [1, 2, 3] } });
    assert_eq!(resolve_for_each("files", &args).unwrap().len(), 2);
    assert_eq!(resolve_for_each("args.files", &args).unwrap().len(), 2);
    assert_eq!(resolve_for_each("nested.rows", &args).unwrap().len(), 3);
    assert!(resolve_for_each("files.x", &args).unwrap_err().contains("does not resolve"));
    assert!(resolve_for_each("nested", &args).unwrap_err().contains("must resolve to an array"));
}
