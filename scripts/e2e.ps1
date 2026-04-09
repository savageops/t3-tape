param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [string]$BinaryPath = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Utf8File {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Content
    )

    $parent = Split-Path -Parent $Path
    if ($parent) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }

    [System.IO.File]::WriteAllText($Path, $Content.Replace("`r`n", "`n"), [System.Text.UTF8Encoding]::new($false))
}

function Format-Argument {
    param([Parameter(Mandatory = $true)][string]$Value)

    if ([string]::IsNullOrEmpty($Value)) {
        return '""'
    }

    if ($Value -match '[\s"]') {
        return '"' + $Value.Replace('"', '\"') + '"'
    }

    return $Value
}

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [string]$WorkingDirectory = $RepoRoot,
        [int[]]$AllowedExitCodes = @(0)
    )

    $stdoutPath = Join-Path ([System.IO.Path]::GetTempPath()) ("t3-tape-e2e-" + [System.Guid]::NewGuid().ToString('N') + ".stdout.log")
    $stderrPath = Join-Path ([System.IO.Path]::GetTempPath()) ("t3-tape-e2e-" + [System.Guid]::NewGuid().ToString('N') + ".stderr.log")
    $argumentLine = ($Arguments | ForEach-Object { Format-Argument $_ }) -join ' '

    try {
        $process = Start-Process `
            -FilePath $FilePath `
            -ArgumentList $argumentLine `
            -WorkingDirectory $WorkingDirectory `
            -NoNewWindow `
            -Wait `
            -PassThru `
            -RedirectStandardOutput $stdoutPath `
            -RedirectStandardError $stderrPath

        $stdout = if (Test-Path -LiteralPath $stdoutPath) {
            $stdoutRaw = Get-Content -LiteralPath $stdoutPath -Raw -ErrorAction SilentlyContinue
            if ($null -eq $stdoutRaw) {
                ''
            }
            else {
                ([string]$stdoutRaw).TrimEnd("`r", "`n")
            }
        }
        else {
            ''
        }

        $stderr = if (Test-Path -LiteralPath $stderrPath) {
            $stderrRaw = Get-Content -LiteralPath $stderrPath -Raw -ErrorAction SilentlyContinue
            if ($null -eq $stderrRaw) {
                ''
            }
            else {
                ([string]$stderrRaw).TrimEnd("`r", "`n")
            }
        }
        else {
            ''
        }

        $output = @($stdout, $stderr) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
        $rendered = $output -join "`n"

        if ($AllowedExitCodes -notcontains $process.ExitCode) {
            throw "Command failed ($($process.ExitCode)): $FilePath $($Arguments -join ' ')`n$rendered"
        }

        return $rendered
    }
    finally {
        Remove-Item -LiteralPath $stdoutPath -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $stderrPath -ErrorAction SilentlyContinue
    }
}

function Invoke-Git {
    param(
        [Parameter(Mandatory = $true)][string]$Repo,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [int[]]$AllowedExitCodes = @(0)
    )

    Invoke-Checked -FilePath 'git' -Arguments $Arguments -WorkingDirectory $Repo -AllowedExitCodes $AllowedExitCodes
}

function Resolve-LatestSandbox {
    param([Parameter(Mandatory = $true)][string]$ForkRoot)

    $sandboxRoot = Join-Path $ForkRoot '.t3\sandbox'
    $dirs = @(Get-ChildItem -LiteralPath $sandboxRoot -Directory | Sort-Object Name)
    if ($dirs.Count -ne 1) {
        throw "Expected exactly one sandbox directory under $sandboxRoot but found $($dirs.Count)"
    }
    return $dirs[0].FullName
}

if ([string]::IsNullOrWhiteSpace($BinaryPath)) {
    $isWindowsHost = $env:OS -eq 'Windows_NT'
    $BinaryPath = if ($isWindowsHost) {
        Join-Path $RepoRoot 'target\release\t3-tape.exe'
    }
    else {
        Join-Path $RepoRoot 'target/release/t3-tape'
    }
}

if (-not (Test-Path -LiteralPath $BinaryPath)) {
    Invoke-Checked -FilePath 'cargo' -Arguments @('build', '--release', '--manifest-path', (Join-Path $RepoRoot 'Cargo.toml'), '-p', 't3-tape') | Out-Null
}

