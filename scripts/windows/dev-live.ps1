[CmdletBinding()]
param(
    [int] $Port = 9222,
    [string] $ExePath,
    [string] $OutputDir = $env:BRAWLER_WINDOWS_OUT,
    [string] $CaptureDir,
    [switch] $DebugPolicyFallback
)

$ErrorActionPreference = "Stop"

# Launches the PORTABLE packaged Windows exe (built by `make package-windows-from-linux`
# / `make windows-package`) with WebView2's Chromium remote-debugging port open, so an
# agent on WSL can drive the real running app - real backend, real local SQLite DB -
# via chromium.connectOverCDP() instead of a human doing manual click-through testing.
# See ADR 0066 and docs/testing.md section Live drive.
#
# This is NOT `dev.ps1`: it does not run `npm run tauri -- dev` against source, it
# launches an already-built portable .exe, matching what a user actually runs.
#
# Invoked two ways: by a human in a Windows PowerShell, and NON-INTERACTIVELY from
# WSL by `make live-up` (powershell.exe -NoProfile -ExecutionPolicy Bypass -File ...).
# Keep it non-interactive: no Read-Host/pauses/prompts, and exit right after
# Start-Process - the WSL side owns waiting for the CDP endpoint to come up.

if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $OutputDir = "D:\Brawler\Builds\latest"
}

function Resolve-PortableExe {
    param(
        [string] $ExplicitPath,
        [string] $SearchDir
    )

    if (-not [string]::IsNullOrWhiteSpace($ExplicitPath)) {
        if (-not (Test-Path $ExplicitPath)) {
            throw "ExePath was given but does not exist: $ExplicitPath"
        }
        return (Resolve-Path $ExplicitPath).Path
    }

    if (-not (Test-Path $SearchDir)) {
        throw @"
Build output directory not found: $SearchDir

Build the portable executable first (from WSL):
  make package-windows-from-linux

Or point at an existing build with -OutputDir / `$env:BRAWLER_WINDOWS_OUT, or
name an exact file with -ExePath.
"@
    }

    # The versioned portable APP artifact only. A bare "brawler-*.exe" filter
    # also matches the brawler-mcp-stdio.exe adapter shipped beside it since
    # v0.52 — which has no window and no CDP, so live-up hung on it
    # (guardrail 2026-07-12).
    $candidate = Get-ChildItem -Path $SearchDir -Filter "brawler-*-windows-x64-portable.exe" -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    if (-not $candidate) {
        throw @"
No brawler-*.exe found under: $SearchDir

Build the portable executable first (from WSL):
  make package-windows-from-linux

Or point at an existing build with -OutputDir / `$env:BRAWLER_WINDOWS_OUT, or
name an exact file with -ExePath.
"@
    }

    return $candidate.FullName
}

$exe = Resolve-PortableExe -ExplicitPath $ExePath -SearchDir $OutputDir

Write-Host "Brawler live-drive launcher" -ForegroundColor Cyan
Write-Host "Executable: $exe"
Write-Host "CDP port:   $Port"
Write-Host ""

# WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS is read by the WebView2 runtime at process
# start and forwarded to the underlying Chromium - this is the supported way to open
# a remote-debugging port on a WebView2 app without any app-code changes. Deliberately
# scoped to this process's environment only (not machine/user-wide), and the port is
# never opened on a non-localhost interface - Chromium's remote-debugging server binds
# 127.0.0.1 by default and this script does not override that.
$browserArgs = "--remote-debugging-port=$Port"
# Extra Chromium args opt-in (CI: GitHub server images have no GPU, so the boot
# smoke passes --disable-gpu here). Never set implicitly for local live-drive.
if (-not [string]::IsNullOrWhiteSpace($env:BRAWLER_EXTRA_BROWSER_ARGS)) {
    $browserArgs = "$browserArgs $($env:BRAWLER_EXTRA_BROWSER_ARGS)"
}
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $browserArgs

# CI fallback: on the GitHub windows runner the env var demonstrably does not
# reach the WebView2 browser process (2026-07-29 run: webview children alive,
# frontend calling commands, yet no listener on the CDP port). The documented
# alternative is the per-exe HKCU policy, which the WebView2 loader reads
# regardless of environment inheritance. Opt-in (-DebugPolicyFallback) so local
# live-drive never leaves a stale policy on the owner's machine.
if ($DebugPolicyFallback) {
    $policyPath = "HKCU:\Software\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments"
    New-Item -Path $policyPath -Force | Out-Null
    $exeLeaf = Split-Path $exe -Leaf
    New-ItemProperty -Path $policyPath -Name $exeLeaf -Value $browserArgs -PropertyType String -Force | Out-Null
    Write-Host "Debug policy fallback: $exeLeaf -> $browserArgs"
}

Write-Host "Launching..." -ForegroundColor Green
# CaptureDir (CI diagnostics): a WebView2Environment creation failure surfaces on
# the exe's stderr, which Start-Process discards unless redirected.
if (-not [string]::IsNullOrWhiteSpace($CaptureDir)) {
    New-Item -ItemType Directory -Force -Path $CaptureDir | Out-Null
    $process = Start-Process -FilePath $exe -PassThru `
        -RedirectStandardOutput (Join-Path $CaptureDir "brawler-stdout.log") `
        -RedirectStandardError (Join-Path $CaptureDir "brawler-stderr.log")
} else {
    $process = Start-Process -FilePath $exe -PassThru
}

Write-Host ""
Write-Host "Brawler is starting with WebView2 remote debugging enabled." -ForegroundColor Green
Write-Host ""
Write-Host "Verify the debug endpoint is up (may take a few seconds):"
Write-Host "  curl http://localhost:$Port/json/version"
Write-Host ""
Write-Host "From WSL, drive it with Playwright (tests/live/, ADR 0066):"
Write-Host "  make live-drive"
Write-Host ""
Write-Host "This port grants full control of the app's UI to anything that can reach it."
Write-Host "It is bound to localhost only - do not port-forward or expose it beyond this"
Write-Host "machine, and close the app (PID $($process.Id)) when the testing session is done."
