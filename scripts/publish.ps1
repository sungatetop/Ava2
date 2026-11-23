# ava2 release publisher (Windows / PowerShell)
#
# Usage (run under ava2/):
#   .\scripts\publish.ps1                    # version check + build + collect + upload (stable)
#   .\scripts\publish.ps1 -Channel beta      # 发布到 beta 频道
#   .\scripts\publish.ps1 -Bump patch        # 先自增版本（或 minor / major），再构建发布
#   .\scripts\publish.ps1 -Version 0.3.0     # 先同步到指定版本，再构建发布
#   .\scripts\publish.ps1 -SkipBuild         # 复用已有 bundle，仅收集 + 上传
#   .\scripts\publish.ps1 -SkipUpload        # 只本地打包，不传 OSS
#   .\scripts\publish.ps1 -DryRun            # 打印上传命令，不实际执行
#   .\scripts\publish.ps1 -Bundles msi       # 打包目标（默认 nsis,msi；自动更新依赖 nsis）
#
# Local output:  release/v<version>/  (安装包 / .sig / update.json / manifest.json / sha256.txt)
#
# OSS layout (bucket: scalingagentlab, 与 ava 的 releases/ 前缀隔离，避免同版本号互相覆盖):
#   ava2/releases/<channel>/windows-x86_64/<version>/<installer+*.sig / manifest.json / sha256.txt>
#   ava2/channels/<channel>/windows-x86_64/update.json      <- 客户端 updater 检查点
#
# Notes:
#   - 版本源是 package.json；scripts/version.mjs 同步 tauri.conf.json / Cargo.toml / Cargo.lock。
#   - 边车（onion-server + onion-sandbox）必须在 tauri build 前构建：bundle.resources 会打包它们。
#   - 自动更新：Windows 上用 NSIS `*-setup.exe` 作为更新包（Tauri v2 直接给原始产物签名，
#     产出 `.exe.sig` / `.msi.sig`，无 zip 包裹）；msi 仅手动分发。
#   - 签名私钥 scripts/keys/ava2-updater.key（不入库），密码当前阶段硬编码于此。

