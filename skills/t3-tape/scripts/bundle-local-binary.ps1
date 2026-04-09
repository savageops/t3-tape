param(
    [Parameter(Mandatory = $true)][string]$SourceBinaryPath,
    [string]$DestinationRoot = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $SourceBinaryPath)) {
    throw "Source binary not found: $SourceBinaryPath"
}

$skillRoot = if ([string]::IsNullOrWhiteSpace($DestinationRoot)) {
    Split-Path -Parent $PSScriptRoot
} else {
    $DestinationRoot
}

$targetDir = Join-Path $skillRoot "assets\\bin\\windows-x64"
New-Item -ItemType Directory -Force -Path $targetDir | Out-Null

$targetBinary = Join-Path $targetDir "t3-tape.exe"
Copy-Item -LiteralPath $SourceBinaryPath -Destination $targetBinary -Force

$manifest = @{
    binary = "t3-tape.exe"
    source = $SourceBinaryPath
    bundled_at_utc = [DateTime]::UtcNow.ToString("o")
} | ConvertTo-Json -Depth 4

Set-Content -LiteralPath (Join-Path $targetDir "manifest.json") -Value $manifest -NoNewline

Write-Output $targetBinary
