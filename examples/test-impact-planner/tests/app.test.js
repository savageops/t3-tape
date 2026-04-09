import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

import { describe, expect, it } from 'vitest';

import {
  buildPlan,
  formatPlan,
  globToRegExp,
  normalizeChangedFiles,
  pathMatchesPattern
} from '../src/index.js';
import { makeTempDir, writeJson } from '../../shared/test-helpers.js';

const cliPath = path.resolve('src/cli.js');

const manifest = {
  defaultCommands: ['pnpm test'],
  defaultOwners: ['platform'],
  ignore: ['docs/**', '**/*.md'],
  fullRunRules: [
    {
      id: 'lockfiles',
      match: ['pnpm-lock.yaml', 'Cargo.lock'],
      commands: ['pnpm test', 'cargo test -p t3-tape'],
      owners: ['release'],
      labels: ['full-run'],
      risk: 'high',
      reason: 'Lockfiles changed.'
    }
  ],
  rules: [
    {
      id: 'rust',
      match: ['crates/**', 'Cargo.toml'],
      commands: ['cargo test -p t3-tape'],
      owners: ['cli'],
      labels: ['rust'],
      risk: 'high',
      reason: 'Rust source changed.'
    },
    {
      id: 'node',
      match: ['packages/**'],
      commands: ['pnpm -C packages/t3-tape-npm test'],
      owners: ['launcher'],
      labels: ['node'],
      risk: 'medium',
      reason: 'Node launcher changed.'
    },
    {
      id: 'examples',
      match: ['examples/**'],
      commands: ['pnpm run test:examples'],
      owners: ['examples'],
      labels: ['examples'],
      risk: 'medium',
      reason: 'Examples changed.'
    }
  ]
};

describe('normalizeChangedFiles', () => {
  it.each([
    [['README.md'], ['README.md']],
    [['./README.md'], ['README.md']],
    [['docs\\guide.md'], ['docs/guide.md']],
    [['docs\\guide.md', 'docs/guide.md'], ['docs/guide.md']],
    [['  crates/t3-tape/src/lib.rs  '], ['crates/t3-tape/src/lib.rs']],
    [['', 'README.md'], ['README.md']],
    [['./docs\\guide.md', './docs/guide.md'], ['docs/guide.md']],
    [['packages\\t3-tape-npm\\src\\cli.ts'], ['packages/t3-tape-npm/src/cli.ts']],
    [['./examples/dev-env-doctor/src/index.js'], ['examples/dev-env-doctor/src/index.js']],
    [['Cargo.toml', 'Cargo.toml'], ['Cargo.toml']],
    [['docs/one.md', 'docs/two.md'], ['docs/one.md', 'docs/two.md']],
    [[], []]
  ])('normalizes %o', (input, expected) => {
    expect(normalizeChangedFiles(input)).toEqual(expected);
  });
});

describe('glob matching', () => {
  it.each([
    ['crates/t3-tape/src/lib.rs', 'crates/**', true],
    ['crates/t3-tape/tests/update.rs', 'crates/**', true],
    ['packages/t3-tape-npm/src/cli.ts', 'packages/**', true],
    ['examples/dev-env-doctor/src/index.js', 'examples/**', true],
    ['README.md', '**/*.md', true],
    ['docs/guide.md', 'docs/**', true],
    ['docs/guide.md', 'packages/**', false],
    ['Cargo.toml', 'Cargo.toml', true],
    ['Cargo.lock', 'Cargo.toml', false],
    ['packages/t3-tape-npm/package.json', 'packages/*/package.json', true],
    ['packages/t3-tape-npm/src/cli.ts', 'packages/*/package.json', false],
    ['examples/dev-env-doctor/src/index.js', 'examples/*/src/*.js', true],
    ['examples/dev-env-doctor/tests/app.test.js', 'examples/*/tests/*.js', true],
    ['examples/dev-env-doctor/tests/app.test.js', 'examples/*/src/*.js', false],
    ['docs/nested/guide.md', 'docs/**', true],
    ['docs/nested/guide.md', 'docs/*.md', false],
    ['packages/t3-tape-npm/src/resolve.ts', '**/resolve.ts', true],
    ['packages/t3-tape-npm/src/resolve.ts', '**/cli.ts', false]
  ])('matches %s with %s', (file, pattern, expected) => {
    expect(pathMatchesPattern(file, pattern)).toBe(expected);
    expect(globToRegExp(pattern).test(file)).toBe(expected);
  });
});

