import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

import { describe, expect, it } from 'vitest';

import {
  buildApproveCommand,
  buildRederiveCommand,
  buildTriageCommand,
  buildUpdateCommand,
  parsePatchRegistry,
  readStateSurface,
  resolveStateDir,
  summarizeTriageCounts
} from '../../agent-kit/index.js';
import { copyDir, makeTempDir, writeJson, writeText } from '../../shared/test-helpers.js';
import {
  buildAgentJob,
  buildHandoffPacket,
  renderMarkdown,
  resolveAgentMode
} from '../src/index.js';

const cliPath = path.resolve('src/cli.js');
const fixtureRoot = path.resolve('../fixtures/agent-demo');
const fixtureStateDir = path.join(fixtureRoot, '.t3');

function cloneFixture() {
  const tempDir = makeTempDir('agent-handoff-builder-');
  const stateDir = path.join(tempDir, '.t3');
  copyDir(fixtureStateDir, stateDir);
  return stateDir;
}

describe('resolveStateDir', () => {
  it.each([
    ['examples/fixtures/agent-demo', path.resolve('examples/fixtures/agent-demo/.t3')],
    ['examples/fixtures/agent-demo/.t3', path.resolve('examples/fixtures/agent-demo/.t3')],
    ['.', path.resolve('.t3')],
    ['..', path.resolve('../.t3')],
    [fixtureStateDir, path.resolve(fixtureStateDir)],
    [fixtureRoot, path.resolve(fixtureRoot, '.t3')]
  ])('resolves %s', (input, expected) => {
    expect(resolveStateDir(input)).toBe(expected);
  });
});

describe('parsePatchRegistry', () => {
  const patchMd = fs.readFileSync(path.join(fixtureStateDir, 'patch.md'), 'utf8');

  it.each([
    ['PATCH-001', 'plugin-settings-toolbar-button', 'active', 'src/toolbar.tsx', 2],
    ['PATCH-002', 'command-palette-plugin-bridge', 'active', 'src/editor/commands.ts', 2],
    ['PATCH-003', 'schema-change-cache-reset', 'active', 'scripts/ci-cache-reset.mjs', 2]
  ])('parses patch %s', (id, title, status, surface, assertionCount) => {
    const patch = parsePatchRegistry(patchMd).find((entry) => entry.id === id);
    expect(patch).toMatchObject({ id, title, status, surface });
    expect(patch.behaviorAssertions).toHaveLength(assertionCount);
  });

  it.each([
    ['PATCH-001', 'Add a settings button to the top-right toolbar so operators can open plugin configuration without leaving the editor shell.'],
    ['PATCH-002', 'Keep the plugin action reachable from the command palette even when upstream rearranges the editor command registry.'],
    ['PATCH-003', 'Reset the workspace cache when the patch schema changes so old artifacts do not leak across update runs.']
  ])('keeps intent for %s', (id, intent) => {
    const patch = parsePatchRegistry(patchMd).find((entry) => entry.id === id);
    expect(patch.intent).toBe(intent);
  });

  it.each([
    ['PATCH-001', 'savage'],
    ['PATCH-002', 'savage'],
    ['PATCH-003', 'savage']
  ])('keeps author metadata for %s', (id, author) => {
    const patch = parsePatchRegistry(patchMd).find((entry) => entry.id === id);
    expect(patch.author).toBe(author);
  });
});

describe('command builders', () => {
  it.each([
    [{ stateDir: 'repo/.t3', ref: 'v2.4.1' }, 't3-tape --state-dir repo/.t3 update --ref v2.4.1'],
    [{ repoRoot: 'repo', ref: 'v2.4.1', ci: true }, 't3-tape --repo-root repo update --ref v2.4.1 --ci'],
    [{ repoRoot: 'repo root', stateDir: 'repo root/.t3', ref: 'v2.4.1', confidenceThreshold: 0.9 }, 't3-tape --repo-root "repo root" --state-dir "repo root/.t3" update --ref v2.4.1 --confidence-threshold 0.9']
  ])('builds update command %#', (input, expected) => {
    expect(buildUpdateCommand(input)).toBe(expected);
  });

  it.each([
    [{ stateDir: 'repo/.t3', json: true }, 't3-tape --state-dir repo/.t3 triage --json'],
    [{ repoRoot: 'repo' }, 't3-tape --repo-root repo triage'],
    [{ stateDir: 'repo/.t3', patchId: 'PATCH-001' }, 't3-tape --state-dir repo/.t3 triage approve PATCH-001'],
    [{ stateDir: 'repo/.t3', patchId: 'PATCH-002' }, 't3-tape --state-dir repo/.t3 rederive PATCH-002']
  ])('builds follow-up command %#', (input, expected) => {
    if (expected.includes('approve')) {
      expect(buildApproveCommand(input)).toBe(expected);
    } else if (expected.includes('rederive')) {
      expect(buildRederiveCommand(input)).toBe(expected);
    } else {
      expect(buildTriageCommand(input)).toBe(expected);
    }
  });
});

