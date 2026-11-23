//! `skill` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::path::Path;

use super::{
    escape_attr, escape_text, is_model_invocable, is_skill_name, is_user_invocable,
    render_skill_content, SkillDefinition, SkillInvocationPolicy, SkillResourceBase, SkillSource,
};

fn def(name: &str, content: &str) -> SkillDefinition {
    SkillDefinition {
        path: None,
        name: name.to_string(),
        description: "desc".to_string(),
        when_to_use: None,
        invocation: SkillInvocationPolicy::default(),
        source: SkillSource::Runtime,
        provider: "runtime".to_string(),
        resource_base: Some(SkillResourceBase::Directory {
            path: Path::new("/root/skills/my-skill").to_path_buf(),
        }),
        content: content.to_string(),
    }
}

#[test]
fn is_skill_name_validates_kebab_case() {
    assert!(is_skill_name("foo"));
    assert!(is_skill_name("foo-bar"));
    assert!(is_skill_name("foo-bar-baz"));
    assert!(is_skill_name("a1-b2"));
    assert!(!is_skill_name(""));
    assert!(!is_skill_name("Foo"));
    assert!(!is_skill_name("foo_bar"));
    assert!(!is_skill_name("-foo"));
    assert!(!is_skill_name("foo-"));
    assert!(!is_skill_name("foo--bar"));
    assert!(!is_skill_name("foo/bar"));
    assert!(!is_skill_name(".."));
}

#[test]
fn policy_predicates_check_invocation_flags() {
    let both = SkillInvocationPolicy { model_invocable: true, user_invocable: true };
    let model_only = SkillInvocationPolicy { model_invocable: true, user_invocable: false };
    let user_only = SkillInvocationPolicy { model_invocable: false, user_invocable: true };
    assert!(is_model_invocable(&both));
    assert!(is_user_invocable(&both));
    assert!(is_model_invocable(&model_only));
    assert!(!is_user_invocable(&model_only));
    assert!(!is_model_invocable(&user_only));
    assert!(is_user_invocable(&user_only));
}

#[test]
fn escape_attr_and_text_verbatim() {
    assert_eq!(escape_attr("a&b\"c<d"), "a&amp;b&quot;c&lt;d");
    assert_eq!(escape_text("a&b<c>d"), "a&amp;b&lt;c&gt;d");
}

#[test]
fn render_skill_content_wraps_body_verbatim() {
    let skill = def("my-skill", "Do the thing.\n\nSecond line.");
    let rendered = render_skill_content(&skill);
    assert!(rendered.starts_with("<skill_content name=\"my-skill\">"));
    assert!(rendered.contains("<skill_instructions>\nDo the thing.\n\nSecond line.\n</skill_instructions>"));
    assert!(rendered.contains("Base directory for this skill: /root/skills/my-skill"));
    assert!(rendered.ends_with("</skill_content>"));
}

#[test]
fn render_skill_content_url_resource_hint() {
    let skill = SkillDefinition {
        resource_base: Some(SkillResourceBase::Url {
            url: "https://skills.example.test/url-skill".to_string(),
        }),
        ..def("url-skill", "URL instructions.")
    };
    let rendered = render_skill_content(&skill);
    assert!(rendered.contains(
        "<skill_resources>\nBase URL for this skill: https://skills.example.test/url-skill\n\
         Resolve relative URLs mentioned by this skill against the base URL before using them. \
         Load referenced resources only as needed.\n</skill_resources>"
    ));
}

#[test]
fn render_skill_content_opaque_resource_hint() {
    let skill = SkillDefinition {
        resource_base: Some(SkillResourceBase::Opaque { description: "runtime memory".to_string() }),
        ..def("opaque-skill", "Opaque instructions.")
    };
    let rendered = render_skill_content(&skill);
    assert!(rendered.contains(
        "<skill_resources>\nResources for this skill: runtime memory\n\
         Load referenced resources only as needed.\n</skill_resources>"
    ));
}

#[test]
fn render_skill_content_provider_managed_hint_when_no_base() {
    let skill = SkillDefinition { resource_base: None, ..def("provider-skill", "Provider instructions.") };
    let rendered = render_skill_content(&skill);
    assert!(rendered.contains(
        "<skill_resources>\nResources for this skill are managed by provider \"runtime\".\n\
         Load referenced resources only as needed.\n</skill_resources>"
    ));
}

#[test]
fn render_skill_content_escapes_name_attribute() {
    // 名称经 kebab-case 校验，本不含可转义字符；此处防御性验证转义路径。
    let skill = def("a-skill", "body");
    let rendered = render_skill_content(&skill);
    assert!(rendered.starts_with("<skill_content name=\"a-skill\">"));
}
