[CmdletBinding()]
param(
    [switch] $SkipInstall,
    [switch] $Build,
    [switch] $Check,
    [switch] $AllowWslPath
)

$ErrorActionPreference = "Stop"

function Assert-Command {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name
    )

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' was not found in PATH."
    }
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
Set-Location $repoRoot

$repoPath = (Get-Location).Path
$isWslShare = $repoPath.StartsWith("\\wsl$") -or $repoPath.StartsWith("\\wsl.localhost")

if ($isWslShare -and -not $AllowWslPath) {
    throw @"
This script is running from a WSL network path:
  $repoPath

Use a native Windows checkout or Git worktree for frequent hands-on Tauri testing.
That keeps Windows node_modules and Rust target artifacts separate from the WSL/Nix tree.

Override with -AllowWslPath only for a deliberate one-off experiment.
"@
}

Assert-Command "node"
Assert-Command "npm"
Assert-Command "cargo"

Write-Host "Brawler Windows desktop sanity path" -ForegroundColor Cyan
Write-Host "Repo: $repoPath"
Write-Host "Node: $(node --version)"
Write-Host "npm:  $(npm --version)"
Write-Host "Cargo: $(cargo --version)"

if (-not $SkipInstall) {
    npm ci
}

if ($Check) {
    npm run check
}

if ($Build) {
    npm run tauri -- build
} else {
    npm run tauri -- dev
}