describe('summarizeTriageCounts', () => {
  const surface = readStateSurface(fixtureStateDir);

  it.each([
    ['clean', 1],
    ['conflict', 0],
    ['missingSurface', 0],
    ['pendingReview', 1],
    ['needsYou', 1]
  ])('tracks %s', (key, expected) => {
    expect(summarizeTriageCounts(surface.triage)[key]).toBe(expected);
  });
});

describe('resolveAgentMode', () => {
  it.each([
    [{ detectedStatus: 'CONFLICT', triageStatus: 'NEEDS-YOU', agentMode: null }, 'conflict-resolution'],
    [{ detectedStatus: 'MISSING-SURFACE', triageStatus: 'NEEDS-YOU', agentMode: null }, 're-derivation'],
    [{ detectedStatus: 'CONFLICT', triageStatus: 'pending-review', agentMode: 'conflict-resolution' }, 'conflict-resolution'],
    [{ detectedStatus: 'MISSING-SURFACE', triageStatus: 'pending-review', agentMode: 're-derivation' }, 're-derivation'],
    [{ detectedStatus: 'CLEAN', triageStatus: 'CLEAN', agentMode: null }, 'conflict-resolution'],
    [{ detectedStatus: 'CONFLICT', triageStatus: 'MISSING-SURFACE', agentMode: null }, 're-derivation'],
    [{ detectedStatus: 'CONFLICT', triageStatus: 'CONFLICT', agentMode: null }, 'conflict-resolution']
  ])('chooses %s for %o', (patch, expected) => {
    expect(resolveAgentMode(patch)).toBe(expected);
  });
});

describe('buildAgentJob', () => {
  const surface = readStateSurface(fixtureStateDir);

  it.each([
    ['PATCH-001', 'conflict-resolution', 'pending-review', true],
    ['PATCH-002', 're-derivation', 'NEEDS-YOU', false]
  ])('builds job for %s', (patchId, mode, triageStatus, hasApproval) => {
    const patch = surface.triage.patches.find((entry) => entry.id === patchId);
    const job = buildAgentJob(surface, patch);
    expect(job.mode).toBe(mode);
    expect(job.triageStatus).toBe(triageStatus);
    expect(Boolean(job.approvalCommand)).toBe(hasApproval);
    expect(job.intent.length).toBeGreaterThan(20);
  });

  it('includes approval command for pending-review items', () => {
    const patch = surface.triage.patches.find((entry) => entry.id === 'PATCH-001');
    expect(buildAgentJob(surface, patch).approvalCommand).toContain('triage approve PATCH-001');
  });

  it('includes rederive command for missing-surface patches', () => {
    const patch = surface.triage.patches.find((entry) => entry.id === 'PATCH-002');
    expect(buildAgentJob(surface, patch).followUpCommand).toContain('rederive PATCH-002');
  });
});

