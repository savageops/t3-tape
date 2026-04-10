import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

import { describe, expect, it } from 'vitest';

import { readStateSurface } from '../../agent-kit/index.js';
import { copyDir, makeTempDir, writeJson } from '../../shared/test-helpers.js';
import {
  buildReviewReport,
  classifyPatchFinding,
  normalizeAssertionResults,
  renderMarkdown
} from '../src/index.js';

const cliPath = path.resolve('src/cli.js');
const fixtureRoot = path.resolve('../fixtures/agent-demo');
const fixtureStateDir = path.join(fixtureRoot, '.t3');
const fixtureAssertionsPath = path.join(fixtureRoot, 'assertions.json');

function cloneFixture() {
  const tempDir = makeTempDir('migration-review-assistant-');
  const stateDir = path.join(tempDir, '.t3');
  copyDir(fixtureStateDir, stateDir);
  return stateDir;
}

describe('normalizeAssertionResults', () => {
  it.each([
    [null, 0],
    [{}, 0],
    [[{ id: 'PATCH-001', passed: 2, failed: 0 }], 1],
    [{ patches: [{ id: 'PATCH-001', passed: 2, failed: 0 }] }, 1],
    [{ 'PATCH-001': { passed: 2, failed: 0 } }, 1],
    [{ patches: [{ id: 'PATCH-002', passed: 1, 'failing-assertions': ['x'] }] }, 1],
    [{ patches: [{ id: 'PATCH-003', passed: 0, failed: 0 }] }, 1],
    [[{ id: 'PATCH-004', passed: 1, failingAssertions: ['a', 'a'] }], 1]
  ])('normalizes %#', (input, expectedSize) => {
    expect(Object.keys(normalizeAssertionResults(input))).toHaveLength(expectedSize);
  });

  it.each([
    [{ id: 'PATCH-001', passed: 2, failed: 0 }, 'passing', 0],
    [{ id: 'PATCH-002', passed: 1, failed: 1, 'failing-assertions': ['x'] }, 'failing', 1],
    [{ id: 'PATCH-003', passed: 0, failed: 0 }, 'unknown', 0],
    [{ id: 'PATCH-004', passed: 1, failingAssertions: ['a', 'a'] }, 'failing', 1],
    [{ id: 'PATCH-005', passed: 3, 'failing-assertions': [] }, 'passing', 0],
    [{ id: 'PATCH-006', failed: 2, 'failing-assertions': ['a', 'b'] }, 'failing', 2]
  ])('computes status for %o', (input, status, failed) => {
    const result = normalizeAssertionResults([input])[input.id];
    expect(result.status).toBe(status);
    expect(result.failed).toBe(failed);
  });

  it.each([
    [{ patches: [{ id: 'PATCH-010', passed: 2, failed: 0 }] }, 'PATCH-010', 'passing'],
    [{ patches: [{ id: 'PATCH-011', passed: 0, failed: 1, 'failing-assertions': ['x'] }] }, 'PATCH-011', 'failing'],
    [{ patches: [{ id: 'PATCH-012', failed: 2, 'failing-assertions': ['x', 'y'] }] }, 'PATCH-012', 'failing'],
    [{ 'PATCH-013': { passed: 1, failed: 0 } }, 'PATCH-013', 'passing'],
    [{ 'PATCH-014': { passed: 0, failed: 0 } }, 'PATCH-014', 'unknown'],
    [[{ id: 'PATCH-015', passed: 4, failed: 0 }], 'PATCH-015', 'passing'],
    [[{ id: 'PATCH-016', passed: 1, failingAssertions: ['a', 'a'] }], 'PATCH-016', 'failing'],
    [{ patches: [{ id: 'PATCH-017', passed: 1, 'failing-assertions': [] }] }, 'PATCH-017', 'passing']
  ])('preserves normalized record %#', (input, id, status) => {
    const record = normalizeAssertionResults(input)[id];
    expect(record.id).toBe(id);
    expect(record.status).toBe(status);
  });
});

