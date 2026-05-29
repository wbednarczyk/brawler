[CmdletBinding()]
param(
    [string] $WindowsRepo = $env:BRAWLER_WINDOWS_REPO,
    [string] $OutputDir = $env:BRAWLER_WINDOWS_OUT,
    [switch] $SkipInstall,
    [switch] $NoRun,
    [switch] $OpenOutput
)

$ErrorActionPreference = "Stop"

function Assert-Command {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name
    )

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw @"
Required command '$Name' was not found in the Windows PowerShell PATH.

Run this from WSL to install/check native Windows prerequisites:
  make package-windows-from-linux

Or install Windows-native prerequisites manually and retry the fallback path:
  make windows-package
"@
    }
}

function Test-WslSharePath {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Path
    )

    return $Path.StartsWith("\\wsl$") -or $Path.StartsWith("\\wsl.localhost")
}

if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $OutputDir = "D:\Brawler\Builds\latest"
}

$scriptRepo = Resolve-Path (Join-Path $PSScriptRoot "..\..")

if ([string]::IsNullOrWhiteSpace($WindowsRepo)) {
    $WindowsRepo = "D:\Brawler"
}

$WindowsRepo = (Resolve-Path $WindowsRepo).Path

if (Test-WslSharePath -Path $WindowsRepo) {
    throw @"
WindowsRepo points to a WSL path:
  $WindowsRepo

Use a native Windows checkout or Git worktree so the packaged app is built with
Windows node_modules and Windows Rust artifacts.
"@
}

Assert-Command "node"
Assert-Command "npm"
Assert-Command "cargo"

Write-Host "Brawler Windows package sanity path" -ForegroundColor Cyan
Write-Host "Repo:   $WindowsRepo"
Write-Host "Output: $OutputDir"
Write-Host "Node:   $(node --version)"
Write-Host "npm:    $(npm --version)"
Write-Host "Cargo:  $(cargo --version)"

Set-Location $WindowsRepo

if (-not $SkipInstall) {
    npm ci
}

npm run tauri -- build

$exePath = Join-Path $WindowsRepo "src-tauri\target\release\brawler.exe"

if (-not (Test-Path $exePath)) {
    throw "Expected packaged executable was not found: $exePath"
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$copiedExe = Join-Path $OutputDir "brawler.exe"
Copy-Item -Force $exePath $copiedExe

$bundleDir = Join-Path $WindowsRepo "src-tauri\target\release\bundle"
if (Test-Path $bundleDir) {
    $copiedBundleDir = Join-Path $OutputDir "bundle"
    New-Item -ItemType Directory -Force -Path $copiedBundleDir | Out-Null
    Copy-Item -Force -Recurse (Join-Path $bundleDir "*") $copiedBundleDir
}

Write-Host "Copied executable: $copiedExe" -ForegroundColor Green

if ($OpenOutput) {
    Start-Process explorer.exe $OutputDir | Out-Null
}

if (-not $NoRun) {
    Write-Host "Starting packaged app..." -ForegroundColor Green
    Start-Process -FilePath $copiedExe -WorkingDirectory $OutputDir | Out-Null
}
