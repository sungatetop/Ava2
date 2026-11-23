# ava2 one-shot dev launcher (Windows / PowerShell)
#
# Usage (run under ava2/):
#   .\dev.ps1                  # build sidecar + set ONION_HOME + tauri dev
#   .\dev.ps1 -SkipBuild       # skip sidecar build (reuse existing artifacts)
#   .\dev.ps1 -Port 9800       # sidecar port (default 9800)
#
# Notes:
#   - Data root is pinned to <repo>\.onion-test, isolated from production ~/.onion.
#   - Builds onion-server sidecar and copies it into src-tauri/resources so tauri dev can spawn it.
#   - Launches ava2 via npm run tauri dev; sidecar is spawned by the Tauri shell (src-tauri/src/sidecar.rs).

param(
    [switch]$SkipBuild = $false,
    [int]$Port = 9800
)

$ErrorActionPreference = "Stop"

# Repo root: this script lives at <repo>/ava2/dev.ps1
$Root    = $PSScriptRoot                            # ava2/
$WsRoot  = $Root
$TauriRes = Join-Path $Root "src-tauri\resources"

# Dev data root（仓库内 .onion-test，已 gitignore；与生产 ~/.onion 隔离）
$DataHome = Join-Path $Root ".onion-test"

function Write-Step([string]$msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }

# ---- 0. Environment checks ----
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "cargo not found" }
if (-not (Get-Command npm -ErrorAction SilentlyContinue))   { throw "npm not found" }

# ---- 1. Build sidecar + OS sandbox (skip with -SkipBuild; copy always runs) ----
if (-not $SkipBuild) {
    Write-Step "1/3 build sidecar + sandbox (debug)"
    Push-Location $WsRoot
    try {
        # onion-sandbox 与 onion-server 必须同目录：onion 侧 `auto_sandbox_exe` 只认
        # current_exe() 同目录的 onion-sandbox.exe。二者一起构建一起拷贝。
        cargo build -p onion-server -p onion-sandbox
        if ($LASTEXITCODE -ne 0) { throw "cargo build onion-server/onion-sandbox failed" }
    } finally {
        Pop-Location
    }
} else {
    Write-Step "1/3 skip build (-SkipBuild)"
}

Write-Step "copy sidecar + sandbox to src-tauri/resources"
if (-not (Test-Path $TauriRes)) { New-Item -ItemType Directory -Path $TauriRes -Force | Out-Null }
foreach ($exe in @("onion-server.exe", "onion-sandbox.exe")) {
    $src = Join-Path $WsRoot "target\debug\$exe"
    if (-not (Test-Path $src)) { throw "artifact not found: $src" }
    Copy-Item $src (Join-Path $TauriRes $exe) -Force
    Write-Host "copied $exe -> $TauriRes"
}

# ---- 2. Dev environment variables ----
Write-Step "2/3 dev env vars"
$env:ONION_HOME = $DataHome
$env:ONION_SERVER_PORT = "$Port"
# 裸边车（不经 Tauri 壳）也必须放行前端来源，否则浏览器跨源访问被拦：
# 与 src-tauri/src/sidecar.rs 的 CORS_ORIGINS 保持一致。
$env:ONION_SERVER_CORS_ORIGINS = "http://localhost:1420,http://127.0.0.1:1420,http://tauri.localhost,tauri://localhost"
Write-Host "ONION_HOME        = $env:ONION_HOME"
Write-Host "ONION_SERVER_PORT = $env:ONION_SERVER_PORT"

# ---- 3. Launch tauri dev ----
Write-Step "3/3 start ava2 (npm run tauri dev)"
Push-Location $Root
try {
    npm run tauri dev
} finally {
    Pop-Location
}
