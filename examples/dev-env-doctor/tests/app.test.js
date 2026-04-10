import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

import { describe, expect, it } from 'vitest';

import {
  buildDoctorReport,
  compareVersions,
  evaluateEnvVar,
  evaluateFile,
  evaluateService,
  evaluateTool,
  formatDoctorReport
} from '../src/index.js';
import { makeTempDir, writeJson } from '../../shared/test-helpers.js';

const cliPath = path.resolve('src/cli.js');

describe('compareVersions', () => {
  it.each([
    ['18.18.0', '18.18.0', 0],
    ['18.18.1', '18.18.0', 1],
    ['18.19.0', '18.18.9', 1],
    ['19.0.0', '18.99.99', 1],
    ['20.0.0', '20.0.1', -1],
    ['18.17.9', '18.18.0', -1],
    ['v20.11.1', '20.11.1', 0],
    ['v20.11.2', '20.11.1', 1],
    ['20.11', '20.11.0', 0],
    ['20.11.0-beta.1', '20.11.0', 0],
    ['1.2.3', '1.10.0', -1],
    ['1.10.0', '1.2.3', 1],
    ['1', '1.0.0', 0],
    ['1.0.0', '1', 0],
    ['', '1.0.0', -1],
    ['1.0.0', '', 1]
  ])('compares %s to %s', (actual, required, expected) => {
    expect(compareVersions(actual, required)).toBe(expected);
  });
});

describe('evaluateTool', () => {
  it.each([
    [{ name: 'node', minVersion: '18.18.0' }, { node: '20.0.0' }, 'ready'],
    [{ name: 'node', minVersion: '18.18.0' }, { node: '18.18.0' }, 'ready'],
    [{ name: 'node', minVersion: '18.18.0' }, { node: '18.17.9' }, 'blocked'],
    [{ name: 'node', minVersion: '18.18.0', required: false }, { node: '18.17.9' }, 'warning'],
    [{ name: 'pnpm', minVersion: '10.0.0' }, {}, 'blocked'],
    [{ name: 'pnpm', minVersion: '10.0.0', required: false }, {}, 'warning'],
    [{ name: 'cargo' }, { cargo: '1.86.0' }, 'ready'],
    [{ name: 'cargo', required: false }, {}, 'warning'],
    [{ name: 'git', minVersion: '2.45.0' }, { git: '2.45.1' }, 'ready'],
    [{ name: 'git', minVersion: '2.45.0' }, { git: '2.44.0' }, 'blocked']
  ])('returns a diagnostic for %o', (definition, tools, expectedStatus) => {
    expect(evaluateTool(definition, tools).status).toBe(expectedStatus);
  });
});

describe('evaluateEnvVar', () => {
  it.each([
    [{ name: 'TOKEN' }, { TOKEN: 'abc' }, 'ready'],
    [{ name: 'TOKEN' }, { TOKEN: '' }, 'blocked'],
    [{ name: 'TOKEN' }, {}, 'blocked'],
    [{ name: 'TOKEN', required: false }, {}, 'warning'],
    [{ name: 'MODE', allowedValues: ['dev', 'ci'] }, { MODE: 'dev' }, 'ready'],
    [{ name: 'MODE', allowedValues: ['dev', 'ci'] }, { MODE: 'ci' }, 'ready'],
    [{ name: 'MODE', allowedValues: ['dev', 'ci'] }, { MODE: 'prod' }, 'blocked'],
    [{ name: 'MODE', allowedValues: ['dev', 'ci'], required: false }, { MODE: 'prod' }, 'warning'],
    [{ name: 'PORT', allowedValues: ['3000'] }, { PORT: '3000' }, 'ready'],
    [{ name: 'PORT', allowedValues: ['3000'] }, { PORT: '8080' }, 'blocked']
  ])('validates env vars for %o', (definition, env, expectedStatus) => {
    expect(evaluateEnvVar(definition, env).status).toBe(expectedStatus);
  });
});

