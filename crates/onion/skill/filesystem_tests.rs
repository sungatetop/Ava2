//! `skill::filesystem` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::fs;

use super::{
    discover_root, find_project_root, parse_frontmatter, parse_invocation_policy,
    parse_skill_file, FileSystemSkillProvider,
};
use crate::skill::registry::SkillProvider;
use crate::skill::{SkillInvocationPolicy, SkillResourceBase, SkillSource};

fn temp_root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("onion_skill_fs_{tag}_{}", std::process::id()))
}

#[test]
fn parse_frontmatter_extracts_data_and_body() {
    let raw = "---\nname: foo\ndescription: A skill\n---\n\nDo the thing.\n";
    let (data, body) = parse_frontmatter(raw).expect("parses");
    assert_eq!(string(&data, "name"), "foo");
    assert_eq!(string(&data, "description"), "A skill");
    assert_eq!(body.trim(), "Do the thing.");
}

#[test]
fn parse_frontmatter_rejects_missing_or_malformed() {
    assert!(parse_frontmatter("no frontmatter").is_none());
    assert!(parse_frontmatter("---\n").is_none()); // 无闭合
    assert!(parse_frontmatter("---\n- a\n- b\n---\n").is_none()); // 非映射
}

#[test]
fn parse_invocation_policy_defaults() {
    let (data, _) = parse_frontmatter("---\nname: x\ndescription: d\n---\n\n").expect("parses");
    let policy = parse_invocation_policy(&data);
    assert_eq!(policy, SkillInvocationPolicy::default());
}

#[test]
fn parse_invocation_policy_disable_model() {
    let raw = "---\nname: x\ndescription: d\ndisable-model-invocation: true\n---\n\n";
    let (data, _) = parse_frontmatter(raw).expect("parses");
    let policy = parse_invocation_policy(&data);
    assert!(!policy.model_invocable);
    assert!(policy.user_invocable);
}

#[test]
fn parse_invocation_policy_user_invocable_false() {
    let raw = "---\nname: x\ndescription: d\nuser-invocable: false\n---\n\n";
    let (data, _) = parse_frontmatter(raw).expect("parses");
    let policy = parse_invocation_policy(&data);
    assert!(policy.model_invocable);
    assert!(!policy.user_invocable);
}

#[test]
fn parse_skill_file_requires_name_and_description() {
    let root = temp_root("required");
    let dir = root.join("s");
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("SKILL.md");
    fs::write(&file, "---\nname: foo\n---\n\nbody\n").unwrap();
    assert!(parse_skill_file(&file).is_none()); // 缺 description

    fs::write(&file, "---\ndescription: d\n---\n\nbody\n").unwrap();
    assert!(parse_skill_file(&file).is_none()); // 缺 name

    fs::write(&file, "---\nname: Bad_Name\ndescription: d\n---\n\nbody\n").unwrap();
    assert!(parse_skill_file(&file).is_none()); // 非法名

    let _ = fs::remove_dir_all(root);
}

#[test]
fn discover_root_finds_directory_and_flat_skills() {
    let root = temp_root("discover");
    fs::create_dir_all(root.join("dir-skill")).unwrap();
    fs::write(
        root.join("dir-skill").join("SKILL.md"),
        "---\nname: dir-skill\ndescription: Dir skill\n---\n\nDir body.\n",
    )
    .unwrap();
    fs::write(
        root.join("flat-skill.md"),
        "---\nname: flat-skill\ndescription: Flat skill\n---\n\nFlat body.\n",
    )
    .unwrap();
    fs::write(root.join("notes.txt"), "not a skill").unwrap();

    let candidates = discover_root(&root, &SkillSource::UserOnion, "filesystem", 400);
    let names: Vec<&str> = candidates.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["dir-skill", "flat-skill"]);

    let dir_skill = candidates.iter().find(|c| c.name == "dir-skill").unwrap();
    assert!(matches!(
        dir_skill.resource_base,
        Some(SkillResourceBase::Directory { .. })
    ));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn provider_get_loads_directory_body_and_resource_base() {
    let root = temp_root("provider");
    fs::create_dir_all(root.join("my-skill")).unwrap();
    fs::write(
        root.join("my-skill").join("SKILL.md"),
        "---\nname: my-skill\ndescription: My skill\n---\n\nMy body.\n",
    )
    .unwrap();

    let provider = FileSystemSkillProvider::new("filesystem", root.clone(), SkillSource::UserOnion);
    let candidates = provider.list(None);
    assert_eq!(candidates.len(), 1);
    let candidate = &candidates[0];
    let definition = provider.get(candidate).expect("loads");
    assert_eq!(definition.name, "my-skill");
    assert_eq!(definition.content, "My body.");
    assert_eq!(definition.provider, "filesystem");
    let _ = fs::remove_dir_all(root);
}

