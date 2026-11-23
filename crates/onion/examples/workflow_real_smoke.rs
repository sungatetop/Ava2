//! workflow 编排真实 API 冒烟。
//!
//! 验证四件事（真实模型调用，走 `Config::from_env`，缺省 local 供应商）：
//! 1. 声明式程序的两个步骤按序执行（fan-out 步骤先跑，汇总步骤能引用其输出）；
//! 2. `for_each` 逐项派发子代理（并发度受 `concurrency` 限流），产出数组；
//! 3. 子代理真实产出进入最终 value（非空文本）；
//! 4. 进度回调按 phase / step 粒度产出可读行（后台 job 的输出环数据源）。
//!
//! 运行：
//! ```powershell
//! cargo run -p onion --example workflow_real_smoke
//! ```

use std::sync::{Arc, Mutex};
use std::time::Duration;

use onion::config::Config;
use onion::llm::OpenAiSseLlm;
use onion::subagent::runtime::{ChildSession, SubagentRuntime};
use onion::tools::ToolRegistry;
use onion::workflow::{self, Program, Progress, StopReason};
use serde_json::json;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[tokio::main]
async fn main() {
    let mut config = Config::from_env();
    config.include_identity = false;
    config.workspace_enabled = false;
    println!("[smoke] provider {} model {}", config.base_url, config.model);

    let factory_config = config.clone();
    let runtime = Arc::new(SubagentRuntime::new(
        1,
        config.clone(),
        Arc::new(OpenAiSseLlm::new(config.base_url.clone(), config.api_key.clone())),
        Arc::new(move |_cwd: Option<&str>, _preset: Option<&str>| {
            // 子代理工具面留空：本冒烟只验证编排链路，不让子代理再调工具。
            Ok(ChildSession::new(&factory_config, ToolRegistry::new()))
        }),
        None,
    ));

    let program_value = json!({
        "meta": {
            "name": "real-smoke",
            "description": "fan out over items, then summarize",
            "when_to_use": "smoke test only",
            "phases": [
                { "title": "scan", "detail": "one subagent per item" },
                { "title": "report", "detail": "single summary subagent" }
            ]
        },
        "steps": [
            {
                "id": "scan",
                "phase": "scan",
                "prompt": "Reply with the single word OK and nothing else. The item is: {{item}}",
                "for_each": "items",
                "label": "scan item"
            },
            {
                "id": "report",
                "phase": "report",
                "prompt": "Reply with the single word DONE and nothing else. Prior outputs: {{steps.scan}}"
            }
        ],
        "concurrency": 2
    });
    let program = match Program::parse(&program_value) {
        Ok(program) => program,
        Err(error) => {
            println!("[smoke] FAIL 程序解析失败：{error}");
            std::process::exit(1);
        }
    };
    let args = json!({ "items": ["alpha", "beta"] });

    let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = lines.clone();
    let progress: Progress = Arc::new(move |line: &str| {
        println!("[smoke] progress: {line}");
        lock(&sink).push(line.to_string());
    });

    println!("[smoke] 执行 workflow（真实模型，2 项 fan-out + 1 汇总）…");
    let report = match tokio::time::timeout(
        Duration::from_secs(300),
        workflow::run(
            runtime,
            &program,
            &args,
            onion::CancellationToken::new(),
            Some(progress),
        ),
    )
    .await
    {
        Ok(report) => report,
        Err(_) => {
            println!("[smoke] FAIL 编排超时（300s）");
            std::process::exit(1);
        }
    };

    println!(
        "[smoke] stop={} agents={} value={}",
        report.stop_reason.as_str(),
        report.agents_started,
        serde_json::to_string_pretty(&report.value).unwrap_or_default()
    );

    let mut failures: Vec<String> = Vec::new();
    if report.stop_reason != StopReason::Completed {
        failures.push(format!(
            "编排未完成：{} ({:?})",
            report.stop_reason.as_str(),
            report.error
        ));
    }
    if report.agents_started != 3 {
        failures.push(format!("派发子代理数应为 3，实际 {}", report.agents_started));
    }
    match report.value.get("scan").and_then(|value| value.as_array()) {
        Some(items) => {
            if items.len() != 2 {
                failures.push(format!("fan-out 数组长度应为 2，实际 {}", items.len()));
            }
            if items.iter().any(|item| item.as_str().unwrap_or_default().trim().is_empty()) {
                failures.push("fan-out 存在空输出".to_string());
            }
            if items.iter().any(serde_json::Value::is_null) {
                failures.push("fan-out 存在 null 输出（子代理失败）".to_string());
            }
        }
        None => failures.push("缺少 scan 步骤的数组输出".to_string()),
    }
    match report.value.get("report").and_then(|value| value.as_str()) {
        Some(text) if !text.trim().is_empty() => {
            if !text.to_uppercase().contains("DONE") {
                failures.push(format!("汇总子代理未按指令回复 DONE：{text}"));
            }
        }
        _ => failures.push("缺少 report 步骤的文本输出".to_string()),
    }
    let lines = lock(&lines).clone();
    if !lines.iter().any(|line| line == "phase: scan — one subagent per item") {
        failures.push("进度回调缺少 phase 行".to_string());
    }
    if !lines.iter().any(|line| line.contains("fan-out over 2 item(s)")) {
        failures.push("进度回调缺少 fan-out 行".to_string());
    }

    if failures.is_empty() {
        println!("\n[smoke] PASS：workflow 串行步骤 + 并发 fan-out + 模板引用 + 进度回调 全部通过");
    } else {
        println!("\n[smoke] FAIL：");
        for failure in &failures {
            println!("  - {failure}");
        }
        std::process::exit(1);
    }
}
