//! skills-manager：工作区技能禁用清单（`<projectRoot>/.onion/skills-manager.json`）
//! 读写面。
//!
//! - `GET` 返回当前生效的禁用名单（fail-open）与落盘文件绝对路径；
//! - `PUT` 全量替换（与文件语义一致，无并发合并）：非法名 400、去重排序、
//!   tmp + rename 原子写、`.onion` 目录缺省创建；
//! - 禁用语义（注入面收敛）由内核 [`onion::skill::DisablePolicy`] 承担，本模块
//!   只做清单文件的读写契约。

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::rest::capabilities::session_param;
use crate::rest::error::ApiError;
use crate::router::RouterState;

use onion::skill::disable::{read_disabled_names, write_disabled_names};

/// `GET/PUT /v1/skills-manager` 出参：禁用名单（排序稳定输出）+ 落盘文件绝对路径。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillManagerInfo {
    pub disabled: Vec<String>,
    pub file: String,
}

/// `PUT /v1/skills-manager` 入参（全量替换）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetSkillManagerRequest {
    pub disabled: Vec<String>,
}

/// `GET /v1/skills-manager?session=<id>`：当前禁用清单（缺文件 / 非法 JSON
/// fail-open 为空集）。
pub(crate) async fn get_skill_manager(
    State(state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<SkillManagerInfo>, ApiError> {
    let session = session_param(&state, &query).await?;
    let working_dir = session.handle.session_cwd.current().as_ref().clone();
    Ok(Json(skill_manager_info(&working_dir)))
}

/// `PUT /v1/skills-manager?session=<id>`：校验（非法名 400）→ 去重排序 →
/// 原子写落盘；回读返回写入后的稳定名单。
pub(crate) async fn put_skill_manager(
    State(state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
    Json(request): Json<SetSkillManagerRequest>,
) -> Result<Json<SkillManagerInfo>, ApiError> {
    let session = session_param(&state, &query).await?;
    if let Some(name) = request.disabled.iter().find(|name| !onion::skill::is_skill_name(name)) {
        return Err(ApiError::bad_request(
            "invalid_skill_name",
            format!("\"{name}\" is not a valid kebab-case skill name"),
        ));
    }
    let working_dir = session.handle.session_cwd.current().as_ref().clone();
    let path = write_disabled_names(&working_dir, &request.disabled)
        .map_err(|error| ApiError::internal(format!("write skills-manager.json failed: {error}")))?;
    let mut disabled: Vec<String> = read_disabled_names(&path).into_iter().collect();
    disabled.sort();
    Ok(Json(SkillManagerInfo { disabled, file: path.display().to_string() }))
}

/// 按 cwd 组装 GET 出参（名单排序稳定输出）。
fn skill_manager_info(working_dir: &std::path::Path) -> SkillManagerInfo {
    let mut disabled = crate::rest::capabilities::disabled_names_for(working_dir)
        .into_iter()
        .collect::<Vec<String>>();
    disabled.sort();
    let file = onion::skill::disable::manager_file_path(Some(working_dir))
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    SkillManagerInfo { disabled, file }
}