fn string(map: &serde_yaml::Mapping, key: &str) -> String {
    match map.get(serde_yaml::Value::String(key.to_string())) {
        Some(serde_yaml::Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

fn write_skill(root: &std::path::Path, name: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join(format!("{name}.md")),
        format!("---\nname: {name}\ndescription: {name} skill\n---\n\nbody\n"),
    )
    .unwrap();
}

#[test]
fn find_project_root_walks_up_to_git() {
    let base = temp_root("projroot");
    let project = base.join("repo");
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::create_dir_all(project.join("a").join("b")).unwrap();
    assert_eq!(find_project_root(&project.join("a").join("b")), project);
    // project 本身含 .git → 自身即 project root。
    assert_eq!(find_project_root(&project), project);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn find_project_root_falls_back_to_cwd() {
    let base = temp_root("projroot_fallback");
    let plain = base.join("plain").join("deep");
    fs::create_dir_all(&plain).unwrap();
    assert_eq!(find_project_root(&plain), plain);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn provider_discovers_project_roots_from_cwd() {
    let base = temp_root("projskills");
    let project = base.join("repo");
    fs::create_dir_all(project.join(".git")).unwrap();
    write_skill(&project.join(".onion").join("skills"), "harness-skill");
    write_skill(&project.join(".agents").join("skills"), "agents-skill");

    let user_root = base.join("user-skills");
    write_skill(&user_root, "user-skill");
    let provider = FileSystemSkillProvider::with_project_roots(
        "filesystem",
        user_root,
        SkillSource::UserOnion,
    );

    let cwd = project.join("nested");
    fs::create_dir_all(&cwd).unwrap();
    let mut names: Vec<String> = provider
        .list(Some(&cwd))
        .into_iter()
        .map(|c| c.name)
        .collect();
    names.sort();
    assert_eq!(names, vec!["agents-skill", "harness-skill", "user-skill"]);

    // 来源桶与 rank：project-onion 100 / project-agents 200 / user-onion 400。
    let candidates = provider.list(Some(&cwd));
    let harness = candidates.iter().find(|c| c.name == "harness-skill").unwrap();
    assert_eq!(harness.source, SkillSource::ProjectOnion);
    assert_eq!(harness.rank, 100);
    let agents = candidates.iter().find(|c| c.name == "agents-skill").unwrap();
    assert_eq!(agents.source, SkillSource::ProjectAgents);
    assert_eq!(agents.rank, 200);
    let user = candidates.iter().find(|c| c.name == "user-skill").unwrap();
    assert_eq!(user.source, SkillSource::UserOnion);
    assert_eq!(user.rank, 400);

    // cwd 为 None 时退化为纯 user root。
    let names: Vec<String> =
        provider.list(None).into_iter().map(|c| c.name).collect();
    assert_eq!(names, vec!["user-skill"]);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn project_root_rank_beats_user_root_on_name_conflict() {
    let base = temp_root("projconflict");
    let project = base.join("repo");
    fs::create_dir_all(project.join(".git")).unwrap();
    write_skill(&project.join(".onion").join("skills"), "alpha");

    let user_root = base.join("user-skills");
    write_skill(&user_root, "alpha");

    let provider = FileSystemSkillProvider::with_project_roots(
        "filesystem",
        user_root.clone(),
        SkillSource::UserOnion,
    );
    let cwd = project;
    let mut registry = crate::skill::registry::SkillRegistry::new();
    registry
        .register_provider(std::sync::Arc::new(provider))
        .unwrap();
    let merged = registry.list(Some(&cwd));
    assert_eq!(merged.len(), 1, "同名冲突合并为一条");
    assert_eq!(merged[0].source, SkillSource::ProjectOnion, "project 级（rank 100）决胜");
    let _ = fs::remove_dir_all(base);
    let _ = fs::remove_dir_all(user_root);
}

#[test]
fn provider_discovers_claude_dsh_agents_project_roots() {
    let base = temp_root("compskills");
    let project = base.join("repo");
    fs::create_dir_all(project.join(".git")).unwrap();
    write_skill(&project.join(".claude").join("skills"), "claude-skill");
    write_skill(&project.join(".dsh").join("skills"), "dsh-skill");
    write_skill(&project.join(".agents").join("skills"), "agents-skill");
    // onion 自有目录与 claude 同名 → rank 100 压过 110。
    write_skill(&project.join(".onion").join("skills"), "claude-skill");

    let provider = FileSystemSkillProvider::with_project_roots(
        "filesystem",
        base.join("user-skills"),
        SkillSource::UserOnion,
    );
    let cwd = project.join("nested");
    fs::create_dir_all(&cwd).unwrap();
    let candidates = provider.list(Some(&cwd));

    let claude = candidates.iter().find(|c| c.name == "claude-skill").unwrap();
    assert_eq!(claude.source, SkillSource::ProjectOnion, "onion root rank 100 压过 claude 110");
    let dsh = candidates.iter().find(|c| c.name == "dsh-skill").unwrap();
    assert_eq!(dsh.source, SkillSource::ProjectDsh);
    assert_eq!(dsh.rank, 130);
    let agents = candidates.iter().find(|c| c.name == "agents-skill").unwrap();
    assert_eq!(agents.source, SkillSource::ProjectAgents);
    assert_eq!(agents.rank, 200);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn provider_merges_user_compat_roots_from_home() {
    let base = temp_root("usercompat");
    let home = base.join("home");
    fs::create_dir_all(&home).unwrap();
    write_skill(&home.join(".claude").join("skills"), "claude-user");
    write_skill(&home.join(".dsh").join("skills"), "dsh-user");
    write_skill(&home.join(".agents").join("skills"), "agents-user");

    let user_root = base.join("user-onion-skills");
    write_skill(&user_root, "onion-user");
    let provider = FileSystemSkillProvider::with_fixed_roots(
        "filesystem",
        vec![
            (user_root.clone(), SkillSource::UserOnion),
            (home.join(".claude").join("skills"), SkillSource::UserClaude),
            (home.join(".dsh").join("skills"), SkillSource::UserDsh),
            (home.join(".agents").join("skills"), SkillSource::UserAgents),
        ],
    );
    let mut names: Vec<String> = provider.list(None).into_iter().map(|c| c.name).collect();
    names.sort();
    assert_eq!(names, vec!["agents-user", "claude-user", "dsh-user", "onion-user"]);

    let candidates = provider.list(None);
    let claude = candidates.iter().find(|c| c.name == "claude-user").unwrap();
    assert_eq!(claude.source, SkillSource::UserClaude);
    assert_eq!(claude.rank, 410);
    let dsh = candidates.iter().find(|c| c.name == "dsh-user").unwrap();
    assert_eq!(dsh.source, SkillSource::UserDsh);
    assert_eq!(dsh.rank, 430);
    let agents = candidates.iter().find(|c| c.name == "agents-user").unwrap();
    assert_eq!(agents.source, SkillSource::UserAgents);
    assert_eq!(agents.rank, 500);
    let _ = fs::remove_dir_all(base);
    let _ = fs::remove_dir_all(user_root);
}
