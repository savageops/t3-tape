import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { readStateSurface } from '../../agent-kit/index.js';
import { buildHandoffPacket } from '../../agent-handoff-builder/src/index.js';
import { buildReviewReport } from '../../migration-review-assistant/src/index.js';
import { uniqueValues } from '../../shared/collections.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../../..');
const isWindows = process.platform === 'win32';

let cachedBinaryPath = null;

function quote(value) {
  const rendered = String(value);
  if (!/[\s"]/u.test(rendered)) {
    return rendered;
  }

  return `"${rendered.replace(/"/g, '\\"')}"`;
}

function ensureParent(filePath) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
}

function writeText(filePath, content) {
  ensureParent(filePath);
  fs.writeFileSync(filePath, content.replace(/\r?\n/g, '\n'));
}

function writeJson(filePath, value) {
  writeText(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function runChecked(filePath, args, options = {}) {
  const result = spawnSync(filePath, args, {
    cwd: options.cwd ?? repoRoot,
    env: { ...process.env, ...(options.env ?? {}) },
    input: options.input ?? undefined,
    encoding: 'utf8'
  });

  const stdout = result.stdout?.trim() ?? '';
  const stderr = result.stderr?.trim() ?? '';
  const exitCode = result.status ?? 1;
  const allowedExitCodes = options.allowedExitCodes ?? [0];
  const command = [filePath, ...args.map(quote)].join(' ');

  if (!allowedExitCodes.includes(exitCode)) {
    throw new Error(
      `Command failed (${exitCode}): ${command}\n${[stdout, stderr].filter(Boolean).join('\n')}`
    );
  }

  return {
    filePath,
    args,
    command,
    cwd: options.cwd ?? repoRoot,
    stdout,
    stderr,
    exitCode
  };
}

function git(cwd, args) {
  return runChecked('git', args, { cwd });
}

function detectBinaryCandidates(binaryPath) {
  const binaryName = isWindows ? 't3-tape.exe' : 't3-tape';
  return uniqueValues([
    binaryPath,
    process.env.T3_TAPE_BINARY_PATH,
    path.join(repoRoot, 'target', 'debug', binaryName),
    path.join(repoRoot, 'target', 'release', binaryName)
  ]);
}

export function resolveBinaryPath(options = {}) {
  if (cachedBinaryPath && fs.existsSync(cachedBinaryPath)) {
    return cachedBinaryPath;
  }

  for (const candidate of detectBinaryCandidates(options.binaryPath)) {
    if (candidate && fs.existsSync(candidate)) {
      cachedBinaryPath = path.resolve(candidate);
      return cachedBinaryPath;
    }
  }

  runChecked('cargo', ['build', '-p', 't3-tape'], { cwd: repoRoot });
  const built = path.join(repoRoot, 'target', 'debug', isWindows ? 't3-tape.exe' : 't3-tape');
  cachedBinaryPath = built;
  return built;
}

function createWorkspaceRoot(prefix = 't3-tape-migration-autopilot-') {
  return fs.mkdtempSync(path.join(os.tmpdir(), prefix));
}

function configureGitIdentity(repoPath) {
  git(repoPath, ['config', 'user.name', 'T3 Tape Example']);
  git(repoPath, ['config', 'user.email', 'example@t3-tape.local']);
  git(repoPath, ['config', 'core.autocrlf', 'false']);
}

function cloneRepo(source, destination) {
  runChecked('git', ['clone', source, destination], { cwd: path.dirname(destination) });
}

function recordPatch(binaryPath, forkRoot, relativePath, content, title, intent) {
  writeText(path.join(forkRoot, relativePath), content);
  const patchAdd = runChecked(binaryPath, ['patch', 'add', '--title', title, '--intent', intent], {
    cwd: forkRoot
  });
  git(forkRoot, ['add', '.']);
  git(forkRoot, ['commit', '-m', title, '--quiet']);
  return patchAdd;
}

function latestCommit(repoPath) {
  return git(repoPath, ['rev-parse', 'HEAD']).stdout;
}

function writeAgentStub(workspaceRoot) {
  const agentPath = path.join(workspaceRoot, 'agent-stub.mjs');
  const script = `import fs from 'node:fs';

const request = JSON.parse(fs.readFileSync(0, 'utf8'));

if (request.mode === 'conflict-resolution') {
  process.stdout.write(JSON.stringify({
    'resolved-diff': 'diff --git a/src/app.txt b/src/app.txt\\n--- a/src/app.txt\\n+++ b/src/app.txt\\n@@ -1,2 +1,2 @@\\n alpha\\n-upstream\\n+patched\\n',
    confidence: 0.94,
    notes: 'Moved the forked toolbar intent onto the upstream rewrite.',
    unresolved: []
  }));
  process.exit(0);
}

if (request.mode === 're-derivation') {
  process.stdout.write(JSON.stringify({
    'derived-diff': 'diff --git a/src/legacy-command.txt b/src/legacy-command.txt\\nnew file mode 100644\\n--- /dev/null\\n+++ b/src/legacy-command.txt\\n@@ -0,0 +1,2 @@\\n+legacy-action\\n+plugin-action\\n',
    confidence: 0.92,
    'scope-update': {
      files: ['src/legacy-command.txt'],
      components: ['LegacyCommandRegistry']
    },
    notes: 'Recreated the plugin command after upstream removed the original command file.',
    unresolved: []
  }));
  process.exit(0);
}

process.stderr.write('Unsupported agent mode: ' + request.mode + '\\n');
process.exit(1);
`;
  writeText(agentPath, script);
  return agentPath;
}

function configureAgent(forkRoot, agentPath) {
  const configPath = path.join(forkRoot, '.t3', 'patch', 'config.json');
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  config.agent.provider = 'exec';
  config.agent.endpoint = `node ${quote(agentPath)}`;
  config.agent['confidence-threshold'] = 0.8;
  config.agent['max-attempts'] = 3;
  config.sandbox['preview-command'] = '';
  writeJson(configPath, config);
}

function createAssertionResults(triage) {
  return {
    patches: triage.patches.map((patch) => ({
      id: patch.id,
      passed: 2,
      failed: 0,
      'failing-assertions': []
    }))
  };
}

function mapPatchSummary(beforeApproval, afterApproval, reviewReport) {
  const approvalCommands = new Map(
    reviewReport.approvalCandidates.map((candidate) => [candidate.patchId, candidate.command])
  );
  const afterById = new Map(afterApproval.patches.map((patch) => [patch.id, patch]));

  return beforeApproval.patches.map((patch) => ({
    id: patch.id,
    title: patch.title,
    detectedStatus: patch.detectedStatus,
    triageBeforeApproval: patch.triageStatus,
    confidence: patch.confidence,
    agentMode: patch.agentMode,
    approvalCommand: approvalCommands.get(patch.id) ?? null,
    approved: afterById.get(patch.id)?.approved ?? false,
    finalTriageStatus: afterById.get(patch.id)?.triageStatus ?? patch.triageStatus
  }));
}

function stageRecord(name, commandResult, detail = {}) {
  return {
    name,
    command: commandResult.command,
    exitCode: commandResult.exitCode,
    stdout: commandResult.stdout,
    stderr: commandResult.stderr,
    ...detail
  };
}

export function runMigrationAutopilot(options = {}) {
  const binaryPath = resolveBinaryPath(options);
  const workspaceRoot = options.workspaceRoot ?? createWorkspaceRoot();
  const upstreamRoot = path.join(workspaceRoot, 'upstream');
  const forkRoot = path.join(workspaceRoot, 'fork');
  const stages = [];

  fs.mkdirSync(upstreamRoot, { recursive: true });
  git(upstreamRoot, ['init']);
  configureGitIdentity(upstreamRoot);

  writeText(path.join(upstreamRoot, 'src', 'app.txt'), 'alpha\nbase\n');
  writeText(path.join(upstreamRoot, 'src', 'plugin.txt'), 'core\n');
  writeText(path.join(upstreamRoot, 'src', 'legacy-command.txt'), 'legacy-action\n');
  writeText(path.join(upstreamRoot, 'src', 'new-command-registry.txt'), 'core-action\n');
  git(upstreamRoot, ['add', '.']);
  git(upstreamRoot, ['commit', '-m', 'baseline', '--quiet']);

  cloneRepo(upstreamRoot, forkRoot);
  configureGitIdentity(forkRoot);

  const initResult = runChecked(binaryPath, ['init', '--upstream', upstreamRoot, '--base-ref', 'HEAD'], {
    cwd: forkRoot
  });
  stages.push(stageRecord('init', initResult));

  stages.push(stageRecord(
    'record-primary-customization',
    recordPatch(
      binaryPath,
      forkRoot,
      'src/app.txt',
      'alpha\npatched\n',
      'primary-line-customization',
      'Keep the forked app line change when upstream rewrites the same surface.'
    )
  ));

  stages.push(stageRecord(
    'record-plugin-customization',
    recordPatch(
      binaryPath,
      forkRoot,
      'src/plugin.txt',
      'core\nplugin-enabled\n',
      'plugin-bridge-customization',
      'Keep the plugin bridge file active when upstream changes unrelated files.'
    )
  ));

  stages.push(stageRecord(
    'record-command-customization',
    recordPatch(
      binaryPath,
      forkRoot,
      'src/legacy-command.txt',
      'legacy-action\nplugin-action\n',
      'command-palette-customization',
      'Keep the plugin command reachable even when the old command registry disappears.'
    )
  ));

  const headBeforeUpdate = latestCommit(forkRoot);
  const agentPath = writeAgentStub(workspaceRoot);
  configureAgent(forkRoot, agentPath);

  writeText(path.join(upstreamRoot, 'src', 'app.txt'), 'alpha\nupstream\n');
  fs.rmSync(path.join(upstreamRoot, 'src', 'legacy-command.txt'));
  writeText(path.join(upstreamRoot, 'src', 'new-command-registry.txt'), 'core-action\n');
  writeText(path.join(upstreamRoot, 'README.md'), '# upstream churn\n');
  git(upstreamRoot, ['add', '-A']);
  git(upstreamRoot, ['commit', '-m', 'upstream churn', '--quiet']);
  const toRef = latestCommit(upstreamRoot);

  const updateResult = runChecked(binaryPath, ['update', '--ref', toRef], { cwd: forkRoot });
  stages.push(stageRecord('update', updateResult, { toRef }));

  const stateDir = path.join(forkRoot, '.t3');
  const surfaceBeforeApproval = readStateSurface(stateDir);
  const assertions = createAssertionResults(surfaceBeforeApproval.triage);
  const handoff = buildHandoffPacket(surfaceBeforeApproval, { includePendingReview: true });
  const review = buildReviewReport(surfaceBeforeApproval, assertions);

  const triageResult = runChecked(binaryPath, ['triage', '--json'], { cwd: forkRoot });
  stages.push(stageRecord('triage-before-approval', triageResult));

  const approvalResults = [];
  if (options.autoApproveReady !== false) {
    for (const candidate of review.approvalCandidates) {
      const result = runChecked(binaryPath, ['triage', 'approve', candidate.patchId], { cwd: forkRoot });
      approvalResults.push(stageRecord(`approve-${candidate.patchId}`, result, { patchId: candidate.patchId }));
      stages.push(approvalResults[approvalResults.length - 1]);
    }
  }

  const validateResult = runChecked(binaryPath, ['validate'], { cwd: forkRoot });
  stages.push(stageRecord('validate-current-state', validateResult));

  const surfaceAfterApproval = readStateSurface(stateDir);
  const triageAfterApproval = runChecked(binaryPath, ['triage', '--json'], { cwd: forkRoot });
  stages.push(stageRecord('triage-final-snapshot', triageAfterApproval));

  const result = {
    example: 'migration-autopilot',
    workspaceRoot,
    keptWorkspace: options.keepTemp === true,
    binaryPath,
    repo: {
      upstreamRoot,
      forkRoot
    },
    refs: {
      headBeforeUpdate,
      toRef,
      headAfterAutomation: latestCommit(forkRoot)
    },
    automation: {
      handoff,
      review,
      approvalsRun: approvalResults.map((stage) => stage.patchId),
      patches: mapPatchSummary(surfaceBeforeApproval.triage, surfaceAfterApproval.triage, review)
    },
    state: {
      beforeApproval: {
        triageCounts: {
          clean: handoff.triageCounts.clean,
          pendingReview: handoff.triageCounts.pendingReview,
          needsYou: handoff.triageCounts.needsYou
        }
      },
      afterApproval: {
        approvedPatchIds: surfaceAfterApproval.triage.patches
          .filter((patch) => patch.approved)
          .map((patch) => patch.id),
        validate: validateResult.stdout || 'OK',
        triageCounts: surfaceAfterApproval.triage.patches.reduce((counts, patch) => {
          counts.total += 1;
          if (patch.approved) {
            counts.approved += 1;
          }
          return counts;
        }, { total: 0, approved: 0 })
      }
    },
    stages
  };

  if (!options.keepTemp) {
    fs.rmSync(workspaceRoot, { recursive: true, force: true });
    result.workspaceRoot = null;
    result.repo = null;
  }

  return result;
}

export function renderMarkdown(result) {
  const lines = [
    '# Migration Autopilot',
    '',
    'This example drives a real temp-repo migration through `t3-tape`, then reuses the supporting example tools to show how operators and bots consume the cycle.',
    '',
    `Binary: ${result.binaryPath}`,
    `Workspace kept: ${result.keptWorkspace ? 'yes' : 'no'}`,
    `Conflict/rederive jobs: ${result.automation.handoff.totalJobs}`,
    `Approval candidates: ${result.automation.review.approvalCandidates.length}`,
    `Approved patches: ${result.state.afterApproval.approvedPatchIds.length}`
  ];

  if (result.workspaceRoot) {
    lines.push(`Workspace: ${result.workspaceRoot}`);
  }

  lines.push('');
  lines.push('## Patch outcomes');
  for (const patch of result.automation.patches) {
    lines.push(`- ${patch.id} ${patch.detectedStatus} -> ${patch.triageBeforeApproval} -> approved=${patch.approved}`);
    if (patch.approvalCommand) {
      lines.push(`  approval: ${patch.approvalCommand}`);
    }
  }

  lines.push('');
  lines.push('## Automation stages');
  for (const stage of result.stages) {
    lines.push(`- ${stage.name}: exit ${stage.exitCode}`);
    lines.push(`  command: ${stage.command}`);
  }

  lines.push('');
  lines.push('## Handoff');
  for (const job of result.automation.handoff.jobs) {
    lines.push(`- ${job.patchId} ${job.mode} -> ${job.followUpCommand}`);
  }

  lines.push('');
  lines.push('## Review');
  lines.push(`- decision: ${result.automation.review.decision}`);
  for (const candidate of result.automation.review.approvalCandidates) {
    lines.push(`- candidate: ${candidate.patchId} ${candidate.command}`);
  }

  lines.push('');
  lines.push('## Final state');
  lines.push(`- validate: ${result.state.afterApproval.validate}`);
  lines.push(`- approved patches: ${result.state.afterApproval.approvedPatchIds.join(', ')}`);
  lines.push('');
  return `${lines.join('\n')}\n`;
}
