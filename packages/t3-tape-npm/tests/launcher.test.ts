import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { T3_TAPE_BINARY_PATH } from '../src/env.js';
import { resolveTargetPackage } from '../src/platform.js';
import { resolveBinaryPath } from '../src/resolve.js';

const packageRoot = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const repoRoot = path.resolve(fileURLToPath(new URL('../../..', import.meta.url)));
const cliPath = path.join(packageRoot, 'dist', 'cli.js');
const localBinaryPath = path.join(
  repoRoot,
  'target',
  'release',
  process.platform === 'win32' ? 't3-tape.exe' : 't3-tape'
);
const multiFileDiffFixture = path.join(
  repoRoot,
  'crates',
  't3-tape',
  'tests',
  'fixtures',
  'multi-file.diff'
);

type RunOptions = {
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  input?: string;
  allowedExitCodes?: number[];
};

let releaseBinaryBuilt = false;

function makeTempDir(): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), 't3-tape-npm-'));
}

function makeExitScript(exitCode: number): string {
  const tempDir = makeTempDir();
  const scriptPath = path.join(
    tempDir,
    process.platform === 'win32' ? 'exit-script.cmd' : 'exit-script.sh'
  );

  if (process.platform === 'win32') {
    fs.writeFileSync(scriptPath, `@echo off\r\nexit /b ${exitCode}\r\n`);
  } else {
    fs.writeFileSync(scriptPath, `#!/bin/sh\nexit ${exitCode}\n`);
    fs.chmodSync(scriptPath, 0o755);
  }

  return scriptPath;
}

function buildLocalBinary(): void {
  if (releaseBinaryBuilt) {
    return;
  }

  const cargoBuild = spawnSync(
    'cargo',
    ['build', '--release', '--manifest-path', path.join(repoRoot, 'Cargo.toml'), '-p', 't3-tape'],
    {
      cwd: repoRoot,
      encoding: 'utf8'
    }
  );
  if (cargoBuild.status !== 0) {
    throw new Error(
      `cargo build failed with status ${cargoBuild.status}\nstdout:\n${cargoBuild.stdout}\nstderr:\n${cargoBuild.stderr}`
    );
  }
  expect(cargoBuild.status).toBe(0);
  releaseBinaryBuilt = true;
}

function runCommand(filePath: string, args: string[], options: RunOptions = {}) {
  const result = spawnSync(filePath, args, {
    cwd: options.cwd ?? repoRoot,
    env: {
      ...process.env,
      ...options.env
    },
    encoding: 'utf8',
    input: options.input
  });

  if (result.error) {
    throw result.error;
  }

  const allowedExitCodes = options.allowedExitCodes ?? [0];
  if (!allowedExitCodes.includes(result.status ?? -1)) {
    throw new Error(
      `command failed: ${filePath} ${args.join(' ')}\nstatus: ${result.status}\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`
    );
  }

  return result;
}

function runBinary(args: string[], options: RunOptions = {}) {
  buildLocalBinary();
  return runCommand(localBinaryPath, args, options);
}

function git(repoRootPath: string, args: string[]): string {
  return runCommand('git', args, { cwd: repoRootPath }).stdout.trim();
}

function configureGitIdentity(repoRootPath: string): void {
  git(repoRootPath, ['config', 'user.name', 'T3 Tape Test']);
  git(repoRootPath, ['config', 'user.email', 't3-tape-test@example.com']);
  git(repoRootPath, ['config', 'core.autocrlf', 'false']);
}

function writeFile(filePath: string, content: string): void {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, content, 'utf8');
}

