//! onion-tui：全屏终端界面（TUI front door）。
//!
//! Phase B 接线：输入编辑器（idle 开轮 / running 排队）+ turn 驱动 + 双态
//! footer + 问题面板。
//!
//! - **按键路由**：overlay 打开 → 选择器状态机；Ctrl+M → 打开 /model 选择器；
//!   Esc / Ctrl+C → 退出；其余 → 编辑器（Enter 提交行）。
//! - **turn 驱动**：所有外部输入经 inbox 投递，由 turn 循环以空轮消费
//!   （对齐 onion-cli `spawn_turn`）；反馈经通道回主循环
//!   渲染，不 `println!`（全屏画面被污染）。
//! - **帧叠加序**：transcript 投影 → 一次性反馈 → overlay → prompt → footer。

mod banner;
mod commands;
mod components;
mod editor;
mod events;
mod footer;
mod frame;
mod model_picker;
mod prompts;
mod provider_form;
mod renderer;
mod status;
mod terminal;
mod transcript;
mod turn;
mod view_state;

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod snapshot_tests;

#[cfg(test)]
#[path = "main_tests.rs"]
mod main_tests;

use std::sync::Arc;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use onion::ask_user::types::AskUserQuestion;
use onion::assemble::{assemble_runtime, FrontDoor};
use onion::runtime::Runtime;
use tokio::sync::mpsc;

use crate::commands::{Action, Commands, FeedbackRing, Switch};
use crate::editor::Editor;
use crate::events::EventPump;
use crate::footer::Footer;
use crate::frame::{Frame, FrameInput, ScrollIntent, ScrollStep, assemble};
use crate::model_picker::{ModelPicker, PickerState};
use crate::prompts::PromptPanel;
use crate::provider_form::{FormKind, FormResult, ProviderForm};
use crate::renderer::{CrosstermBackend, Line};
use crate::terminal::Terminal;
use crate::transcript::Transcript;
use crate::turn::TurnDriver;
use crate::view_state::ViewState;

/// 问题面板展示切片：答复句柄由 `Commands` 持有（`/approve` `/deny` `/answer`
/// 消费），此处只留可渲染内容，两者生命周期解耦。
enum Panel {
    Approval { name: String, input: serde_json::Value },
    PlanReview { plan: String },
    AskUser { questions: Vec<AskUserQuestion> },
}

/// 终端输入事件（按键 + 鼠标滚轮统一入口）。
#[derive(Debug)]
enum UiEvent {
    Key(KeyEvent),
    /// 滚轮一格（alternate screen 无 scrollback，滚轮转视口滚动）。
    Wheel { up: bool },
}

/// 交互面状态：选择器 + 编辑器 + 问题面板 + 当前 model。
///
/// 收成一个可整体借用的单元：`redraw` 只读借用它，按键分支再逐字段可变，
/// 避免「长期持有 `&` 引用 + 循环内 `&mut`」的借用冲突。
struct Ui {
    picker: ModelPicker,
    editor: Editor,
    panel: PromptPanel,
    active: Option<Panel>,
    /// /provider 表单 overlay（关闭态时忽略）。
    form: ProviderForm,
    /// 运行期目录（/provider 写回后刷新；/model 选择器数据源）。
    catalog: onion::provider_catalog::ProviderCatalog,
    /// 当前 model（footer idle 态展示；`/model` 切换后随之更新）。
    model: String,
    /// 启动工作目录（banner 展示快照）。
    cwd: String,
    /// 最近一轮报告（turn, steps）：footer 轮次统计（TurnReport 驱动）。
    turn_stats: Option<(u32, u32)>,
    /// 视图级开关（§7.3：推理显隐 / 工具卡片展开）。
    view: ViewState,
    /// 视口冻结顶（内容段行索引；None = 跟随底部，新内容自动贴底）。
    /// 回看历史（PgUp/PgDn/滚轮）时冻结，新内容不推移窗口；触底自动解除。
    scroll_top: Option<usize>,
    /// 本帧滚动意图（按键/滚轮产出，redraw 结算后清空）。
    scroll_intent: Option<ScrollIntent>,
}