describe('evaluateFile', () => {
  it.each([
    [{ path: 'README.md' }, ['README.md'], 'ready'],
    [{ path: 'README.md' }, [], 'blocked'],
    [{ path: '.env.example', required: false }, [], 'warning'],
    [{ path: 'docs/guide.md' }, ['docs/guide.md'], 'ready'],
    [{ path: 'src/index.js' }, ['README.md', 'src/index.js'], 'ready']
  ])('validates files for %o', (definition, files, expectedStatus) => {
    expect(evaluateFile(definition, files).status).toBe(expectedStatus);
  });
});

describe('evaluateService', () => {
  it.each([
    [{ name: 'docker' }, { docker: { status: 'running', reachable: true } }, 'ready'],
    [{ name: 'docker', needReachable: true }, { docker: { status: 'running', reachable: false } }, 'blocked'],
    [{ name: 'docker', needReachable: true, required: false }, { docker: { status: 'running', reachable: false } }, 'warning'],
    [{ name: 'docker' }, { docker: { status: 'stopped', reachable: false } }, 'blocked'],
    [{ name: 'docker', required: false }, {}, 'warning'],
    [{ name: 'git' }, {}, 'blocked'],
    [{ name: 'git' }, { git: { status: 'running', reachable: false } }, 'ready'],
    [{ name: 'registry', needReachable: true }, { registry: { status: 'running', reachable: true } }, 'ready']
  ])('validates services for %o', (definition, services, expectedStatus) => {
    expect(evaluateService(definition, services).status).toBe(expectedStatus);
  });
});

describe('buildDoctorReport', () => {
  const baseProfile = {
    name: 'workspace',
    tools: [{ name: 'node', minVersion: '18.18.0' }],
    env: [{ name: 'TOKEN', required: false }],
    files: [{ path: 'README.md' }],
    services: [{ name: 'git' }]
  };

  it.each([
    [
      { tools: { node: '20.0.0' }, env: {}, files: ['README.md'], services: { git: { status: 'running', reachable: true } } },
      'warning'
    ],
    [
      { tools: { node: '18.0.0' }, env: {}, files: ['README.md'], services: { git: { status: 'running', reachable: true } } },
      'blocked'
    ],
    [
      { tools: { node: '20.0.0' }, env: { TOKEN: 'x' }, files: ['README.md'], services: { git: { status: 'running', reachable: true } } },
      'ready'
    ],
    [
      { tools: {}, env: {}, files: ['README.md'], services: { git: { status: 'running', reachable: true } } },
      'blocked'
    ],
    [
      { tools: { node: '20.0.0' }, env: {}, files: [], services: { git: { status: 'running', reachable: true } } },
      'blocked'
    ],
    [
      { tools: { node: '20.0.0' }, env: {}, files: ['README.md'], services: {} },
      'blocked'
    ],
    [
      { tools: { node: '20.0.0' }, env: {}, files: ['README.md'], services: { git: { status: 'stopped', reachable: false } } },
      'blocked'
    ],
    [
      { tools: { node: '20.0.0' }, env: { TOKEN: '' }, files: ['README.md'], services: { git: { status: 'running', reachable: true } } },
      'warning'
    ],
    [
      { tools: { node: '20.0.0' }, env: { TOKEN: 'x' }, files: ['README.md', 'extra.md'], services: { git: { status: 'running', reachable: true } } },
      'ready'
    ],
    [
      { tools: { node: '20.0.0' }, env: {}, files: ['README.md'], services: { git: { status: 'running', reachable: true } } },
      'warning'
    ]
  ])('computes top-level status', (snapshot, expectedStatus) => {
    expect(buildDoctorReport(baseProfile, snapshot).status).toBe(expectedStatus);
  });

  it('dedupes next steps', () => {
    const profile = {
      name: 'dup-steps',
      tools: [
        { name: 'node', minVersion: '20.0.0', fixHint: 'Install Node.' },
        { name: 'pnpm', minVersion: '10.0.0', fixHint: 'Install Node.' }
      ]
    };
    const report = buildDoctorReport(profile, { tools: { node: '18.0.0' } });
    expect(report.nextSteps).toEqual(['Install Node.']);
  });

  it('counts ready, warning, and blocked checks', () => {
    const report = buildDoctorReport(baseProfile, {
      tools: { node: '20.0.0' },
      env: {},
      files: ['README.md'],
      services: { git: { status: 'running', reachable: true } }
    });
    expect(report.counts).toEqual({
      total: 4,
      ready: 3,
      warning: 1,
      blocked: 0
    });
  });

  it('builds a remediation workflow for automation', () => {
    const report = buildDoctorReport(baseProfile, {
      tools: { node: '18.0.0' },
      env: {},
      files: [],
      services: {}
    });
    expect(report.workflow.name).toBe('environment-remediation-loop');
    expect(report.workflow.stages.map((stage) => stage.id)).toEqual([
      'fix-blockers',
      'clear-warnings',
      'rerun-readiness'
    ]);
    expect(report.workflow.gateConditions.some((note) => note.includes('blocked check'))).toBe(true);
  });

  it('renders a readable text report', () => {
    const report = buildDoctorReport(baseProfile, {
      tools: { node: '20.0.0' },
      env: {},
      files: ['README.md'],
      services: { git: { status: 'running', reachable: true } }
    });
    const output = formatDoctorReport(report);
    expect(output).toContain('Profile: workspace');
    expect(output).toContain('Warnings: 1');
    expect(output).toContain('[WARNING] env TOKEN');
    expect(output).toContain('Automation loop:');
    expect(output).toContain('Fix blockers');
  });
});