describe('buildHandoffPacket', () => {
  it.each([
    [{}, ['PATCH-002']],
    [{ includePendingReview: true }, ['PATCH-001', 'PATCH-002']],
    [{ patchId: 'PATCH-001', includePendingReview: true }, ['PATCH-001']],
    [{ patchId: 'PATCH-002' }, ['PATCH-002']],
    [{ patchId: 'PATCH-003', includePendingReview: true }, []],
    [{ patchId: 'PATCH-404', includePendingReview: true }, []]
  ])('filters jobs for %o', (options, expectedPatchIds) => {
    const packet = buildHandoffPacket(fixtureStateDir, options);
    expect(packet.jobs.map((job) => job.patchId)).toEqual(expectedPatchIds);
  });

  it.each([
    [{}, 1, 0],
    [{ includePendingReview: true }, 1, 1]
  ])('counts jobs by mode for %o', (options, rederivation, conflictResolution) => {
    const packet = buildHandoffPacket(fixtureStateDir, options);
    expect(packet.jobsByMode.rederivation).toBe(rederivation);
    expect(packet.jobsByMode.conflictResolution).toBe(conflictResolution);
  });

  it('includes common commands', () => {
    const packet = buildHandoffPacket(fixtureStateDir, { includePendingReview: true });
    expect(packet.commands.some((command) => command.includes('triage --json'))).toBe(true);
    expect(packet.commands.some((command) => command.includes('update --ref v2.4.1'))).toBe(true);
  });

  it('preserves provider configuration in the packet', () => {
    const packet = buildHandoffPacket(fixtureStateDir, {});
    expect(packet.provider).toBe('exec');
    expect(packet.endpoint).toContain('agent-runner');
    expect(packet.confidenceThreshold).toBe(0.8);
    expect(packet.configuration.agent.maxAttempts).toBe(3);
    expect(packet.configuration.sandbox.previewCommand).toBe('pnpm test');
  });

  it('builds an automation workflow from config and triage state', () => {
    const packet = buildHandoffPacket(fixtureStateDir, { includePendingReview: true });
    expect(packet.workflow.name).toBe('agent-handoff-loop');
    expect(packet.workflow.stages.map((stage) => stage.id)).toEqual([
      'read-triage',
      'dispatch-agents',
      'refresh-triage',
      'approve-ready'
    ]);
    expect(packet.workflow.stages[1].notes.some((note) => note.includes('max-attempts=3'))).toBe(true);
    expect(packet.workflow.gateConditions.some((note) => note.includes('preview-command configured'))).toBe(true);
  });

  it('warns when a filtered patch is missing from patch.md', () => {
    const stateDir = cloneFixture();
    const triage = JSON.parse(fs.readFileSync(path.join(stateDir, 'patch', 'triage.json'), 'utf8'));
    triage.patches.push({
      id: 'PATCH-777',
      title: 'ghost-patch',
      'detected-status': 'MISSING-SURFACE',
      'triage-status': 'NEEDS-YOU',
      'merged-upstream-candidate': false,
      'apply-stderr': 'missing',
      confidence: 0.2,
      'agent-mode': 're-derivation',
      notes: 'missing',
      unresolved: ['ghost'],
      'resolved-diff-path': null,
      'notes-path': null,
      'raw-response-path': null,
      'apply-commit': null,
      approved: false
    });
    writeJson(path.join(stateDir, 'patch', 'triage.json'), triage);

    const packet = buildHandoffPacket(stateDir, {});
    expect(packet.warnings.some((warning) => warning.includes('PATCH-777'))).toBe(true);
  });

  it('treats provider none as configuration-needed', () => {
    const stateDir = cloneFixture();
    const config = JSON.parse(fs.readFileSync(path.join(stateDir, 'patch', 'config.json'), 'utf8'));
    config.agent.endpoint = '';
    delete config.agent.provider;
    writeJson(path.join(stateDir, 'patch', 'config.json'), config);

    const packet = buildHandoffPacket(stateDir, {});
    expect(packet.provider).toBe('none');
    expect(packet.jobs[0].needsConfiguration).toBe(true);
  });

  it('renders markdown output', () => {
    const packet = buildHandoffPacket(fixtureStateDir, { includePendingReview: true });
    const output = renderMarkdown(packet);
    expect(output).toContain('# Agent Handoff Queue');
    expect(output).toContain('PATCH-001 conflict-resolution');
    expect(output).toContain('PATCH-002 re-derivation');
    expect(output).toContain('Automation loop:');
    expect(output).toContain('Dispatch agent jobs');
  });
});

describe('cli', () => {
  it('prints help output', () => {
    const result = spawnSync('node', [cliPath, '--help'], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(0);
    expect(result.stdout).toContain('agent-handoff-builder');
  });

  it('fails without a state dir', () => {
    const result = spawnSync('node', [cliPath], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('Missing required option');
  });

  it('fails for unsupported formats', () => {
    const result = spawnSync('node', [cliPath, '--state-dir', fixtureStateDir, '--format', 'xml'], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('Unsupported format');
  });

  it.each([
    [['--state-dir', fixtureStateDir], 1, 'json'],
    [['--state-dir', fixtureStateDir, '--include-pending-review'], 2, 'json'],
    [['--state-dir', fixtureStateDir, '--patch', 'PATCH-002'], 1, 'json'],
    [['--state-dir', fixtureStateDir, '--patch', 'PATCH-001', '--include-pending-review'], 1, 'json'],
    [['--state-dir', fixtureStateDir, '--format', 'markdown'], 1, 'markdown'],
    [['--state-dir', fixtureStateDir, '--patch', 'PATCH-404'], 0, 'json']
  ])('supports args %o', (args, expectedJobs, format) => {
    const result = spawnSync('node', [cliPath, ...args], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });

    expect(result.status).toBe(0);
    if (format === 'markdown') {
      expect(result.stdout).toContain('Jobs: 1');
    } else {
      expect(JSON.parse(result.stdout).totalJobs).toBe(expectedJobs);
    }
  });

  it('prints markdown jobs with patch identifiers', () => {
    const result = spawnSync('node', [cliPath, '--state-dir', fixtureStateDir, '--format', 'markdown', '--include-pending-review'], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });

    expect(result.status).toBe(0);
    expect(result.stdout).toContain('PATCH-001');
    expect(result.stdout).toContain('PATCH-002');
  });

  it('works when the patch registry is edited in a temp fixture', () => {
    const stateDir = cloneFixture();
    const patchMdPath = path.join(stateDir, 'patch.md');
    writeText(patchMdPath, fs.readFileSync(patchMdPath, 'utf8').replace('PATCH-002', 'PATCH-020'));

    const result = spawnSync('node', [cliPath, '--state-dir', stateDir], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });

    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout).warnings.length).toBeGreaterThan(0);
  });
});