function readJson(filePath: string): any {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function setupRepoPair(): { tempDir: string; upstream: string; fork: string } {
  const tempDir = makeTempDir();
  const upstream = path.join(tempDir, 'upstream');
  const fork = path.join(tempDir, 'fork');

  fs.mkdirSync(upstream, { recursive: true });
  git(upstream, ['init']);
  configureGitIdentity(upstream);
  writeFile(path.join(upstream, 'src', 'app.txt'), 'alpha\nbase\n');
  writeFile(path.join(upstream, 'src', 'plugin.txt'), 'core\n');
  git(upstream, ['add', '.']);
  git(upstream, ['commit', '-m', 'baseline', '--quiet']);

  runCommand('git', ['clone', upstream, fork], { cwd: tempDir });
  configureGitIdentity(fork);
  writeFile(path.join(fork, 'src', 'app.txt'), 'alpha\nbase\n');
  writeFile(path.join(fork, 'src', 'plugin.txt'), 'core\n');

  return { tempDir, upstream, fork };
}

function initManagedFork(fork: string, upstream: string): void {
  runBinary(['init', '--upstream', upstream, '--base-ref', 'HEAD'], { cwd: fork });
}

function commitAll(repoRootPath: string, message: string): void {
  git(repoRootPath, ['add', '.']);
  git(repoRootPath, ['commit', '-m', message, '--quiet']);
}

function commitChange(repoRootPath: string, relativePath: string, content: string, message: string): string {
  writeFile(path.join(repoRootPath, relativePath), content);
  commitAll(repoRootPath, message);
  return git(repoRootPath, ['rev-parse', 'HEAD']);
}

function deleteAndCommit(repoRootPath: string, relativePath: string, message: string): string {
  fs.rmSync(path.join(repoRootPath, relativePath));
  git(repoRootPath, ['add', '-A']);
  git(repoRootPath, ['commit', '-m', message, '--quiet']);
  return git(repoRootPath, ['rev-parse', 'HEAD']);
}

function setConfigValue(fork: string, keyPath: string[], value: unknown): void {
  const configPath = path.join(fork, '.t3', 'config.json');
  const config = readJson(configPath);
  let current = config;
  for (const key of keyPath.slice(0, -1)) {
    current = current[key];
  }
  current[keyPath[keyPath.length - 1]!] = value;
  fs.writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`, 'utf8');
}

function configureExecAgent(fork: string, endpoint: string, threshold = 0.8): void {
  setConfigValue(fork, ['agent', 'provider'], 'exec');
  setConfigValue(fork, ['agent', 'endpoint'], endpoint);
  setConfigValue(fork, ['agent', 'confidence-threshold'], threshold);
}

function createExecAgentScript(tempDir: string, name: string, response: unknown): string {
  const responsePath = path.join(tempDir, `${name}-response.json`);
  writeFile(responsePath, `${JSON.stringify(response, null, 2)}\n`);

  if (process.platform === 'win32') {
    const scriptPath = path.join(tempDir, `${name}.cmd`);
    writeFile(scriptPath, `@echo off\r\ntype "${responsePath}"\r\n`);
    return scriptPath;
  }

  const scriptPath = path.join(tempDir, `${name}.sh`);
  writeFile(scriptPath, `#!/bin/sh\ncat ${JSON.stringify(responsePath)}\n`);
  fs.chmodSync(scriptPath, 0o755);
  return scriptPath;
}

describe('platform mapping', () => {
  it('resolves supported target metadata deterministically', () => {
    expect(resolveTargetPackage('win32', 'x64')).toMatchObject({
      targetTriple: 'x86_64-pc-windows-msvc',
      packageName: '@t3-tape/t3-tape-x86_64-pc-windows-msvc',
      binaryName: 't3-tape.exe'
    });
    expect(resolveTargetPackage('linux', 'x64')).toMatchObject({
      targetTriple: 'x86_64-unknown-linux-gnu',
      packageName: '@t3-tape/t3-tape-x86_64-unknown-linux-gnu',
      binaryName: 't3-tape'
    });
    expect(resolveTargetPackage('darwin', 'x64')).toMatchObject({
      targetTriple: 'x86_64-apple-darwin',
      packageName: '@t3-tape/t3-tape-x86_64-apple-darwin'
    });
    expect(resolveTargetPackage('darwin', 'arm64')).toMatchObject({
      targetTriple: 'aarch64-apple-darwin',
      packageName: '@t3-tape/t3-tape-aarch64-apple-darwin'
    });
  });

  it('rejects unsupported platform and arch clearly', () => {
    expect(() => resolveTargetPackage('linux', 'arm64')).toThrowError(
      'Unsupported platform/arch linux/arm64.'
    );
  });
});

describe('binary resolution', () => {
  it('prefers the explicit env override', () => {
    const overridePath = makeExitScript(0);
    const resolved = resolveBinaryPath({
      env: {
        [T3_TAPE_BINARY_PATH]: overridePath
      }
    });

    expect(resolved).toMatchObject({
      source: 'env',
      binaryPath: path.resolve(overridePath)
    });
  });

  it('reports missing packaged targets with actionable text', () => {
    const emptyRoot = makeTempDir();
    expect(() =>
      resolveBinaryPath({
        env: {},
        platform: 'win32',
        arch: 'x64',
        packageRoot: emptyRoot
      })
    ).toThrowError(
      'Missing packaged target @t3-tape/t3-tape-x86_64-pc-windows-msvc for win32/x64.'
    );
  });
});

describe('cli integration', () => {
  it('preserves non-zero exit codes from the resolved binary', () => {
    buildLocalBinary();
    const emptyRepo = makeTempDir();
    const result = spawnSync(process.execPath, [cliPath, 'validate'], {
      cwd: emptyRepo,
      env: {
        ...process.env,
        [T3_TAPE_BINARY_PATH]: localBinaryPath
      },
      encoding: 'utf8'
    });

    expect(result.status).toBe(2);
  });

  it('runs the locally built rust binary through T3_TAPE_BINARY_PATH', () => {
    buildLocalBinary();

    const result = spawnSync(process.execPath, [cliPath, '--help'], {
      env: {
        ...process.env,
        [T3_TAPE_BINARY_PATH]: localBinaryPath
      },
      encoding: 'utf8'
    });

    expect(result.status).toBe(0);
    expect(result.stdout).toContain('Usage:');
  });

  it('runs the built release binary through the main PatchMD workflow', () => {
    const { tempDir, upstream, fork } = setupRepoPair();
    initManagedFork(fork, upstream);

    const preCommitHook = runBinary(['hooks', 'print', 'pre-commit'], { cwd: fork });
    expect(preCommitHook.stdout).toContain('t3-tape validate --staged');

    writeFile(path.join(fork, 'src', 'app.txt'), 'alpha\npatched\n');
    const patchAdd = runBinary(
      [
        'patch',
        'add',
        '--title',
        'conflict-line-patch',
        '--intent',
        'Keep the forked line change when upstream rewrites the same line.',
        '--assert',
        'patched line persists after migration'
      ],
      { cwd: fork }
    );
    expect(patchAdd.stdout).toContain('created patch PATCH-001');
    expect(runBinary(['patch', 'list'], { cwd: fork }).stdout).toContain(
      'PATCH-001\tconflict-line-patch\tactive'
    );
    expect(runBinary(['patch', 'show', 'PATCH-001'], { cwd: fork }).stdout).toContain(
      '## [PATCH-001] conflict-line-patch'
    );
    expect(runBinary(['patch', 'show', 'PATCH-001', '--diff'], { cwd: fork }).stdout).toContain(
      'PATCH-001.diff'
    );

    const validateJson = JSON.parse(runBinary(['--json', 'validate'], { cwd: fork }).stdout);
    expect(validateJson.status).toBe('ok');

    const exportPath = path.join(fork, 'CUSTOMIZATIONS.md');
    runBinary(['export', '--format', 'markdown', '--output', exportPath], { cwd: fork });
    expect(fs.readFileSync(exportPath, 'utf8')).toContain('## [PATCH-001] conflict-line-patch');

    commitAll(fork, 'record conflict patch');

    writeFile(path.join(fork, 'src', 'plugin.txt'), 'plugin\n');
    const secondPatch = runBinary(
      [
        'patch',
        'add',
        '--title',
        'clean-plugin-patch',
        '--intent',
        'Keep the plugin file across unrelated upstream changes.'
      ],
      { cwd: fork }
    );
    expect(secondPatch.stdout).toContain('created patch PATCH-002');
    commitAll(fork, 'record clean patch');

    writeFile(path.join(fork, '.t3', 'reports', 'example-summary.md'), 'foreign report content\n');
    expect(runBinary(['validate'], { cwd: fork }).stdout).toBe('OK\n');

    const agentEndpoint = createExecAgentScript(tempDir, 'conflict-agent', {
      'resolved-diff':
        'diff --git a/src/app.txt b/src/app.txt\n--- a/src/app.txt\n+++ b/src/app.txt\n@@ -1,2 +1,2 @@\n alpha\n-upstream\n+patched\n',
      confidence: 0.93,
      notes: 'Reapplied the fork intent against the upstream rewrite.',
      unresolved: []
    });
    configureExecAgent(fork, agentEndpoint, 0.8);

    const headBefore = git(fork, ['rev-parse', 'HEAD']);
    const toRef = commitChange(upstream, path.join('src', 'app.txt'), 'alpha\nupstream\n', 'upstream churn');

    const updateResult = runBinary(['update', '--ref', toRef], { cwd: fork });
    expect(updateResult.stdout).toContain('pending-review');
    expect(updateResult.stdout).toContain('CLEAN');

    expect(runBinary(['triage'], { cwd: fork }).stdout).toContain('PATCH-001\tconflict-line-patch\tpending-review');
    const triageJson = JSON.parse(runBinary(['--json', 'triage'], { cwd: fork }).stdout);
    expect(triageJson.patches).toHaveLength(2);
    expect(triageJson.patches[0]['triage-status']).toBe('pending-review');
    expect(triageJson.patches[1]['triage-status']).toBe('CLEAN');

    expect(runBinary(['triage', 'approve', 'PATCH-001'], { cwd: fork }).stdout).toContain(
      'PATCH-001\tactive'
    );
    expect(runBinary(['triage', 'approve', 'PATCH-002'], { cwd: fork }).stdout).toContain(
      'PATCH-002\tactive\tCOMPLETE'
    );
    runBinary(['hooks', 'install', 'pre-commit'], { cwd: fork });
    expect(fs.existsSync(path.join(fork, '.git', 'hooks', 'pre-commit'))).toBe(true);
    expect(git(fork, ['rev-parse', 'HEAD'])).toBe(headBefore);
    expect(runBinary(['validate'], { cwd: fork }).stdout).toBe('OK\n');
    expect(fs.readFileSync(path.join(fork, '.t3', 'migration.log'), 'utf8')).toContain('COMPLETE');
  });

  it('imports a diff through the built release binary', () => {
    const { fork, upstream } = setupRepoPair();
    initManagedFork(fork, upstream);

    const importResult = runBinary(['patch', 'import', '--diff', multiFileDiffFixture], {
      cwd: fork,
      input:
        'y\nalpha-import\nDescribe the alpha import.\nbeta-import\nDescribe the beta import.\n'
    });
    expect(importResult.stdout).toContain('created patch PATCH-001');
    expect(importResult.stdout).toContain('created patch PATCH-002');

    const patchList = runBinary(['patch', 'list'], { cwd: fork }).stdout;
    expect(patchList).toContain('PATCH-001\talpha-import\tactive');
    expect(patchList).toContain('PATCH-002\tbeta-import\tactive');
  });

  it('rederives a missing surface through the built release binary', () => {
    const { tempDir, upstream, fork } = setupRepoPair();
    initManagedFork(fork, upstream);

    writeFile(path.join(fork, 'src', 'app.txt'), 'alpha\npatched\n');
    runBinary(
      [
        'patch',
        'add',
        '--title',
        'missing-surface',
        '--intent',
        'Keep the patch intent when the original file disappears upstream.'
      ],
      { cwd: fork }
    );
    commitAll(fork, 'record missing-surface patch');

    const toRef = deleteAndCommit(upstream, path.join('src', 'app.txt'), 'remove tracked file');
    const updateResult = runBinary(['update', '--ref', toRef], {
      cwd: fork,
      allowedExitCodes: [3]
    });
    expect(updateResult.stdout).toContain('NEEDS-YOU');

    const agentEndpoint = createExecAgentScript(tempDir, 'rederive-agent', {
      'derived-diff':
        'diff --git a/src/app.txt b/src/app.txt\nnew file mode 100644\n--- /dev/null\n+++ b/src/app.txt\n@@ -0,0 +1,2 @@\n+alpha\n+patched\n',
      confidence: 0.92,
      'scope-update': {
        files: ['src/app.txt'],
        components: []
      },
      notes: 'Recreated the missing surface from intent.',
      unresolved: []
    });
    configureExecAgent(fork, agentEndpoint, 0.8);

    const rederiveResult = runBinary(['rederive', 'PATCH-001'], { cwd: fork });
    expect(rederiveResult.stdout).toContain('pending-review');

    const triageJson = JSON.parse(runBinary(['--json', 'triage'], { cwd: fork }).stdout);
    expect(triageJson.patches[0]['detected-status']).toBe('MISSING-SURFACE');
    expect(triageJson.patches[0]['triage-status']).toBe('pending-review');

    expect(runBinary(['triage', 'approve', 'PATCH-001'], { cwd: fork }).stdout).toContain(
      'PATCH-001\tactive\tCOMPLETE'
    );
    expect(runBinary(['validate'], { cwd: fork }).stdout).toBe('OK\n');
  });
});