$workspace = Join-Path ([System.IO.Path]::GetTempPath()) ('t3-tape-e2e-' + [System.Guid]::NewGuid().ToString('N'))
$upstream = Join-Path $workspace 'upstream'
$fork = Join-Path $workspace 'fork'
New-Item -ItemType Directory -Force -Path $upstream | Out-Null

Invoke-Git -Repo $upstream -Arguments @('init') | Out-Null
Invoke-Git -Repo $upstream -Arguments @('config', 'user.name', 'T3 Tape E2E') | Out-Null
Invoke-Git -Repo $upstream -Arguments @('config', 'user.email', 't3-tape-e2e@example.com') | Out-Null
Invoke-Git -Repo $upstream -Arguments @('config', 'core.autocrlf', 'false') | Out-Null

Write-Utf8File -Path (Join-Path $upstream 'src\app.txt') -Content "alpha`nbase`n"
Write-Utf8File -Path (Join-Path $upstream 'src\plugin.txt') -Content "core`n"
Invoke-Git -Repo $upstream -Arguments @('add', '.') | Out-Null
Invoke-Git -Repo $upstream -Arguments @('commit', '-m', 'baseline', '--quiet') | Out-Null

Invoke-Checked -FilePath 'git' -Arguments @('clone', $upstream, $fork) -WorkingDirectory $workspace | Out-Null
Invoke-Git -Repo $fork -Arguments @('config', 'user.name', 'T3 Tape E2E') | Out-Null
Invoke-Git -Repo $fork -Arguments @('config', 'user.email', 't3-tape-e2e@example.com') | Out-Null
Invoke-Git -Repo $fork -Arguments @('config', 'core.autocrlf', 'false') | Out-Null
Write-Utf8File -Path (Join-Path $fork 'src\app.txt') -Content "alpha`nbase`n"
Write-Utf8File -Path (Join-Path $fork 'src\plugin.txt') -Content "core`n"

Invoke-Checked -FilePath $BinaryPath -Arguments @('init', '--upstream', $upstream, '--base-ref', 'HEAD') -WorkingDirectory $fork | Out-Null

Write-Utf8File -Path (Join-Path $fork 'src\app.txt') -Content "alpha`npatched`n"
Invoke-Checked -FilePath $BinaryPath -Arguments @('patch', 'add', '--title', 'conflict-line-patch', '--intent', 'Keep the forked line change when upstream rewrites the same line.') -WorkingDirectory $fork | Out-Null
Invoke-Git -Repo $fork -Arguments @('add', '.') | Out-Null
Invoke-Git -Repo $fork -Arguments @('commit', '-m', 'record conflict patch', '--quiet') | Out-Null

Write-Utf8File -Path (Join-Path $fork 'src\plugin.txt') -Content "plugin`n"
Invoke-Checked -FilePath $BinaryPath -Arguments @('patch', 'add', '--title', 'clean-plugin-patch', '--intent', 'Keep the plugin file across unrelated upstream changes.') -WorkingDirectory $fork | Out-Null
Invoke-Git -Repo $fork -Arguments @('add', '.') | Out-Null
Invoke-Git -Repo $fork -Arguments @('commit', '-m', 'record clean patch', '--quiet') | Out-Null

Write-Utf8File -Path (Join-Path $fork '.t3\reports\example-summary.md') -Content "foreign report content`n"
$validateOutput = Invoke-Checked -FilePath $BinaryPath -Arguments @('validate') -WorkingDirectory $fork
if ($validateOutput -notmatch '(^|\s)OK(\s|$)') {
    throw "Expected validate output to contain 'OK' but got:`n$validateOutput"
}

$agentResponsePath = Join-Path $workspace 'agent-response.json'
$resolvedDiff = @"
diff --git a/src/app.txt b/src/app.txt
--- a/src/app.txt
+++ b/src/app.txt
@@ -1,2 +1,2 @@
 alpha