describe('classifyPatchFinding', () => {
  const surface = readStateSurface(fixtureStateDir);

  it.each([
    ['PATCH-002', { passed: 1, failed: 1 }, 1],
    ['PATCH-001', { passed: 2, failed: 0 }, 3],
    ['PATCH-003', { passed: 2, failed: 0 }, null]
  ])('classifies %s', (patchId, assertion, priority) => {
    const patch = surface.triage.patches.find((entry) => entry.id === patchId);
    const finding = classifyPatchFinding(surface, patch, assertion);
    expect(finding?.priority ?? null).toBe(priority);
  });

  it.each([
    ['PATCH-001', { passed: 0, failed: 1, failingAssertions: ['a'] }, 1],
    ['PATCH-001', { passed: 2, failed: 0 }, 3],
    ['PATCH-002', { passed: 1, failed: 0 }, 1]
  ])('reacts to assertion results for %s', (patchId, assertion, priority) => {
    const patch = surface.triage.patches.find((entry) => entry.id === patchId);
    const finding = classifyPatchFinding(surface, patch, assertion);
    expect(finding.priority).toBe(priority);
  });

  it('flags confidence near the threshold', () => {
    const stateDir = cloneFixture();
    const triage = JSON.parse(fs.readFileSync(path.join(stateDir, 'patch', 'triage.json'), 'utf8'));
    triage.patches[0].confidence = 0.82;
    writeJson(path.join(stateDir, 'patch', 'triage.json'), triage);

    const surfaceWithLowConfidence = readStateSurface(stateDir);
    const patch = surfaceWithLowConfidence.triage.patches[0];
    const finding = classifyPatchFinding(surfaceWithLowConfidence, patch, { passed: 2, failed: 0 });
    expect(finding.priority).toBe(2);
  });

  it('flags merged-upstream candidates', () => {
    const stateDir = cloneFixture();
    const triage = JSON.parse(fs.readFileSync(path.join(stateDir, 'patch', 'triage.json'), 'utf8'));
    triage.patches[0]['merged-upstream-candidate'] = true;
    triage.patches[0].confidence = 0.95;
    writeJson(path.join(stateDir, 'patch', 'triage.json'), triage);

    const surfaceWithCandidate = readStateSurface(stateDir);
    const patch = surfaceWithCandidate.triage.patches[0];
    const finding = classifyPatchFinding(surfaceWithCandidate, patch, { passed: 2, failed: 0 }, { margin: 0.01 });
    expect(finding.priority).toBe(2);
  });

  it.each([
    [{ triageStatus: 'pending-review', approved: true }, { passed: 2, failed: 0 }, null],
    [{ triageStatus: 'CLEAN', approved: true }, { passed: 2, failed: 0 }, null],
    [{ triageStatus: 'pending-review', unresolved: ['x'], approved: false }, { passed: 2, failed: 0 }, 2],
    [{ triageStatus: 'pending-review', confidence: 0.81, approved: false }, { passed: 2, failed: 0 }, 2],
    [{ triageStatus: 'pending-review', confidence: 0.96, approved: false }, { passed: 2, failed: 0 }, 3],
    [{ triageStatus: 'CLEAN', approved: false }, { passed: 2, failed: 0 }, 3],
    [{ triageStatus: 'NEEDS-YOU', approved: false }, { passed: 2, failed: 0 }, 1],
    [{ triageStatus: 'MISSING-SURFACE', detectedStatus: 'MISSING-SURFACE', approved: false }, { passed: 2, failed: 0 }, 1]
  ])('handles patch-state variant %#', (overrides, assertion, expectedPriority) => {
    const patch = {
      ...surface.triage.patches[0],
      unresolved: [],
      confidence: 0.91,
      ...overrides
    };
    const finding = classifyPatchFinding(surface, patch, assertion);
    expect(finding?.priority ?? null).toBe(expectedPriority);
  });
});

