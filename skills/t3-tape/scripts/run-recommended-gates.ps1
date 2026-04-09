param(
    [Parameter(Mandatory = $true)][string]$RepoRoot,
    [switch]$PlanOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$commands = @(
    "pnpm install --frozen-lockfile",
    "pnpm -C packages/t3-tape-npm build",
    "pnpm -C packages/t3-tape-npm test",
    "pnpm run test:examples",
    "cargo test -p t3-tape",
    "cargo build --release -p t3-tape",
    "powershell -ExecutionPolicy Bypass -File scripts/e2e.ps1"
)

if ($PlanOnly) {
    $commands | ForEach-Object { Write-Output $_ }
    exit 0
}

foreach ($command in $commands) {
    Write-Output "RUN $command"
    powershell -NoProfile -Command $command | Write-Output
    if ($LASTEXITCODE -ne 0) {
        throw "Gate failed: $command"
    }
}