-upstream
+patched
"@
$agentResponse = @"
{
  "resolved-diff": $(ConvertTo-Json $resolvedDiff -Compress),
  "confidence": 0.93,
  "notes": "Reapplied the fork intent against the upstream rewrite.",
  "unresolved": []
}
"@
Write-Utf8File -Path $agentResponsePath -Content $agentResponse
$agentScriptPath = Join-Path $workspace 'agent-stub.cmd'
$agentScript = @"
@echo off
type "$agentResponsePath"
"@
Write-Utf8File -Path $agentScriptPath -Content $agentScript

$configPath = Join-Path $fork '.t3\config.json'
$config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
$config.agent.provider = 'exec'
$config.agent.endpoint = $agentScriptPath
$config.agent.model = 'stub'
$config.agent.'confidence-threshold' = 0.8
$config.agent.'max-attempts' = 3
$config.agent.'parallel-rederivation' = $false
$config | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $configPath

$headBefore = (Invoke-Git -Repo $fork -Arguments @('rev-parse', 'HEAD')).Trim()
Write-Utf8File -Path (Join-Path $upstream 'src\app.txt') -Content "alpha`nupstream`n"
Write-Utf8File -Path (Join-Path $upstream 'README.md') -Content "# upstream`n"
Invoke-Git -Repo $upstream -Arguments @('add', '.') | Out-Null
Invoke-Git -Repo $upstream -Arguments @('commit', '-m', 'upstream churn', '--quiet') | Out-Null
$toRef = (Invoke-Git -Repo $upstream -Arguments @('rev-parse', 'HEAD')).Trim()

$diffBeforeApproval = Get-Content -LiteralPath (Join-Path $fork '.t3\patches\PATCH-001.diff') -Raw
$updateOutput = Invoke-Checked -FilePath $BinaryPath -Arguments @('update', '--ref', $toRef) -WorkingDirectory $fork
$triageOutput = Invoke-Checked -FilePath $BinaryPath -Arguments @('triage') -WorkingDirectory $fork
$approveConflictOutput = Invoke-Checked -FilePath $BinaryPath -Arguments @('triage', 'approve', 'PATCH-001') -WorkingDirectory $fork
$approveCleanOutput = Invoke-Checked -FilePath $BinaryPath -Arguments @('triage', 'approve', 'PATCH-002') -WorkingDirectory $fork
$finalValidateOutput = Invoke-Checked -FilePath $BinaryPath -Arguments @('validate') -WorkingDirectory $fork

$headAfter = (Invoke-Git -Repo $fork -Arguments @('rev-parse', 'HEAD')).Trim()
if ($headBefore -ne $headAfter) {
    throw "Current branch head changed during update flow.`nBefore: $headBefore`nAfter:  $headAfter"
}

$diffAfterApproval = Get-Content -LiteralPath (Join-Path $fork '.t3\patches\PATCH-001.diff') -Raw
if ($diffBeforeApproval -eq $diffAfterApproval) {
    throw 'PATCH-001 diff was not rewritten during approval.'
}

$migrationLogPath = Join-Path $fork '.t3\migration.log'
$migrationLog = Get-Content -LiteralPath $migrationLogPath -Raw
if ($migrationLog -notmatch 'COMPLETE') {
    throw "Expected migration log to contain COMPLETE but got:`n$migrationLog"
}

$sandboxDir = Resolve-LatestSandbox -ForkRoot $fork
$triagePath = Join-Path $sandboxDir 'triage.json'

Write-Output 'E2E_VALIDATE_BEFORE:'
Write-Output $validateOutput.Trim()
Write-Output 'E2E_UPDATE:'
Write-Output $updateOutput.Trim()
Write-Output 'E2E_TRIAGE:'
Write-Output $triageOutput.Trim()
Write-Output 'E2E_APPROVE_PATCH_001:'
Write-Output $approveConflictOutput.Trim()
Write-Output 'E2E_APPROVE_PATCH_002:'
Write-Output $approveCleanOutput.Trim()
Write-Output 'E2E_VALIDATE_AFTER:'
Write-Output $finalValidateOutput.Trim()
Write-Output 'E2E_TRIAGE_PATH:'
Write-Output $triagePath
Write-Output 'E2E_MIGRATION_LOG:'
Write-Output $migrationLog.Trim()
Write-Output 'E2E_STATUS:'
Write-Output 'COMPLETE'