describe('buildReviewReport', () => {
  const assertions = JSON.parse(fs.readFileSync(fixtureAssertionsPath, 'utf8'));

  it.each([
    ['request-changes', 1, 0],
    ['comment', 0, 1]
  ])('derives review decisions %#', (decision, previewExitCode, approvals) => {
    const stateDir = cloneFixture();
    const triage = JSON.parse(fs.readFileSync(path.join(stateDir, 'patch', 'triage.json'), 'utf8'));
    const localAssertions = JSON.parse(fs.readFileSync(fixtureAssertionsPath, 'utf8'));
    triage.preview['exit-code'] = previewExitCode;
    if (approvals === 1) {
      triage.patches[1]['triage-status'] = 'pending-review';
      triage.patches[1].confidence = 0.91;
      triage.patches[1].unresolved = [];
      triage.patches[1].notes = 'Agent rebuilt the command source against the new registry.';
      localAssertions.patches[1].failed = 0;
      localAssertions.patches[1]['failing-assertions'] = [];
    }
    writeJson(path.join(stateDir, 'patch', 'triage.json'), triage);

    const report = buildReviewReport(stateDir, localAssertions);
    expect(report.decision).toBe(decision);
  });

  it.each([
    [0, 1],
    [1, 1],
    [2, 0]
  ])('counts approval candidates when patch states change %#', (index, expectedCandidates) => {
    const stateDir = cloneFixture();
    const triage = JSON.parse(fs.readFileSync(path.join(stateDir, 'patch', 'triage.json'), 'utf8'));

    if (index === 0) {
      triage.patches[0]['triage-status'] = 'pending-review';
      triage.patches[0].approved = false;
      triage.patches[0].confidence = 0.91;
    }

    if (index === 1) {
      triage.patches[1]['triage-status'] = 'pending-review';
      triage.patches[1].confidence = 0.91;
      triage.patches[1].unresolved = [];
    }

    if (index === 2) {
      triage.patches[0].approved = true;
      triage.patches[1].approved = true;
    }

    writeJson(path.join(stateDir, 'patch', 'triage.json'), triage);
    const report = buildReviewReport(stateDir, assertions);
    expect(report.approvalCandidates).toHaveLength(expectedCandidates);
  });

  it('includes review comments and commands', () => {
    const report = buildReviewReport(fixtureStateDir, assertions);
    expect(report.reviewComments.length).toBeGreaterThan(0);
    expect(report.commands.some((command) => command.includes('triage --json'))).toBe(true);
  });

  it('builds queues and workflow stages for the review loop', () => {
    const report = buildReviewReport(fixtureStateDir, assertions);
    expect(report.queues.blockers).toContain('PATCH-002');
    expect(report.workflow.name).toBe('review-approval-loop');
    expect(report.workflow.stages.map((stage) => stage.id)).toEqual([
      'load-review-surface',
      'clear-blockers',
      'guarded-review',
      'approve-safe-patches'
    ]);
    expect(report.workflow.gateConditions.some((note) => note.includes('preview-command configured'))).toBe(true);
    expect(report.workflow.stages[1].status).toBe('action-required');
  });

  it('renders markdown output', () => {
    const report = buildReviewReport(fixtureStateDir, assertions);
    const output = renderMarkdown(report);
    expect(output).toContain('# Migration Review Report');
    expect(output).toContain('request-changes');
    expect(output).toContain('Review workflow:');
    expect(output).toContain('Approve safe patches');
  });

  it.each([
    [false, false, 'approve'],
    [true, false, 'comment'],
    [true, true, 'request-changes'],
    [false, true, 'approve']
  ])('derives final decision variants %#', (pendingReview, failingAssertion, expectedDecision) => {
    const stateDir = cloneFixture();
    const triage = JSON.parse(fs.readFileSync(path.join(stateDir, 'patch', 'triage.json'), 'utf8'));
    const localAssertions = JSON.parse(fs.readFileSync(fixtureAssertionsPath, 'utf8'));

    triage.patches[0]['triage-status'] = pendingReview ? 'pending-review' : 'CLEAN';
    triage.patches[0].approved = !pendingReview;
    triage.patches[1]['triage-status'] = pendingReview ? 'pending-review' : 'CLEAN';
    triage.patches[1].approved = !pendingReview;
    triage.patches[1].unresolved = [];
    triage.preview['exit-code'] = 0;
    localAssertions.patches[1].failed = failingAssertion ? 1 : 0;
    localAssertions.patches[1]['failing-assertions'] = failingAssertion ? ['x'] : [];

    writeJson(path.join(stateDir, 'patch', 'triage.json'), triage);
    const report = buildReviewReport(stateDir, localAssertions);
    expect(report.decision).toBe(expectedDecision);
  });

  it.each([
    [0, 2],
    [1, 2],
    [2, 0],
    [3, 0]
  ])('changes finding counts when triage mutates %#', (mode, expectedFindings) => {
    const stateDir = cloneFixture();
    const triage = JSON.parse(fs.readFileSync(path.join(stateDir, 'patch', 'triage.json'), 'utf8'));
    const localAssertions = JSON.parse(fs.readFileSync(fixtureAssertionsPath, 'utf8'));

    if (mode === 1) {
      triage.patches[1]['triage-status'] = 'pending-review';
      triage.patches[1].unresolved = [];
      localAssertions.patches[1].failed = 0;
      localAssertions.patches[1]['failing-assertions'] = [];
    }

    if (mode === 2) {
      triage.patches[0].approved = true;
      triage.patches[1].approved = true;
      triage.patches[1]['triage-status'] = 'CLEAN';
      triage.patches[1].unresolved = [];
      localAssertions.patches[1].failed = 0;
      localAssertions.patches[1]['failing-assertions'] = [];
    }

    if (mode === 3) {
      triage.patches.forEach((patch) => {
        patch.approved = true;
        patch['triage-status'] = 'CLEAN';
        patch.unresolved = [];
      });
      localAssertions.patches.forEach((patch) => {
        patch.failed = 0;
        patch['failing-assertions'] = [];
      });
    }

    writeJson(path.join(stateDir, 'patch', 'triage.json'), triage);
    const report = buildReviewReport(stateDir, localAssertions);
    expect(report.findings).toHaveLength(expectedFindings);
  });
});