describe('cli', () => {
  it('prints help output', () => {
    const result = spawnSync('node', [cliPath, '--help'], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(0);
    expect(result.stdout).toContain('dev-env-doctor');
  });

  it('errors when profile is missing', () => {
    const result = spawnSync('node', [cliPath], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('Missing required option');
  });

  it.each([
    ['ready', { tools: { node: '20.0.0' }, files: ['README.md'], services: { git: { status: 'running', reachable: true } }, env: { TOKEN: 'x' } }, 0],
    ['warning', { tools: { node: '20.0.0' }, files: ['README.md'], services: { git: { status: 'running', reachable: true } }, env: {} }, 0],
    ['blocked-tool', { tools: { node: '18.0.0' }, files: ['README.md'], services: { git: { status: 'running', reachable: true } }, env: {} }, 2],
    ['blocked-file', { tools: { node: '20.0.0' }, files: [], services: { git: { status: 'running', reachable: true } }, env: {} }, 2],
    ['blocked-service', { tools: { node: '20.0.0' }, files: ['README.md'], services: {}, env: {} }, 2],
    ['blocked-env', { tools: { node: '20.0.0' }, files: ['README.md'], services: { git: { status: 'running', reachable: true } }, env: { TOKEN: '' } }, 0]
  ])('supports scenario %s', (_name, snapshot, expectedStatus) => {
    const tempDir = makeTempDir('dev-env-doctor-');
    const profilePath = path.join(tempDir, 'profile.json');
    const snapshotPath = path.join(tempDir, 'snapshot.json');
    const profile = {
      name: 'cli-profile',
      tools: [{ name: 'node', minVersion: '20.0.0' }],
      env: [{ name: 'TOKEN', required: false }],
      files: [{ path: 'README.md' }],
      services: [{ name: 'git' }]
    };

    writeJson(profilePath, profile);
    writeJson(snapshotPath, snapshot);

    const result = spawnSync('node', [cliPath, '--profile', profilePath, '--snapshot', snapshotPath, '--json'], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });

    expect(result.status).toBe(expectedStatus);
    expect(JSON.parse(result.stdout).profile).toBe('cli-profile');
  });

  it('fails for invalid json', () => {
    const tempDir = makeTempDir('dev-env-doctor-');
    const profilePath = path.join(tempDir, 'broken.json');
    fs.writeFileSync(profilePath, '{bad json');

    const result = spawnSync('node', [cliPath, '--profile', profilePath], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });

    expect(result.status).toBe(1);
    expect(result.stderr.length).toBeGreaterThan(0);
  });
});
