//! `shell` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::approval::ToolRisk;
use super::{classify_shell_command, classify_shell_risk, confine_to_sandbox};

#[test]
fn read_commands_are_readonly() {
    for cmd in [
        "ls",
        "ls -la",
        "dir",
        "pwd",
        "Get-ChildItem -Recurse",
        "Get-Content file.txt",
        "cat notes.md",
        "grep foo main.rs",
        "git status",
        "git diff",
        "git log --oneline -5",
        "whoami",
        "date",
    ] {
        assert_eq!(classify_shell_command(cmd), ToolRisk::ReadOnly, "cmd: {cmd}");
    }
}

#[test]
fn readonly_pipelines_and_chains_are_readonly() {
    // 免打扰体验：只读命令组成的管道 / 链整体放行（此前管道一律升级）。
    for cmd in [
        "ls -la 2>&1 | head -60",
        "ls | Select-String foo",
        "cat x | grep foo | head -5",
        "get-process | sort-object cpu | select-object -first 3",
        "echo hi | Out-Null",
        "ls > /dev/null",
        "ls 2>/dev/null",
        "ls >> /dev/null",
        "git log --oneline 2>&1 | head -20",
        "cat a && cat b",
        "ls; pwd; whoami",
        "pwd || ls",
        "grep \"a|b\" file.rs",
        "echo \"x > y\"",
        "find . -name \"*.rs\"",
    ] {
        assert_eq!(classify_shell_command(cmd), ToolRisk::ReadOnly, "cmd: {cmd}");
    }
}

#[test]
fn workspace_writes_are_workspace_write() {
    // 工作区内写（创建 / 复制 / 构建 / 测试）：沙箱兜底遏制越界写，故归 WorkspaceWrite。
    for cmd in [
        "mkdir build",
        "mkdir -p a/b && touch a/b/c.txt",
        "touch new.txt",
        "cp a b",
        "git add .",
        "git commit -m x",
        "cargo build",
        "cargo build --release && cargo test",
        "npm run build",
        "npx tsc --noEmit",
        "New-Item -ItemType Directory smoke",
    ] {
        assert_eq!(classify_shell_command(cmd), ToolRisk::WorkspaceWrite, "cmd: {cmd}");
    }
}

#[test]
fn readonly_mixed_with_workspace_write_is_workspace_write() {
    // 只读与工作区写混排：整体按较高档（仍有写操作，只是其中的读不再是升级理由）。
    for cmd in ["cat a && mkdir b", "ls -la | head -5 && cargo test", "test -f x || touch x"] {
        assert_eq!(classify_shell_command(cmd), ToolRisk::WorkspaceWrite, "cmd: {cmd}");
    }
}

#[test]
fn destructive_or_network_commands_escalate() {
    for cmd in [
        "rm file.txt",
        "rm -rf dist",
        "mv a b",
        "chmod +x run.sh",
        "echo x > out.txt",
        "echo x >> append.log",
        "ls 2>err.log",
        "cat > file.txt",
        "sudo apt update",
        "curl example.com",
        "git push",
        "git push origin main",
        "npm install",
        "npm ci",
        "pip install requests",
        "cargo install cargo-shear",
        "git clone https://example.com/x.git",
    ] {
        assert_eq!(classify_shell_command(cmd), ToolRisk::Escalating, "cmd: {cmd}");
    }
}

#[test]
fn unsandboxed_shell_escalates_fail_closed() {
    // 无 OS 沙箱（或后台执行）时只读与工作区写一并退回显式审批兜底（fail-closed，
    // confinement 缺失拒绝执行）；沙箱就位时各档保持原判定。
    assert_eq!(confine_to_sandbox(ToolRisk::ReadOnly, false), ToolRisk::Escalating);
    assert_eq!(confine_to_sandbox(ToolRisk::WorkspaceWrite, false), ToolRisk::Escalating);
    assert_eq!(confine_to_sandbox(ToolRisk::Escalating, false), ToolRisk::Escalating);
    assert_eq!(confine_to_sandbox(ToolRisk::ReadOnly, true), ToolRisk::ReadOnly);
    assert_eq!(confine_to_sandbox(ToolRisk::WorkspaceWrite, true), ToolRisk::WorkspaceWrite);
    assert_eq!(confine_to_sandbox(ToolRisk::Escalating, true), ToolRisk::Escalating);
}

