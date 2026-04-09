param(
    [string]$BinaryPath = "",
    [string]$RepoRoot = "",
    [switch]$UseBundledBinary
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$skillRoot = Split-Path -Parent $PSScriptRoot
$bundledBinary = Join-Path $skillRoot "assets\\bin\\windows-x64\\t3-tape.exe"
$repoBinary = if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    ""
} else {
    Join-Path $RepoRoot "target\\release\\t3-tape.exe"
}

$resolved = $null

if (-not [string]::IsNullOrWhiteSpace($BinaryPath) -and (Test-Path -LiteralPath $BinaryPath)) {
    $resolved = (Resolve-Path -LiteralPath $BinaryPath).Path
} elseif (-not [string]::IsNullOrWhiteSpace($env:T3_TAPE_BINARY_PATH) -and (Test-Path -LiteralPath $env:T3_TAPE_BINARY_PATH)) {
    $resolved = (Resolve-Path -LiteralPath $env:T3_TAPE_BINARY_PATH).Path
} elseif ($UseBundledBinary -and (Test-Path -LiteralPath $bundledBinary)) {
    $resolved = (Resolve-Path -LiteralPath $bundledBinary).Path
} elseif (-not [string]::IsNullOrWhiteSpace($repoBinary) -and (Test-Path -LiteralPath $repoBinary)) {
    $resolved = (Resolve-Path -LiteralPath $repoBinary).Path
} else {
    $resolved = "t3-tape"
}

Write-Output $resolved