describe('buildPlan', () => {
  it.each([
    [['docs/guide.md'], 'docs-only', [], 'none'],
    [['README.md'], 'docs-only', [], 'none'],
    [['crates/t3-tape/src/lib.rs'], 'targeted', ['cargo test -p t3-tape'], 'high'],
    [['Cargo.toml'], 'targeted', ['cargo test -p t3-tape'], 'high'],
    [['packages/t3-tape-npm/src/cli.ts'], 'targeted', ['pnpm -C packages/t3-tape-npm test'], 'medium'],
    [['examples/dev-env-doctor/src/index.js'], 'targeted', ['pnpm run test:examples'], 'medium'],
    [['pnpm-lock.yaml'], 'full-run', ['pnpm test', 'cargo test -p t3-tape'], 'high'],
    [['Cargo.lock'], 'full-run', ['pnpm test', 'cargo test -p t3-tape'], 'high'],
    [['scripts/e2e.ps1'], 'fallback', ['pnpm test'], 'low'],
    [['scripts/e2e.ps1', 'docs/guide.md'], 'fallback', ['pnpm test'], 'low'],
    [['crates/t3-tape/src/lib.rs', 'packages/t3-tape-npm/src/cli.ts'], 'targeted', ['cargo test -p t3-tape', 'pnpm -C packages/t3-tape-npm test'], 'high'],
    [['crates/t3-tape/src/lib.rs', 'pnpm-lock.yaml'], 'full-run', ['pnpm test', 'cargo test -p t3-tape'], 'high'],
    [['examples/dev-env-doctor/src/index.js', 'packages/t3-tape-npm/src/cli.ts'], 'targeted', ['pnpm -C packages/t3-tape-npm test', 'pnpm run test:examples'], 'medium'],
    [['docs/guide.md', 'packages/t3-tape-npm/src/cli.ts'], 'targeted', ['pnpm -C packages/t3-tape-npm test'], 'medium'],
    [['unknown.file'], 'fallback', ['pnpm test'], 'low'],
    [['crates/t3-tape/src/lib.rs', 'unknown.file'], 'targeted', ['cargo test -p t3-tape'], 'high'],
    [['examples/dev-env-doctor/src/index.js', 'unknown.file'], 'targeted', ['pnpm run test:examples'], 'medium'],
    [['pnpm-lock.yaml', 'packages/t3-tape-npm/src/cli.ts'], 'full-run', ['pnpm test', 'cargo test -p t3-tape', 'pnpm -C packages/t3-tape-npm test'], 'high'],
    [['Cargo.lock', 'examples/dev-env-doctor/src/index.js'], 'full-run', ['pnpm test', 'cargo test -p t3-tape', 'pnpm run test:examples'], 'high'],
    [['docs/guide.md', 'README.md'], 'docs-only', [], 'none']
  ])('builds mode %s for %o', (files, expectedMode, expectedCommands, expectedRisk) => {
    const plan = buildPlan(manifest, files);
    expect(plan.mode).toBe(expectedMode);
    expect(plan.commands).toEqual(expectedCommands);
    expect(plan.risk).toBe(expectedRisk);
  });

  it('dedupes commands, owners, and labels', () => {
    const plan = buildPlan(manifest, [
      'examples/dev-env-doctor/src/index.js',
      'examples/test-impact-planner/src/index.js'
    ]);
    expect(plan.commands).toEqual(['pnpm run test:examples']);
    expect(plan.owners).toEqual(['examples']);
    expect(plan.labels).toEqual(['examples']);
  });

  it('tracks unmatched files', () => {
    const plan = buildPlan(manifest, ['scripts/e2e.ps1', 'crates/t3-tape/src/lib.rs']);
    expect(plan.unmatchedFiles).toEqual(['scripts/e2e.ps1']);
  });

  it('tracks ignored files', () => {
    const plan = buildPlan(manifest, ['docs/guide.md', 'packages/t3-tape-npm/src/cli.ts']);
    expect(plan.ignoredFiles).toEqual(['docs/guide.md']);
  });

  it('includes reasons from matching rules', () => {
    const plan = buildPlan(manifest, ['packages/t3-tape-npm/src/cli.ts']);
    expect(plan.reasons).toEqual(['Node launcher changed.']);
  });

  it('falls back to default owners when nothing matches', () => {
    const plan = buildPlan(manifest, ['scripts/e2e.ps1']);
    expect(plan.owners).toEqual(['platform']);
  });

  it('returns empty mode for no changes', () => {
    const plan = buildPlan(manifest, []);
    expect(plan).toMatchObject({
      mode: 'empty',
      risk: 'none',
      commands: []
    });
  });

  it('renders a readable plan', () => {
    const plan = buildPlan(manifest, ['packages/t3-tape-npm/src/cli.ts']);
    const output = formatPlan(plan);
    expect(output).toContain('Mode: targeted');
    expect(output).toContain('Run:');
    expect(output).toContain('Node launcher changed.');
  });
});