impl Ui {
    fn new(catalog: &onion::provider_catalog::ProviderCatalog, model: String, cwd: String) -> Self {
        Self {
            picker: ModelPicker::from_catalog(catalog),
            editor: Editor::new(),
            panel: PromptPanel::new(),
            active: None,
            form: ProviderForm::closed(),
            catalog: catalog.clone(),
            model,
            cwd,
            turn_stats: None,
            view: ViewState::default(),
            scroll_top: None,
            scroll_intent: None,
        }
    }

    /// /provider 写回后：刷新目录快照 + 重建 /model 选择器。
    fn refresh_catalog(&mut self, catalog: onion::provider_catalog::ProviderCatalog) {
        self.catalog = catalog.clone();
        self.picker = ModelPicker::from_catalog(&catalog);
    }
}

/// 按键路由结果（主循环消费）。
#[derive(Debug, PartialEq, Eq)]
enum Routed {
    /// 无驱动动作，仅重绘。
    Redraw,
    /// 编辑器提交一行（斜杠命令 / 普通消息）→ 经 `Commands::dispatch` 分发。
    Submit(String),
    /// 选择器确认了一个 model → 经 `Commands::dispatch("/model <id>")` 分发。
    Picked(String),
    /// /provider 表单提交 → main 写回 providers.json + 刷新目录。
    FormSubmitted(FormResult),
    /// 取消活动轮（`CancellationToken`）；无活动轮时视为退出请求。
    Cancel,
    /// 视图级清空 transcript（durable 日志不动）。
    ClearTranscript,
    /// 退出（restore 终端）。
    Quit,
}

