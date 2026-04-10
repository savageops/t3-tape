import path from 'node:path';
import { spawnSync } from 'node:child_process';

import { describe, expect, it } from 'vitest';

import { renderMarkdown, runMigrationAutopilot } from '../src/index.js';

const cliPath = path.resolve('src/cli.js');

describe('migration-autopilot', () => {
  it('runs a real migration pipeline and auto-approves ready patches', () => {
    const result = runMigrationAutopilot({ keepTemp: false });

    expect(result.example).toBe('migration-autopilot');
    expect(result.refs.headBeforeUpdate).toBe(result.refs.headAfterAutomation);
    expect(result.automation.handoff.totalJobs).toBe(2);
    expect(result.automation.review.approvalCandidates).toHaveLength(3);
    expect(result.state.afterApproval.approvedPatchIds).toEqual([
      'PATCH-001',
      'PATCH-002',
      'PATCH-003'
    ]);
    expect(result.state.afterApproval.validate).toContain('OK');

    expect(
      result.automation.patches.some((patch) =>
        patch.detectedStatus === 'MISSING-SURFACE' &&
        patch.triageBeforeApproval === 'pending-review' &&
        patch.approved === true
      )
    ).toBe(true);
    expect(
      result.automation.patches.some((patch) =>
        patch.detectedStatus === 'CONFLICT' &&
        patch.triageBeforeApproval === 'pending-review' &&
        patch.approved === true
      )
    ).toBe(true);
  }, 120000);

  it('renders operator-grade markdown output', () => {
    const result = runMigrationAutopilot({ keepTemp: false });
    const markdown = renderMarkdown(result);
    expect(markdown).toContain('# Migration Autopilot');
    expect(markdown).toContain('Conflict/rederive jobs: 2');
    expect(markdown).toContain('MISSING-SURFACE -> pending-review -> approved=true');
    expect(markdown).toContain('CONFLICT -> pending-review -> approved=true');
    expect(markdown).toContain('decision: comment');
  }, 120000);

  it('supports plan-only mode from the CLI', () => {
    const result = spawnSync('node', [cliPath, '--format', 'json', '--no-auto-approve'], {
      encoding: 'utf8'
    });

    expect(result.status).toBe(0);
    const payload = JSON.parse(result.stdout);
    expect(payload.automation.review.approvalCandidates).toHaveLength(3);
    expect(payload.state.afterApproval.approvedPatchIds).toEqual([]);
  }, 120000);
});