describe('cli', () => {
  it('prints help output', () => {
    const result = spawnSync('node', [cliPath, '--help'], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(0);
    expect(result.stdout).toContain('test-impact-planner');
  });

  it('fails without a manifest', () => {
    const result = spawnSync('node', [cliPath, '--changed', 'README.md'], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('Missing required option');
  });

  it('fails without changed files', () => {
    const tempDir = makeTempDir('test-impact-planner-');
    const manifestPath = path.join(tempDir, 'manifest.json');
    writeJson(manifestPath, manifest);
    const result = spawnSync('node', [cliPath, '--manifest', manifestPath], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('Provide --changes-file');
  });

  it.each([
    [['packages/t3-tape-npm/src/cli.ts'], 0, 'targeted'],
    [['pnpm-lock.yaml'], 2, 'full-run'],
    [['README.md'], 0, 'docs-only'],
    [['scripts/e2e.ps1'], 0, 'fallback'],
    [['examples/dev-env-doctor/src/index.js'], 0, 'targeted'],
    [['crates/t3-tape/src/lib.rs'], 0, 'targeted']
  ])('supports changed files %o', (files, expectedStatus, expectedMode) => {
    const tempDir = makeTempDir('test-impact-planner-');
    const manifestPath = path.join(tempDir, 'manifest.json');
    const changesPath = path.join(tempDir, 'changes.json');
    writeJson(manifestPath, manifest);
    writeJson(changesPath, files);

    const result = spawnSync('node', [cliPath, '--manifest', manifestPath, '--changes-file', changesPath, '--json'], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });

    expect(result.status).toBe(expectedStatus);
    expect(JSON.parse(result.stdout).mode).toBe(expectedMode);
  });

  it('supports inline --changed values', () => {
    const tempDir = makeTempDir('test-impact-planner-');
    const manifestPath = path.join(tempDir, 'manifest.json');
    writeJson(manifestPath, manifest);

    const result = spawnSync(
      'node',
      [cliPath, '--manifest', manifestPath, '--changed', 'packages/t3-tape-npm/src/cli.ts', '--json'],
      {
        cwd: path.resolve('.'),
        encoding: 'utf8'
      }
    );

    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout).commands).toEqual(['pnpm -C packages/t3-tape-npm test']);
  });
});