param(
    [ValidateSet("stable", "beta", "dev")]
    [string]$Channel = "stable",
    [string]$Version = "",
    [string]$Bump = "",
    [string]$Notes = "",
    [switch]$SkipBuild = $false,
    [switch]$SkipUpload = $false,
    [switch]$DryRun = $false,
    [string]$Bundles = "nsis,msi",
    [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"

$Root         = Split-Path $PSScriptRoot -Parent          # ava2/
$TauriDir     = Join-Path $Root "src-tauri"
$TauriRes     = Join-Path $TauriDir "resources"
$BundleDir    = Join-Path $TauriDir "target\release\bundle"
if ([string]::IsNullOrWhiteSpace($OutDir)) { $OutDir = Join-Path $Root "release" }

$Platform        = "windows-x86_64"
$OssBucket       = "scalingagentlab"
$OssEndpoint     = "oss-cn-beijing.aliyuncs.com"
$OssPrefix       = "ava2"
$SigningKey      = Join-Path $Root "scripts\keys\ava2-updater.key"

# 本地凭据（不入库）：scripts/publish.local.ps1 定义 $OssAccessKeyId /
# $OssAccessKeySecret / $SigningPassword；缺失时回退同名环境变量。
$LocalSecrets = Join-Path $PSScriptRoot "publish.local.ps1"
if (Test-Path $LocalSecrets) { . $LocalSecrets }
if (-not $OssAccessKeyId)     { $OssAccessKeyId     = $env:AVA2_OSS_ACCESS_KEY_ID }
if (-not $OssAccessKeySecret) { $OssAccessKeySecret = $env:AVA2_OSS_ACCESS_KEY_SECRET }
if (-not $SigningPassword)    { $SigningPassword    = $env:AVA2_SIGNING_PASSWORD }

function Write-Step([string]$msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Fail([string]$msg) { Write-Host $msg -ForegroundColor Red; exit 1 }
function Write-Utf8File([string]$Path, [string]$Content) {
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $utf8NoBom)
}
function Get-Sha256([string]$Path) { (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLowerInvariant() }

function Find-OssUtil {
    foreach ($candidate in @(
        (Join-Path $Root "ossutil64.exe"),
        (Join-Path $Root "scripts\ossutil.exe"),
        (Join-Path $Root "scripts\ossutil64.exe")
    )) {
        if (Test-Path $candidate) { return $candidate }
    }
    return $null
}

function Invoke-OssUpload([string]$OssUtil, [string]$LocalPath, [string]$RemotePath) {
    if ($DryRun) {
        Write-Host "[DryRun] ossutil cp $LocalPath -> $RemotePath" -ForegroundColor DarkYellow
        return
    }
    & $OssUtil cp $LocalPath $RemotePath --force -i $OssAccessKeyId -k $OssAccessKeySecret -e $OssEndpoint
    if ($LASTEXITCODE -ne 0) { Write-Fail "OSS upload failed: $RemotePath" }
    Write-Host "  uploaded $RemotePath"
}

# 覆盖运行中的 exe 会因文件占用失败；Windows 允许重命名使用中的可执行文件，
# 故先改名腾出目标路径（旧文件留在同级，下次构建前清理）。
function Copy-ExeTolerant([string]$Source, [string]$Destination) {
    if (Test-Path $Destination) {
        $stale = "$Destination.old"
        if (Test-Path $stale) { Remove-Item $stale -Force -ErrorAction SilentlyContinue }
        try { Move-Item $Destination $stale -Force } catch { Remove-Item $Destination -Force -ErrorAction Stop }
    }
    Copy-Item $Source $Destination -Force
}

# ---- 0. Environment checks ----
Write-Step "0/8 environment checks"
foreach ($cmd in @("cargo", "npm", "node")) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) { Write-Fail "$cmd not found in PATH" }
}

# ---- 1. Version management ----
Write-Step "1/8 version"
if ($Bump -ne "") {
    if ($Version -ne "") { Write-Fail "-Version and -Bump are mutually exclusive" }
    & node (Join-Path $Root "scripts\version.mjs") --bump $Bump
    if ($LASTEXITCODE -ne 0) { Write-Fail "version bump failed" }
} elseif ($Version -ne "") {
    & node (Join-Path $Root "scripts\version.mjs") --set $Version
    if ($LASTEXITCODE -ne 0) { Write-Fail "version set failed" }
}
& node (Join-Path $Root "scripts\version.mjs") --check
if ($LASTEXITCODE -ne 0) { Write-Fail "version mismatch, run: npm run version:set -- <x.y.z>" }

$ver = (& node (Join-Path $Root "scripts\version.mjs") --get).Trim()
Write-Host "release version = $ver (channel: $Channel)"
$verDir = Join-Path $OutDir "v$ver"
$notesText = if ([string]::IsNullOrWhiteSpace($Notes)) { "ava2 v$ver" } else { $Notes }

$gitCommit = ""
if (Get-Command git -ErrorAction SilentlyContinue) {
    $gitCommit = (& git -C $Root rev-parse --short HEAD 2>$null)
    if ($LASTEXITCODE -ne 0) { $gitCommit = "" }
}

# ---- 2. Build & stage default apps ----
# 默认安装 chatapp / file-explorer / file-viewer / task-manager / model-manager / skills-manager：
# 各自构建（esbuild 或静态复制），dist/ 复制进 src-tauri/resources/apps/<id>/，
# 随安装包打包，边车首次启动时种子安装进 <ONION_HOME>/apps。
$DefaultApps = @("chatapp", "file-explorer", "file-viewer", "task-manager", "model-manager", "skills-manager")
if (-not $SkipBuild) {
    Write-Step "2/8 build & stage default apps"
    foreach ($app in $DefaultApps) {
        $appDir = Join-Path $Root "apps\$app"
        if (-not (Test-Path (Join-Path $appDir "build.mjs"))) {
            Write-Fail "missing apps\$app\build.mjs (default app not found)"
        }
        Push-Location $appDir
        try {
            node build.mjs
            if ($LASTEXITCODE -ne 0) { Write-Fail "app build failed: $app" }
        } finally { Pop-Location }
    }
} else {
    Write-Step "2/8 skip default app build (-SkipBuild)"
}
$StagedApps = Join-Path $TauriRes "apps"
New-Item -ItemType Directory -Path $StagedApps -Force | Out-Null
foreach ($app in $DefaultApps) {
    $dist = Join-Path $Root "apps\$app\dist"
    if (-not (Test-Path (Join-Path $dist "manifest.json"))) {
        Write-Fail "staged dist missing for $app (run without -SkipBuild)"
    }
    $target = Join-Path $StagedApps $app
    if (Test-Path $target) { Remove-Item $target -Recurse -Force }
    Copy-Item $dist $target -Recurse
    Write-Host "staged apps/$app -> $target"
}

# ---- 3. Build release sidecar ----
if (-not $SkipBuild) {
    Write-Step "3/8 build sidecar (release)"
    Push-Location $Root
    try {
        # onion-sandbox 与 onion-server 一同构建：边车按同目录规则定位沙箱，
        # 缺件会让 workspace-write 降级为逐次审批（fail-closed 但体验崩坏）
        cargo build -p onion-server -p onion-sandbox --release
        if ($LASTEXITCODE -ne 0) { Write-Fail "cargo build -p onion-server -p onion-sandbox --release failed" }
    } finally { Pop-Location }
} else {
    Write-Step "3/8 skip sidecar build (-SkipBuild)"
}

# ---- 4. Copy sidecar into tauri resources ----
Write-Step "4/8 copy sidecar to src-tauri/resources"
if (-not (Test-Path $TauriRes)) { New-Item -ItemType Directory -Path $TauriRes -Force | Out-Null }
foreach ($name in @("onion-server.exe", "onion-sandbox.exe")) {
    $src = Join-Path $Root "target\release\$name"
    if (-not (Test-Path $src)) { Write-Fail "release artifact not found: $src (run without -SkipBuild)" }
    Copy-ExeTolerant -Source $src -Destination (Join-Path $TauriRes $name)
    Write-Host "copied $name -> $TauriRes"
}

# ---- 5. Build app (frontend + tauri bundle, signed for updater) ----
if (-not $SkipBuild) {
    Write-Step "5/8 tauri build (bundles: $Bundles, signed)"
    if (-not (Test-Path $SigningKey)) {
        Write-Fail "updater signing key not found: $SigningKey (tauri signer generate -w scripts/keys/ava2-updater.key)"
    }
    # 签名环境：tauri build 阶段对 NSIS/MSI 产物直接产出 .sig（供 update.json 使用）
    $env:TAURI_SIGNING_PRIVATE_KEY = $SigningKey
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $SigningPassword
    # 构建期数据与生产 ~/.onion 隔离
    $env:ONION_HOME = Join-Path $Root ".onion-test"
    Push-Location $Root
    try {
        npm run tauri build -- --bundles $Bundles
        if ($LASTEXITCODE -ne 0) { Write-Fail "npm run tauri build failed" }
    } finally { Pop-Location }
} else {
    Write-Step "5/8 skip tauri build (-SkipBuild)"
}

# ---- 6. Collect artifacts ----
Write-Step "6/8 collect artifacts -> $verDir"
if (Test-Path $verDir) { Remove-Item $verDir -Recurse -Force }
New-Item -ItemType Directory -Path $verDir -Force | Out-Null

$artifactSpecs = @(
    @{ Kind = "nsis"; Dir = (Join-Path $BundleDir "nsis"); Filter = "*-setup.exe" },
    @{ Kind = "msi";  Dir = (Join-Path $BundleDir "msi");  Filter = "*.msi" }
)

$artifacts = @()
foreach ($spec in $artifactSpecs) {
    if (-not (Test-Path $spec.Dir)) { continue }
    foreach ($file in (Get-ChildItem -Path $spec.Dir -Filter $spec.Filter -File)) {
        # 严格匹配当前版本，避免收集到历史产物
        if ($file.Name -notlike "*$ver*") {
            Write-Host "  skip (version mismatch): $($file.Name)" -ForegroundColor DarkYellow
            continue
        }
        $dest = Join-Path $verDir $file.Name
        Copy-Item $file.FullName $dest -Force
        $sigFile = Get-ChildItem -Path $spec.Dir -Filter "$($file.Name).sig" -File -ErrorAction SilentlyContinue | Select-Object -First 1
        $sigDest = $null
        if ($sigFile) {
            $sigDest = Join-Path $verDir $sigFile.Name
            Copy-Item $sigFile.FullName $sigDest -Force
        }
        $artifacts += [ordered]@{
            kind           = $spec.Kind
            file           = $file.Name
            size           = (Get-Item $dest).Length
            sha256         = Get-Sha256 $dest
            signature_file = if ($sigDest) { $sigFile.Name } else { "" }
        }
        Write-Host ("  {0}  {1}  {2:N1} MB  sig={3}" -f (Get-Sha256 $dest).Substring(0, 12), $file.Name, ((Get-Item $dest).Length / 1MB), $(if ($sigDest) { "yes" } else { "no" }))
    }
}
if ($artifacts.Count -eq 0) {
    Write-Fail "no installer found under $BundleDir (expected nsis/*-setup.exe or msi/*.msi for v$ver)"
}

# ---- 7. Release metadata (update.json / manifest.json / sha256.txt) ----
Write-Step "7/8 write update.json / manifest.json / sha256.txt"
$baseUrl = "https://$OssBucket.$OssEndpoint/$OssPrefix/releases/$Channel/$Platform/$ver"
$pubDate = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

$nsis = $artifacts | Where-Object { $_.kind -eq "nsis" } | Select-Object -First 1
$msi  = $artifacts | Where-Object { $_.kind -eq "msi" }  | Select-Object -First 1
# Tauri v2 的 Windows updater 同时支持 NSIS 与 MSI 两种安装器：
# 优先 NSIS（默认安装模式体验最好），NSIS 缺失时（如 GitHub 下载被网络阻断）回退 MSI。
$updaterArtifact = if ($nsis -and $nsis.signature_file) { $nsis }
                   elseif ($msi -and $msi.signature_file) { $msi }
                   else { $null }
$nsisSigUrl = if ($updaterArtifact) { "$baseUrl/$($updaterArtifact.signature_file)" } else { "" }
$platforms = [ordered]@{}
if ($updaterArtifact) {
    Write-Host "  updater artifact: $($updaterArtifact.file) ($($updaterArtifact.kind))"
    $platforms[$Platform] = [ordered]@{
        signature = (Get-Content (Join-Path $verDir $updaterArtifact.signature_file) -Raw).Trim()
        url       = "$baseUrl/$($updaterArtifact.file)"
    }
} else {
    Write-Host "  WARNING: 无已签名安装器（nsis/msi），update.json 无 platforms 条目，客户端无法自动更新" -ForegroundColor Yellow
}

# Tauri v2 updater 清单格式
$updateJson = [ordered]@{
    version   = $ver
    notes     = $notesText
    pub_date  = $pubDate
    platforms = $platforms
}

$manifestJson = [ordered]@{
    product    = "ava2"
    version    = $ver
    channel    = $Channel
    platform   = $Platform
    identifier = "com.ava.ava2"
    builtAt    = $pubDate
    gitCommit  = $gitCommit
    notes      = $notesText
    updaterSigUrl = $nsisSigUrl
    artifacts  = $artifacts
}

$updatePath   = Join-Path $verDir "update.json"
$manifestPath = Join-Path $verDir "manifest.json"
$shaPath      = Join-Path $verDir "sha256.txt"
Write-Utf8File -Path $updatePath -Content ($updateJson | ConvertTo-Json -Depth 6)
Write-Utf8File -Path $manifestPath -Content ($manifestJson | ConvertTo-Json -Depth 6)
$shaLines = @()
foreach ($a in $artifacts) {
    $shaLines += "$($a.sha256)  $($a.file)"
    if ($a.signature_file) { $shaLines += "$(Get-Sha256 (Join-Path $verDir $a.signature_file))  $($a.signature_file)" }
}
Write-Utf8File -Path $shaPath -Content (($shaLines -join "`n") + "`n")

# ---- 8. Upload to OSS ----
if ($SkipUpload) {
    Write-Step "8/8 skip OSS upload (-SkipUpload)"
} else {
    Write-Step "8/8 upload to OSS (oss://$OssBucket/$OssPrefix)"
    $ossUtil = Find-OssUtil
    if (-not $ossUtil) { Write-Fail "ossutil.exe not found (expected ava2/scripts/ossutil.exe)" }

    $versionPath = "oss://$OssBucket/$OssPrefix/releases/$Channel/$Platform/$ver"
    foreach ($file in (Get-ChildItem -Path $verDir -File | Where-Object { $_.Name -ne "update.json" })) {
        Invoke-OssUpload -OssUtil $ossUtil -LocalPath $file.FullName -RemotePath "$versionPath/$($file.Name)"
    }
    Invoke-OssUpload -OssUtil $ossUtil -LocalPath $updatePath -RemotePath "oss://$OssBucket/$OssPrefix/channels/$Channel/$Platform/update.json"
}

Write-Host "`n==> done: ava2 v$ver ($Channel)" -ForegroundColor Green
Write-Host "artifacts : $verDir"
Write-Host "manifest  : $manifestPath"
Write-Host "checksums : $shaPath"
Write-Host "update    : https://$OssBucket.$OssEndpoint/$OssPrefix/channels/$Channel/$Platform/update.json"
if ($nsisSigUrl) { Write-Host "nsis sig  : $nsisSigUrl" }
