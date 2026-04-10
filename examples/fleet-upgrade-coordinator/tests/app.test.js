import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

import { describe, expect, it } from 'vitest';

import { writeJson } from '../../shared/test-helpers.js';
import {
  buildFleetPlan,
  buildProjectPlan,
  classifyProjectAction,
  classifyVersionDelta,
  compareVersions,
  pickTargetRelease,
  renderMarkdown
} from '../src/index.js';

const cliPath = path.resolve('src/cli.js');
const manifestPath = path.resolve('sample/manifest.json');
const releasesPath = path.resolve('sample/releases.json');
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
const releases = JSON.parse(fs.readFileSync(releasesPath, 'utf8'));

describe('compareVersions', () => {
  it.each([
    ['1.0.0', '1.0.0', 0],
    ['1.0.1', '1.0.0', 1],
    ['1.1.0', '1.0.9', 1],
    ['2.0.0', '1.9.9', 1],
    ['1.0.0', '1.0.1', -1],
    ['1.0.0', '1.1.0', -1],
    ['v2.4.1', '2.4.1', 0],
    ['v2.4.1-beta.1', '2.4.1', -1],
    ['3.0', '3.0.0', 0],
    ['3', '3.0.0', 0],
    ['3.0.1', '3', 1],
    ['10.0.0', '9.9.9', 1],
    ['1.10.0', '1.2.0', 1],
    ['1.2.0', '1.10.0', -1],
    ['v3.0.0-beta.2', 'v3.0.0-beta.1', 1],
    ['v3.0.0-beta.1', 'v3.0.0-beta.2', -1],
    ['v3.0.0', 'v3.0.0-beta.2', 1],
    ['v3.0.0-beta.2', 'v3.0.0', -1],
    ['v2.0.0-rc.1', 'v2.0.0-beta.5', 1],
    ['v2.0.0-beta.5', 'v2.0.0-rc.1', -1]
  ])('compares %s to %s', (left, right, expected) => {
    expect(compareVersions(left, right)).toBe(expected);
  });
});

describe('classifyVersionDelta', () => {
  it.each([
    ['v2.4.0', 'v2.4.1', 'patch'],
    ['v2.4.0', 'v2.5.0', 'minor'],
    ['v2.4.0', 'v3.0.0', 'major'],
    ['v2.4.0', 'v2.4.0', 'same'],
    ['v3.0.0-beta.1', 'v3.0.0-beta.2', 'patch'],
    ['v1.8.0', 'v1.8.1', 'patch'],
    ['v1.8.0', 'v1.9.0', 'minor'],
    ['v5.1.1', 'v6.0.0', 'major'],
    ['v2.4.1', 'v2.4.1', 'same'],
    ['v2.4.1', 'v2.4.0', 'same'],
    ['v3.0.0-beta.2', 'v3.1.0-beta.1', 'minor']
  ])('classifies %s to %s', (currentRef, targetRef, expected) => {
    expect(classifyVersionDelta(currentRef, targetRef)).toBe(expected);
  });
});

describe('pickTargetRelease', () => {
  it.each([
    ['editor-shell-fork', 'v2.4.1'],
    ['plugin-runtime-fork', 'v1.8.1'],
    ['preview-shell-fork', 'v3.0.0-beta.2'],
    ['docs-fork', 'v6.0.0']
  ])('picks a target release for %s', (projectId, expectedVersion) => {
    const project = manifest.projects.find((entry) => entry.id === projectId);
    expect(pickTargetRelease(project, releases).version).toBe(expectedVersion);
  });

  it('returns null when nothing is newer', () => {
    const project = { ...manifest.projects[0], 'current-ref': 'v9.0.0' };
    expect(pickTargetRelease(project, releases)).toBeNull();
  });

  it('ignores prereleases when a project does not accept them', () => {
    const project = {
      ...manifest.projects[2],
      'accept-prerelease': false,
      'update-channel': 'beta'
    };
    expect(pickTargetRelease(project, releases)).toBeNull();
  });

  it('prefers security releases over a later feature release', () => {
    const project = manifest.projects[0];
    expect(pickTargetRelease(project, releases).version).toBe('v2.4.1');
  });
});

describe('classifyProjectAction', () => {
  it.each([
    ['editor-shell-fork', 'run-now'],
    ['plugin-runtime-fork', 'hold'],
    ['preview-shell-fork', 'schedule'],
    ['docs-fork', 'hold']
  ])('classifies %s', (projectId, expectedAction) => {
    const project = manifest.projects.find((entry) => entry.id === projectId);
    const release = pickTargetRelease(project, releases);
    expect(classifyProjectAction(project, release).action).toBe(expectedAction);
  });

  it.each([
    [{ ...manifest.projects[0], 'auto-run': false }, pickTargetRelease(manifest.projects[0], releases), 'run-now'],
    [{ ...manifest.projects[0], 'has-open-triage': true }, pickTargetRelease(manifest.projects[0], releases), 'hold'],
    [{ ...manifest.projects[3], 'allow-breaking': true }, pickTargetRelease(manifest.projects[3], releases), 'schedule'],
    [{ ...manifest.projects[3], 'allow-breaking': false }, null, 'skip']
  ])('routes custom project policy %#', (project, release, expectedAction) => {
    expect(classifyProjectAction(project, release).action).toBe(expectedAction);
  });
});