#[test]
fn sandboxed_unknown_commands_run_in_sandbox() {
    // dsh web ui 对齐：沙箱就位时命令不做字符串级前置审批——静态未识别 / 不可
    // 判定档（未知命令、脚本块、文件重定向、危险参数）交 OS 沙箱运行时遏制，
    // 越界写在运行时被拒后走 denial → 升级流程。
    for cmd in [
        "node build.js",
        "python script.py",
        "git checkout -b feature",
        "go test ./...",
        "echo hi > out.txt",
        "foreach-object { remove-item $_ }",
        "ls $(rm -rf /)",
        "find . -delete",
    ] {
        assert_eq!(classify_shell_risk(cmd, true), ToolRisk::WorkspaceWrite, "cmd: {cmd}");
    }
    // 白名单识别的档位保持不变
    assert_eq!(classify_shell_risk("git status", true), ToolRisk::ReadOnly);
    assert_eq!(classify_shell_risk("cargo test", true), ToolRisk::WorkspaceWrite);
    // 网络命令是沙箱盲区（不做网络遏制），唯一保留的前置升级
    for cmd in [
        "npm install react",
        "curl https://example.com",
        "git push origin main",
        "echo ok && npm i lodash",
    ] {
        assert_eq!(classify_shell_risk(cmd, true), ToolRisk::Escalating, "cmd: {cmd}");
    }
    // 沙箱未就位：维持 fail-closed，全部升级
    for cmd in ["node build.js", "git status", "npm install react", "echo hi > out.txt"] {
        assert_eq!(classify_shell_risk(cmd, false), ToolRisk::Escalating, "cmd: {cmd}");
    }
}

#[test]
fn dynamic_content_escalates_fail_closed() {
    // 命令替换 / 反引号 / 脚本块 / 危险参数：内容不可静态判定，一律升级。
    for cmd in [
        "ls $(rm -rf /)",
        "echo `whoami`",
        "foreach-object { remove-item $_ }",
        "ls | select-object { $_.name }",
        "find . -delete",
        "find . -name x -exec rm {} \\;",
        "ls | xargs rm",
    ] {
        assert_eq!(classify_shell_command(cmd), ToolRisk::Escalating, "cmd: {cmd}");
    }
}

#[test]
fn side_effect_free_commands_are_readonly() {
    // 白名单扩容：等待 / 纯计算 / 路径解析类命令不再逐次审批。
    for cmd in [
        "sleep 0.5",
        "true",
        "false",
        "test -f Cargo.toml && echo ok",
        "printf '%s\\n' hello",
        "basename /tmp/a.txt",
        "dirname /tmp/a.txt",
        "realpath .",
        "seq 1 5",
        "nproc",
        "sleep 0.5 && sleep 0.5",
        "sleep 1 | cat",
    ] {
        assert_eq!(classify_shell_command(cmd), ToolRisk::ReadOnly, "cmd: {cmd}");
    }
}

#[test]
fn empty_command_escalates() {
    assert_eq!(classify_shell_command(""), ToolRisk::Escalating);
}

#[test]
fn benign_pwsh_cmdlets_are_readonly() {
    // pwsh 无害 cmdlet（等待 / 纯输出）与 bash 同类命令同档：`Start-Sleep` 此前不在
    // 白名单，workspace-write 下被逐次人工审批（实测冒烟中挂起等待复决的真实案例）。
    for cmd in [
        "Start-Sleep -Milliseconds 500",
        "Start-Sleep -Seconds 2 && Start-Sleep -Seconds 1",
        "Start-Sleep 1 | Out-Null",
        "Write-Output hello",
        "Write-Host done",
        "Write-Output 'a|b' | Out-Null",
    ] {
        assert_eq!(classify_shell_command(cmd), ToolRisk::ReadOnly, "cmd: {cmd}");
    }
}

#[test]
fn non_whitelisted_pwsh_verbs_still_escalate() {
    // 白名单外的 pwsh 动词仍 fail-closed（Start-Process 可执行任意程序）。
    for cmd in [
        "Start-Process notepad",
        "Start-Job { Get-Date }",
        "Invoke-Expression 'Get-Date'",
    ] {
        assert_eq!(classify_shell_command(cmd), ToolRisk::Escalating, "cmd: {cmd}");
    }
}

#[test]
fn unknown_command_escalates_fail_closed() {
    assert_eq!(classify_shell_command("some_unknown_binary --flag"), ToolRisk::Escalating);
}
