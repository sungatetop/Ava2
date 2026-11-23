# ava2 App install/manage REST wrapper (self-contained, no ava2 repo dependency).
# Usage:
#   install   : .\install-app.ps1 -Action install -Path C:\path\to\my-app [-Port 9800] [-Overwrite]
#   list      : .\install-app.ps1 -Action list [-Port 9800]
#   enable    : .\install-app.ps1 -Action enable -Id my-app [-Port 9800]
#   disable   : .\install-app.ps1 -Action disable -Id my-app [-Port 9800]
#   uninstall : .\install-app.ps1 -Action uninstall -Id my-app [-Port 9800]
# Install body is sent as UTF8 bytes (no BOM) to avoid app_manifest_invalid.
param(
  [ValidateSet("install", "list", "enable", "disable", "uninstall")]
  [string]$Action = "install",
  [string]$Path,
  [string]$Id,
  [int]$Port = 9800,
  [switch]$Overwrite
)

$ErrorActionPreference = "Stop"
$base = "http://127.0.0.1:$Port"

switch ($Action) {
  "install" {
    if (-not $Path) { throw "install requires -Path (app product dir: bare-ESM source dir or built dist/)" }
    $resolved = (Resolve-Path $Path).Path
    $body = @{ path = $resolved; overwrite = [bool]$Overwrite } | ConvertTo-Json -Compress
    Invoke-RestMethod -Method Post -Uri "$base/v1/apps/install" `
      -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body))
  }
  "list" {
    Invoke-RestMethod -Uri "$base/v1/apps"
  }
  "enable" {
    if (-not $Id) { throw "enable requires -Id" }
    $body = '{"enabled":true}'
    Invoke-RestMethod -Method Put -Uri "$base/v1/apps/$Id/enabled" `
      -ContentType 'application/json' -Body ([Text.Encoding]::UTF8.GetBytes($body))
  }
  "disable" {
    if (-not $Id) { throw "disable requires -Id" }
    $body = '{"enabled":false}'
    Invoke-RestMethod -Method Put -Uri "$base/v1/apps/$Id/enabled" `
      -ContentType 'application/json' -Body ([Text.Encoding]::UTF8.GetBytes($body))
  }
  "uninstall" {
    if (-not $Id) { throw "uninstall requires -Id" }
    Invoke-RestMethod -Method Delete -Uri "$base/v1/apps/$Id"
  }
}