describe('buildProjectPlan', () => {
  it.each([
    ['editor-shell-fork', 'run-now', 'v2.4.1'],
    ['plugin-runtime-fork', 'hold', 'v1.8.1'],
    ['preview-shell-fork', 'schedule', 'v3.0.0-beta.2'],
    ['docs-fork', 'hold', 'v6.0.0']
  ])('builds a project plan for %s', (projectId, expectedAction, expectedTarget) => {
    const project = manifest.projects.find((entry) => entry.id === projectId);
    const plan = buildProjectPlan(project, releases);
    expect(plan.action).toBe(expectedAction);
    expect(plan.targetRef).toBe(expectedTarget);
  });

  it('includes update commands when a release exists', () => {
    const plan = buildProjectPlan(manifest.projects[0], releases);
    expect(plan.command).toContain('t3-tape');
    expect(plan.command).toContain('update --ref');
  });

  it('leaves the command empty when no release is available', () => {
    const plan = buildProjectPlan(
      { ...manifest.projects[0], upstream: 'acme/missing', 'current-ref': 'v9.9.9' },
      releases
    );
    expect(plan.command).toBeNull();
    expect(plan.action).toBe('skip');
  });
});

describe('buildFleetPlan', () => {
  it('builds grouped counts', () => {
    expect(buildFleetPlan(manifest, releases).counts).toEqual({
      runNow: 1,
      schedule: 1,
      hold: 2,
      skip: 0
    });
  });

  it('builds workflow waves for schedulers and operators', () => {
    const plan = buildFleetPlan(manifest, releases);
    expect(plan.waves.runNow).toHaveLength(1);
    expect(plan.waves.schedule).toHaveLength(1);
    expect(plan.workflow.name).toBe('fleet-update-loop');
    expect(plan.workflow.stages.map((stage) => stage.id)).toEqual([
      'run-now-wave',
      'scheduled-wave',
      'blocked-wave'
    ]);
  });

  it('renders markdown output', () => {
    const output = renderMarkdown(buildFleetPlan(manifest, releases));
    expect(output).toContain('# Fleet Upgrade Plan');
    expect(output).toContain('editor-shell-fork');
    expect(output).toContain('docs-fork');
    expect(output).toContain('Automation waves:');
    expect(output).toContain('Execute immediate upgrades');
  });
});

describe('cli', () => {
  it('prints help output', () => {
    const result = spawnSync('node', [cliPath, '--help'], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(0);
    expect(result.stdout).toContain('fleet-upgrade-coordinator');
  });

  it('fails without a manifest', () => {
    const result = spawnSync('node', [cliPath, '--releases', releasesPath], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('Missing required option: --manifest');
  });

  it('fails without releases', () => {
    const result = spawnSync('node', [cliPath, '--manifest', manifestPath], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('Missing required option: --releases');
  });

  it('fails for unsupported formats', () => {
    const result = spawnSync('node', [cliPath, '--manifest', manifestPath, '--releases', releasesPath, '--format', 'xml'], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('Unsupported format');
  });

  it.each([
    [['--manifest', manifestPath, '--releases', releasesPath], 4, 'json'],
    [['--manifest', manifestPath, '--releases', releasesPath, '--format', 'markdown'], 4, 'markdown']
  ])('supports args %o', (args, expectedPlans, format) => {
    const result = spawnSync('node', [cliPath, ...args], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });

    expect(result.status).toBe(0);
    if (format === 'markdown') {
      expect(result.stdout).toContain('Run now: 1');
    } else {
      expect(JSON.parse(result.stdout).plans).toHaveLength(expectedPlans);
    }
  });

  it('handles modified manifests in temp files', () => {
    const tempManifestPath = path.resolve('sample/temp-manifest.json');
    const tempManifest = {
      projects: [
        {
          id: 'no-update',
          'repo-root': 'repos/no-update',
          'state-dir': 'repos/no-update/.t3',
          upstream: 'acme/editor-shell',
          'current-ref': 'v9.9.9',
          'update-channel': 'stable',
          'auto-run': true,
          'allow-breaking': false,
          'accept-prerelease': false,
          'confidence-threshold': 0.8,
          'has-open-triage': false,
          priority: 'low'
        }
      ]
    };
    writeJson(tempManifestPath, tempManifest);

    const result = spawnSync('node', [cliPath, '--manifest', tempManifestPath, '--releases', releasesPath], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });

    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout).plans[0].action).toBe('skip');
    fs.unlinkSync(tempManifestPath);
  });
});
