# 贡献指南

感谢参与 ava2。本文说明本地开发、提交与评审的基本约定。

## 开发环境

- Node.js 20+ / npm
- Rust stable（`cargo`、`rustfmt`、`clippy`）
- Windows 需 WebView2

```powershell
npm install
.\dev.ps1
```

## 代码规范

完整规范见 [.trae/rules/project_rules.md](.trae/rules/project_rules.md)，要点：

- **Rust**：遵循 `Cargo.toml` 中 `[workspace.lints.clippy]` 的 deny 规则集；模块目标 <500 LoC；
  单元测试使用独立 `*_tests.rs` 兄弟文件（`#[path]` 挂载），不写内联 `#[cfg(test)] mod tests`。
- **前端**：React 19 函数组件 + Hooks；类型放 `src/types/`，请求放 `src/api/`；样式使用
  `--ava-*` 设计令牌，禁止硬编码色值字号。
- **App**：遵循 [App 开发规范](docs/references/app-dev-standards.md)，`setup(ctx)` 只返回组件工厂。

## 提交前自检

```powershell
cargo fmt --all -- --check
cargo clippy -p onion -p onion-server --all-targets
cargo test -p onion
cargo test -p onion-server
npm run check:contracts
```

## 提交信息

采用 Conventional Commits 风格，中文描述可接受：

```
feat(chatapp): 新增图片附件粘贴上传
fix(onion-server): 修正会话写租约释放时机
docs: 补充 App 设置页契约说明
```

## 分支与 PR

1. 从 `master` 切出特性分支：`feat/<topic>` / `fix/<topic>`。
2. 保持提交粒度可评审；大改动拆分为多个子提交。
3. PR 描述包含：改动动机、影响面、验证方式（含冒烟截图）。
4. 单测通过后再提 PR 进入 review。

## 安全

- 严禁提交任何密钥、令牌、内网地址或私有二进制。
- 本地凭据放在 `scripts/publish.local.ps1`、`<ONION_HOME>/local-defaults.json` 等已 gitignore 的文件中。
- 发现安全问题请私下联系维护者，不要公开提交 issue。
