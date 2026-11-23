//! web 工具族：`web_search` / `web_fetch`。
//!
//! - `fetch.rs`：`web_fetch`——抓取特定 HTTP(S) URL 并解码为文本（reqwest 直连，
//!   无需密钥）；content-type 分类 html/text、HTML→文本转换、大小上限、外部
//!   内容提示、截断 footer。
//! - `search.rs`：`web_search`——向 Anthropic-compatible Messages 端点发起原生
//!   `web_search` 服务器工具调用，规范化 results；端点未配置时工具仍注册但执行期
//!   结构化失败（工具可见但 provider 不可用）。
//!
//! 两者均 `ReadOnly`、非 deferred；结果标注「外部内容不可信」，
//! 提示模型以 markdown 链接引用来源。

pub mod fetch;
pub mod search;

pub use fetch::WebFetchTool;
pub use search::WebSearchTool;