describe('cli', () => {
  it('prints help output', () => {
    const result = spawnSync('node', [cliPath, '--help'], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(0);
    expect(result.stdout).toContain('migration-review-assistant');
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
    [['--state-dir', fixtureStateDir], 'request-changes', 'json'],
    [['--state-dir', fixtureStateDir, '--assertions', fixtureAssertionsPath], 'request-changes', 'json'],
    [['--state-dir', fixtureStateDir, '--format', 'markdown'], 'request-changes', 'markdown'],
    [['--state-dir', fixtureStateDir, '--assertions', fixtureAssertionsPath, '--format', 'markdown'], 'request-changes', 'markdown']
  ])('supports args %o', (args, expectedDecision, format) => {
    const result = spawnSync('node', [cliPath, ...args], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });

    expect(result.status).toBe(0);
    if (format === 'markdown') {
      expect(result.stdout).toContain(`Decision: ${expectedDecision}`);
    } else {
      expect(JSON.parse(result.stdout).decision).toBe(expectedDecision);
    }
  });

  it.each([
    [['--state-dir', fixtureStateDir], true],
    [['--state-dir', fixtureStateDir, '--assertions', fixtureAssertionsPath], true],
    [['--state-dir', fixtureStateDir, '--format', 'markdown'], true],
    [['--state-dir', fixtureStateDir, '--assertions', fixtureAssertionsPath, '--format', 'markdown'], true]
  ])('always emits review output %#', (args, expectedOutput) => {
    const result = spawnSync('node', [cliPath, ...args], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });

    expect(result.status).toBe(0);
    expect(result.stdout.length > 20).toBe(expectedOutput);
  });
});