/// 按键路由（设计文档 §7.1 / §7.3）：
/// overlay 打开 → 选择器状态机（按键不落到编辑器）；`Ctrl+M` → 开选择器；
/// `Ctrl+R`/`Ctrl+E`/`Ctrl+L`/`Ctrl+U` → 视图级开关；`Esc` → 取消活动轮
/// （空闲时退出）；`Ctrl+C` → 取消活动轮（空闲时退出）；其余 → 编辑器
/// （`Enter` 提交行）。
fn route_key(key: KeyEvent, ui: &mut Ui, busy: bool) -> Routed {
    // /provider 表单打开：按键落到表单状态机（不落编辑器/选择器）。
    if ui.form.is_open() {
        return match ui.form.handle_key(key) {
            Some(result) => Routed::FormSubmitted(result),
            None => Routed::Redraw,
        };
    }
    if !matches!(ui.picker.state(), PickerState::Closed) {
        return match ui.picker.handle_key(key) {
            Some(picked) => Routed::Picked(picked),
            None => Routed::Redraw,
        };
    }
    if key.modifiers == KeyModifiers::CONTROL {
        match key.code {
            // 开 /model 选择器。
            KeyCode::Char('m') => {
                ui.picker.open(&ui.model);
                return Routed::Redraw;
            }
            // 推理显隐切换。
            KeyCode::Char('r') => {
                ui.view.toggle_thinking();
                return Routed::Redraw;
            }
            // 工具卡片统一展开 / 折叠。
            KeyCode::Char('e') => {
                ui.view.toggle_tools();
                return Routed::Redraw;
            }
            // 强制重绘（本循环末尾本就无条件 redraw，此处显式返回以表意）。
            KeyCode::Char('l') => return Routed::Redraw,
            // 视图级清空 transcript。
            KeyCode::Char('u') => return Routed::ClearTranscript,
            // Ctrl+C：活动轮优先取消，空闲才退出。
            KeyCode::Char('c') => {
                return if busy { Routed::Cancel } else { Routed::Quit };
            }
            _ => {}
        }
    }
    // Esc：overlay 已在上面短路；此处活动轮取消，空闲退出。
    if matches!(key, KeyEvent { code: KeyCode::Esc, .. }) {
        return if busy { Routed::Cancel } else { Routed::Quit };
    }
    // PgUp/PgDn：回看历史（编辑器不消费这两个键；意图由 assemble 结算）。
    match key.code {
        KeyCode::PageUp => {
            ui.scroll_intent = Some(ScrollIntent::Up(ScrollStep::Page));
            return Routed::Redraw;
        }
        KeyCode::PageDown => {
            ui.scroll_intent = Some(ScrollIntent::Down(ScrollStep::Page));
            return Routed::Redraw;
        }
        _ => {}
    }
    match ui.editor.handle_key(key) {
        Some(line) => Routed::Submit(line),
        None => Routed::Redraw,
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // TTY gate（fail-loud）：非 TTY 环境报错退出，不静默降级。
    terminal::require_tty()?;

    let config = onion::config::Config::from_env();
    let root = std::env::current_dir()?;

    // color:false：NO_COLOR 非空即关样式（纯文本渲染，净化照常）。
    let mut backend = CrosstermBackend::new();
    if !CrosstermBackend::color_from_env() {
        backend = backend.without_color();
    }
    let mut term = Terminal::enter()?;
    terminal::install_panic_hook();

    // panic 注入点（仅供 tests/pty_acceptance.rs 验证 panic hook 恢复路径；
    // 正常使用不受影响）。
    if std::env::var("ONION_TUI_PANIC").is_ok() {
        panic!("onion-tui: injected panic (ONION_TUI_PANIC)");
    }

    // crossterm 同步事件 → 专用线程 → mpsc 通道（阻塞 read 不阻塞 select!）。
    // 线程跨会话存活：会话切换只重建会话级状态，终端所有权不变。
    let (key_tx, mut key_rx) = mpsc::unbounded_channel::<UiEvent>();
    std::thread::spawn(move || loop {
        match crossterm::event::read() {
            Ok(Event::Key(key)) => {
                // Windows（win32-input-mode）keydown/keyup 各发一事件：不过滤
                // KeyEventKind 会让每个字符插入两次（IME 输入"你好"→"你你好好"）。
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if key_tx.send(UiEvent::Key(key)).is_err() {
                    break;
                }
            }
            Ok(Event::Mouse(mouse)) => {
                // 滚轮 → 视口滚动意图（一格 3 行，对齐常见终端习惯）；
                // 其余鼠标事件（移动/点击）不消费。
                let up = match mouse.kind {
                    crossterm::event::MouseEventKind::ScrollUp => true,
                    crossterm::event::MouseEventKind::ScrollDown => false,
                    _ => continue,
                };
                if key_tx.send(UiEvent::Wheel { up }).is_err() {
                    break;
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    });

    // 外层会话循环：/new 或 /resume 切换会话 = 整体重建装配（对齐 onion-cli：
    // 先收尾当前会话，再经完整启动路径恢复目标会话）。
    let mut next_session: Option<String> = None;
    loop {
        let assembled = match assemble_runtime(&root, &config, next_session.as_deref(), FrontDoor::Tui) {
            Ok(assembled) => assembled,
            Err(error) => {
                // 装配失败不静默退出：渲染一行并回到上一会话（首次装配则退出）。
                let page = [
                    Line::plain(format!("（会话装配失败：{error}）")),
                    Line::plain("按 Esc 退出"),
                ];
                let _ = backend.render_frame(&page, None);
                if next_session.is_none() {
                    term.restore()?;
                    return Err(error);
                }
                next_session = None;
                continue;
            }
        };
        let (mut commands, mut channels) = Commands::new(assembled);
        let runtime: Arc<Runtime> = commands.runtime.clone();
        let budget = commands.context_window_tokens;

        // 会话级交互面：编辑器缓冲与历史随会话重建（跨会话不共享输入草稿）。
        let mut ui = Ui::new(
            &commands.provider_catalog,
            current_model_of(&commands),
            root.to_string_lossy().to_string(),
        );
        let mut feedback = FeedbackRing::new();

        // turn 驱动：反馈与轮报告经通道回主循环（TUI 侧不 println!）。
        let (feedback_tx, mut feedback_rx) = mpsc::unbounded_channel::<String>();
        let (done_tx, mut done_rx) = mpsc::unbounded_channel::<onion::kernel::TurnReport>();
        let driver = Arc::new(TurnDriver::new(
            runtime.clone(),
            commands.busy.clone(),
            commands.goal_driver.clone(),
            commands.plan_mode.clone(),
            feedback_tx,
            done_tx,
        ));

        // 订阅 live 总线（装配后、订阅后 replay，避免丢 durable 事件）。
        let mut bus_rx = runtime.session().bus().subscribe();
        let mut pump = EventPump::new(runtime.clone());
        pump.replay();
        redraw(&mut backend, &pump, &feedback, &mut ui, &driver, budget);

        // 帧调度（对齐 ratatui/codex 的帧合流）：高频事件（流式 delta / todo /
        // 工具卡）只置 dirty，由 33ms tick 统一出帧——增量平滑且避免每 delta
        // 一次终端写入；按键事件即时出帧（输入延迟敏感）。
        let mut ticker = tokio::time::interval(std::time::Duration::from_millis(33));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut dirty = false;

        // 内层循环 break = 会话切换（携带目标）；None = 退出 TUI。
        let switch: Option<Switch> = loop {
            tokio::select! {
                ev = key_rx.recv() => {
                    let Some(ev) = ev else { break None };
                    match ev {
                        // 滚轮：一格 3 行的滚动意图（不落编辑器）。
                        UiEvent::Wheel { up } => {
                            ui.scroll_intent = Some(if up {
                                ScrollIntent::Up(ScrollStep::Lines(3))
                            } else {
                                ScrollIntent::Down(ScrollStep::Lines(3))
                            });
                        }
                        UiEvent::Key(key) => {
                            let line = match route_key(key, &mut ui, driver.busy()) {
                                Routed::Redraw => None,
                                Routed::Quit => break None,
                                Routed::Cancel => {
                                    runtime.cancel_handle().cancel();
                                    feedback.push("（已请求中断当前轮）".to_string());
                                    None
                                }
                                Routed::ClearTranscript => {
                                    pump.clear_transcript();
                                    None
                                }
                                Routed::Picked(picked) => Some(format!("/model {picked}")),
                                Routed::FormSubmitted(result) => {
                                    // /provider 表单提交：写回 providers.json + 刷新目录。
                                    apply_provider_form(&mut ui, commands.provider_catalog.clone(), result, &mut feedback);
                                    None
                                }
                                Routed::Submit(line) => Some(line),
                            };
                            if let Some(line) = line {
                                let (fb, action) = commands.dispatch(&line).await;
                                feedback.extend(fb);
                                ui.model = current_model_of(&commands);
                                // 提交即回底：新轮内容从跟随态展示。
                                ui.scroll_top = None;
                                ui.scroll_intent = None;
                                match action {
                                    Action::Quit => break None,
                                    Action::Kick => driver.kick(),
                                    Action::OpenModelPicker => ui.picker.open(&ui.model),
                                    Action::OpenProviderForm(kind) => {
                                        ui.form = match kind {
                                            FormKind::Provider => ProviderForm::open_provider(),
                                            FormKind::Model => ProviderForm::open_model(default_provider_of(&commands)),
                                        };
                                    }
                                    Action::Continue => {}
                                }
                                // /approve /deny /answer 已消费句柄 → 面板随之收起。
                                if !commands.has_pending() {
                                    ui.active = None;
                                }
                            }
                        }
                    }
                    redraw(&mut backend, &pump, &feedback, &mut ui, &driver, budget);
                    dirty = false;
                }
                _ = ticker.tick() => {
                    if dirty {
                        dirty = false;
                        redraw(&mut backend, &pump, &feedback, &mut ui, &driver, budget);
                    }
                }
                live = bus_rx.recv() => {
                    match live {
                        Ok(event) => pump.handle(event),
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break None,
                    }
                    dirty = true;
                }
                // turn 驱动的反馈行（下一轮开轮 / goal 续轮 / 启动失败等）。
                Some(line) = feedback_rx.recv() => {
                    feedback.push(line);
                    dirty = true;
                }
                // 轮报告：只更新 footer 轮次统计（[turn] 摘要行已移除，画面清爽）。
                Some(report) = done_rx.recv() => {
                    ui.turn_stats = Some((report.turn, report.steps));
                    dirty = true;
                }
                // 工具审批请求（内核挂起等待人工复决 → /approve /deny 答复）。
                Some(request) = channels.approval_rx.recv() => {
                    let onion::listeners::ApprovalRequest { name, input, reply, .. } = request;
                    commands.pending_approval = Some(reply);
                    ui.active = Some(Panel::Approval { name, input });
                    dirty = true;
                }
                // plan-review 请求（exit_plan_mode 提交审批 → /approve /deny 答复）。
                Some(request) = channels.plan_review_rx.recv() => {
                    let onion::plan::service::PlanReviewRequest { plan, reply } = request;
                    commands.pending_plan_review = Some(reply);
                    ui.active = Some(Panel::PlanReview { plan });
                    dirty = true;
                }
                // ask_user 问询（阻塞工具等待人工答复 → /answer 答复）。
                Some(request) = channels.ask_user_rx.recv() => {
                    let onion::ask_user::service::AskUserRequest { questions, reply, call_id, .. } = request;
                    commands.pending_ask_user = Some(crate::commands::PendingAskUser {
                        reply,
                        asked: questions.clone(),
                        questions: questions.clone(),
                        answered: Vec::new(),
                        call_id,
                    });
                    ui.active = Some(Panel::AskUser { questions });
                    dirty = true;
                }
            }

            // 会话切换预检已在 Commands 内完成（busy / subagent / 日志存在）→
            // 此处统一收尾当前会话（对齐 onion-cli：cancel hooks → end）。
            if let Some(target) = commands.switch.take() {
                if let Some(cancel) = &commands.hook_cancel {
                    cancel.cancel();
                }
                let _ = runtime.end(switch_reason(&target)).await;
                break Some(target);
            }
        };

        match switch {
            // 退出 TUI：收尾后交给外层 restore。
            None => break,
            Some(target) => next_session = next_session_of(&target),
        }
    }

    term.restore()?;
    Ok(())
}

/// 切换目标 → 下一次装配的会话 id（None = 新建空会话）。
fn next_session_of(target: &Switch) -> Option<String> {
    match target {
        Switch::New => None,
        Switch::Resume(id) => Some(id.clone()),
    }
}

/// 会话收尾的 durable 原因（`Runtime::end` 的 reason，对齐 onion-cli）。
fn switch_reason(target: &Switch) -> &'static str {
    match target {
        Switch::New => "switch new",
        Switch::Resume(_) => "switch resume",
    }
}

/// 当前 model 名（footer idle 态展示；未绑定时为空）。
fn current_model_of(commands: &Commands) -> String {
    commands
        .model_target
        .current()
        .map(|t| t.model)
        .unwrap_or_default()
}

/// /provider 模型表单的缺省 provider_id：当前 model 所属 provider（找不到留空）。
fn default_provider_of(commands: &Commands) -> String {
    let model = current_model_of(commands);
    if model.is_empty() {
        return String::new();
    }
    commands
        .provider_catalog
        .providers
        .iter()
        .find(|p| p.models.iter().any(|m| m.id == model))
        .map(|p| p.id.clone())
        .unwrap_or_default()
}

/// 应用 /provider 表单提交：变更目录 → 写回 providers.json → 刷新 Ui 目录。
/// 写回失败仅反馈（目录仍内存生效，不阻塞）。
fn apply_provider_form(
    ui: &mut Ui,
    mut catalog: onion::provider_catalog::ProviderCatalog,
    result: FormResult,
    feedback: &mut FeedbackRing,
) {
    let (message, replaced) = match result {
        FormResult::AddProvider(id, name, base_url, api_key, models) => {
            if id.is_empty() || name.is_empty() || base_url.is_empty() {
                feedback.push("（供应商 id / name / base_url 不能为空，未保存）".to_string());
                return;
            }
            let replaced = catalog.add_provider(id.clone(), name, base_url, api_key);
            // models 字段：逗号分隔模型 id，逐个追加（空字段跳过）。
            for mid in models.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                catalog.add_model(&id, mid.to_string(), mid.to_string(), None);
            }
            let model_count = models.split(',').map(str::trim).filter(|s| !s.is_empty()).count();
            let verb = if replaced { "替换" } else { "新增" };
            (
                format!("（已{verb}供应商 {id}（{model_count} 个模型）并写回 providers.json）"),
                replaced,
            )
        }
        FormResult::AddModel(provider_id, id, name, description) => {
            if provider_id.is_empty() || id.is_empty() {
                feedback.push("（模型 provider_id / id 不能为空，未保存）".to_string());
                return;
            }
            let replaced = catalog.add_model(
                &provider_id,
                id.clone(),
                if name.is_empty() { id.clone() } else { name },
                if description.is_empty() { None } else { Some(description) },
            );
            let verb = if replaced { "替换" } else { "新增" };
            (format!("（已{verb}模型 {provider_id}/{id} 并写回 providers.json）"), replaced)
        }
    };
    let _ = replaced;
    match catalog.save(&onion::paths::data_home()) {
        Ok(path) => feedback.push(format!("{message} → {}", path.display())),
        Err(error) => feedback.push(format!("（{message}，但写盘失败：{error}）")),
    }
    ui.refresh_catalog(catalog);
}

/// overlay 行集：/model 选择器 + /provider 表单 + 问题面板（有待答复请求时呈现）。
/// 携带样式（Vec<Line> 直通 assemble，红框卡样式不降级）。
fn overlay_lines(ui: &Ui) -> Vec<crate::renderer::Line> {
    let mut overlay = ui.picker.render();
    overlay.extend(ui.form.render());
    overlay.extend(match &ui.active {
        Some(Panel::Approval { name, input }) => ui.panel.render_approval(name, input),
        Some(Panel::PlanReview { plan }) => ui.panel.render_plan_review(plan),
        Some(Panel::AskUser { questions }) => ui.panel.render_ask_user(questions),
        None => Vec::new(),
    });
    overlay
}

/// 一轮的呈现态（footer 双态 + 流式/思考指示）。
///
/// 三者同源于「轮是否在飞」，绑定成结构既让 `compose` 的参数保持在 lint
/// 上限内，也避免调用方各自拼出不一致的组合。
struct TurnView {
    /// footer 双态（idle / running）。
    state: Footer,
    /// 轮在飞（流式输出中）。
    streaming: bool,
    /// Thinking 状态行数据（elapsed 秒，流式字符数）。
    thinking: Option<(u64, usize)>,
}

/// 组装一帧（纯函数，便于快照测试）。
///
/// 叠加序：transcript 投影 → 一次性反馈 → overlay（选择器 + 问题面板）→
/// prompt → 命令提示（缓冲以 `/` 开头）→ footer。
///
/// 滚动语义：overlay / 面板 / 选择器 / 表单 / 命令提示激活时不冻结视口
/// （这些内容贴底展示，冻结会把它推出屏幕外）；提交一行即回底（主循环侧）。
fn compose(
    transcript: &Transcript,
    feedback: &FeedbackRing,
    ui: &Ui,
    turn: &TurnView,
    budget: u64,
    width: u16,
    height: u16,
) -> Frame {
    let overlay = overlay_lines(ui);
    let feedback_lines: Vec<String> = feedback.lines().cloned().collect();
    // 命令提示：仅空闲交互面（选择器 / 面板未打开）且缓冲以 `/` 开头时展示，
    // 避免与 overlay 叠加混淆。
    let buffer = ui.editor.line().strip_prefix("> ").unwrap_or("").to_string();
    let hints = match ui.active {
        None if matches!(ui.picker.state(), PickerState::Closed) && !ui.form.is_open() => {
            crate::commands::command_hints(&buffer)
        }
        _ => Vec::new(),
    };
    // footer：右下角状态行（右对齐，右侧留 1 列防换行）+ model 名红色 accent。
    let mut footer_text = turn.state.render(transcript, &ui.model, budget, ui.turn_stats);
    // 冻结回看时 footer 前缀滚动状态提示（可发现性：告知 PgDn 回底）。
    if ui.scroll_top.is_some() {
        footer_text = format!("↑ 回看中 PgDn 回底 · {footer_text}");
    }
    let pad = width
        .saturating_sub(1)
        .saturating_sub(crate::frame::display_width(&footer_text) as u16);
    let footer_text = format!("{}{footer_text}", " ".repeat(pad as usize));
    let footer = match footer_text.find(&ui.model) {
        Some(start) if !ui.model.is_empty() => Line::styled(
            footer_text,
            crate::renderer::Style::new(),
        )
        .accent(start, start + ui.model.len(), crossterm::style::Color::Red),
        _ => Line::plain(footer_text),
    };
    // overlay / 面板 / 提示菜单激活 → 交出冻结态（这些内容贴底，须保持跟随）。
    let scroll_free = ui.active.is_some()
        || !matches!(ui.picker.state(), PickerState::Closed)
        || ui.form.is_open()
        || !hints.is_empty();
    let (scroll_top, scroll_intent) = if scroll_free {
        (None, None)
    } else {
        (ui.scroll_top, ui.scroll_intent)
    };
    assemble(FrameInput {
        transcript: transcript.nodes(),
        feedback: &feedback_lines,
        overlay: &overlay,
        prompt: ui.editor.line(),
        cursor_col: ui.editor.cursor_col(),
        footer,
        view: ui.view,
        streaming: turn.streaming,
        thinking: turn.thinking,
        banner: crate::banner::banner_lines(&ui.cwd, &ui.model),
        hints,
        width,
        height,
        scroll_top,
        scroll_intent,
    })
}

/// 帧重绘：组装 → 行级 diff 渲染（只写变化行，无清屏闪烁）→ 光标定位输入行。
fn redraw(
    backend: &mut CrosstermBackend,
    pump: &EventPump,
    feedback: &FeedbackRing,
    ui: &mut Ui,
    driver: &TurnDriver,
    budget: u64,
) {
    let transcript = pump.transcript();
    // 流式态：轮在飞 → 最后一条非空 assistant 行尾跟块光标 ▌。
    let streaming = driver.started_at().is_some();
    // footer 双态：轮在飞 → running（elapsed + Esc 中断）；否则 idle（占用率 + model）。
    let started_at = driver.started_at();
    let state = match started_at {
        Some(started_at) => Footer::Running { started_at },
        None => Footer::Idle,
    };
    // Thinking 状态行数据：轮开始计时 + 流式字符数（最后 assistant 块）。
    let thinking = started_at.map(|started_at| {
        let chars = transcript
            .nodes()
            .iter()
            .rev()
            .find_map(|node| match node {
                crate::transcript::Node::AssistantBlock { text, .. } => {
                    Some(text.chars().count())
                }
                _ => None,
            })
            .unwrap_or(0);
        (started_at.elapsed().as_secs(), chars)
    });
    let (width, height) = crossterm::terminal::size().unwrap_or((100, 24));
    let turn = TurnView { state, streaming, thinking };
    let built = compose(transcript, feedback, ui, &turn, budget, width, height);
    // 滚动结算值回写：冻结顶对齐本帧窗口（夹取/触底解冻），意图已消费。
    ui.scroll_top = built.scroll_top;
    ui.scroll_intent = None;
    // 光标定位 prompt 行（hints 叠加时 prompt 不再是倒数第二行）。
    let cursor = built
        .cursor_col
        .zip(built.prompt_row)
        .map(|(col, row)| (row, col));
    let _ = backend.render_frame(&built.lines, cursor);
}
