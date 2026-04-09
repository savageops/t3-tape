import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

import { describe, expect, it } from 'vitest';

import {
  buildReleaseSummary,
  determineVersionBump,
  normalizeCommitEntry,
  parseCommitLine,
  renderMarkdown
} from '../src/index.js';
import { makeTempDir, writeJson } from '../../shared/test-helpers.js';

const cliPath = path.resolve('src/cli.js');

describe('parseCommitLine', () => {
  it.each([
    ['feat: add preview', { type: 'feat', scope: null, summary: 'add preview', breaking: false }],
    ['fix(cli): resolve approval bug', { type: 'fix', scope: 'cli', summary: 'resolve approval bug', breaking: false }],
    ['perf(update): skip redundant parsing', { type: 'perf', scope: 'update', summary: 'skip redundant parsing', breaking: false }],
    ['docs: expand README', { type: 'docs', scope: null, summary: 'expand README', breaking: false }],
    ['chore: refresh lockfile', { type: 'chore', scope: null, summary: 'refresh lockfile', breaking: false }],
    ['build(ci): publish checksums', { type: 'build', scope: 'ci', summary: 'publish checksums', breaking: false }],
    ['ci(actions): tighten matrix', { type: 'ci', scope: 'actions', summary: 'tighten matrix', breaking: false }],
    ['refactor(parser): simplify diff reader', { type: 'refactor', scope: 'parser', summary: 'simplify diff reader', breaking: false }],
    ['test(update): cover triage approval', { type: 'test', scope: 'update', summary: 'cover triage approval', breaking: false }],
    ['security: rotate webhook secret', { type: 'security', scope: null, summary: 'rotate webhook secret', breaking: false }],
    ['sec(auth): reject empty tokens', { type: 'sec', scope: 'auth', summary: 'reject empty tokens', breaking: false }],
    ['feat(ui)!: replace patch cards', { type: 'feat', scope: 'ui', summary: 'replace patch cards', breaking: true }],
    ['fix!: keep state durable', { type: 'fix', scope: null, summary: 'keep state durable', breaking: true }],
    ['release notes without conventional prefix', { type: 'other', scope: null, summary: 'release notes without conventional prefix', breaking: false }],
    ['BREAKING CHANGE: swap storage format', { type: 'other', scope: null, summary: 'BREAKING CHANGE: swap storage format', breaking: true }],
    [' feat(ui): trim spaces ', { type: 'feat', scope: 'ui', summary: 'trim spaces', breaking: false }],
    ['', { type: 'other', scope: null, summary: '', breaking: false }],
    ['docs(readme): explain CI', { type: 'docs', scope: 'readme', summary: 'explain CI', breaking: false }],
    ['perf(cache)!: bust old state', { type: 'perf', scope: 'cache', summary: 'bust old state', breaking: true }],
    ['unknown(scope): still parse', { type: 'unknown', scope: 'scope', summary: 'still parse', breaking: false }]
  ])('parses %s', (line, expected) => {
    expect(parseCommitLine(line)).toMatchObject(expected);
  });
});

describe('normalizeCommitEntry', () => {
  it.each([
    [{ type: 'feat', summary: 'add preview' }, { type: 'feat', scope: null, summary: 'add preview', breaking: false }],
    [{ type: 'fix', scope: 'cli', summary: 'resolve bug' }, { type: 'fix', scope: 'cli', summary: 'resolve bug', breaking: false }],
    [{ type: 'feat', scope: 'ui', summary: 'add preview', breaking: true }, { type: 'feat', scope: 'ui', summary: 'add preview', breaking: true }],
    [{ type: 'fix', summary: 'resolve bug', body: 'BREAKING CHANGE: storage format changed' }, { type: 'fix', summary: 'resolve bug', breaking: true }],
    [{ type: 'docs', scope: 'readme', summary: 'add setup guide' }, { type: 'docs', scope: 'readme', summary: 'add setup guide', breaking: false }],
    [{ type: 'security', summary: 'rotate token' }, { type: 'security', scope: null, summary: 'rotate token', breaking: false }],
    [{ type: 'perf', scope: 'update', summary: 'skip diff parsing' }, { type: 'perf', scope: 'update', summary: 'skip diff parsing', breaking: false }],
    [{ type: 'chore', summary: 'sync release workflow' }, { type: 'chore', scope: null, summary: 'sync release workflow', breaking: false }],
    [{ type: 'ci', scope: 'actions', summary: 'tighten matrix' }, { type: 'ci', scope: 'actions', summary: 'tighten matrix', breaking: false }],
    [{ raw: 'feat: from raw', type: 'feat', summary: 'from raw' }, { type: 'feat', summary: 'from raw', breaking: false }],
    ['fix(api): keep auth stable', { type: 'fix', scope: 'api', summary: 'keep auth stable', breaking: false }],
    ['BREAKING CHANGE: swap schema', { type: 'other', summary: 'BREAKING CHANGE: swap schema', breaking: true }]
  ])('normalizes %o', (entry, expected) => {
    expect(normalizeCommitEntry(entry)).toMatchObject(expected);
  });
});

describe('determineVersionBump', () => {
  it.each([
    [['feat: add preview'], 'minor'],
    [['fix: resolve bug'], 'patch'],
    [['perf: speed up update'], 'patch'],
    [['security: rotate token'], 'patch'],
    [['docs: update README'], 'none'],
    [['chore: refresh workflow'], 'none'],
    [['fix!: change output format'], 'major'],
    [['feat(ui)!: replace renderer'], 'major'],
    [['feat: add preview', 'fix: resolve bug'], 'minor'],
    [['docs: update README', 'fix: resolve bug'], 'patch'],
    [['docs: update README', 'chore: cleanup'], 'none'],
    [['BREAKING CHANGE: swap schema'], 'major']
  ])('returns %s for %o', (entries, expected) => {
    expect(determineVersionBump(entries)).toBe(expected);
  });
});

describe('buildReleaseSummary', () => {
  it.each([
    [['feat(ui): add preview'], ['Features'], 'minor'],
    [['fix(cli): resolve bug'], ['Fixes'], 'patch'],
    [['perf(update): speed up triage'], ['Performance'], 'patch'],
    [['security: rotate token'], ['Security'], 'patch'],
    [['docs: update README'], ['Documentation'], 'none'],
    [['chore: sync workflows'], ['Maintenance'], 'none'],
    [['custom entry without prefix'], ['Other'], 'none'],
    [['feat(ui): add preview', 'fix(cli): resolve bug'], ['Features', 'Fixes'], 'minor'],
    [['feat(ui)!: replace renderer', 'docs: update README'], ['Features', 'Documentation'], 'major'],
    [['docs: update README', 'chore: sync workflows'], ['Documentation', 'Maintenance'], 'none'],
    [['sec(auth): reject empty token', 'fix(cli): resolve bug'], ['Fixes', 'Security'], 'patch'],
    [['refactor(parser): simplify reader', 'test(update): add coverage'], ['Maintenance'], 'none']
  ])('groups commits for %o', (entries, groupTitles, bump) => {
    const summary = buildReleaseSummary(entries);
    expect(summary.groups.map((group) => group.title)).toEqual(groupTitles);
    expect(summary.bump).toBe(bump);
  });

  it('preserves entry descriptions with scopes', () => {
    const summary = buildReleaseSummary(['feat(ui): add preview']);
    expect(summary.groups[0].entries[0].line).toBe('ui: add preview');
  });

  it('keeps unscoped entries readable', () => {
    const summary = buildReleaseSummary(['fix: resolve bug']);
    expect(summary.groups[0].entries[0].line).toBe('resolve bug');
  });

  it('includes total commit count', () => {
    const summary = buildReleaseSummary(['feat: add preview', 'fix: resolve bug']);
    expect(summary.totalCommits).toBe(2);
  });

  it('supports object-based input', () => {
    const summary = buildReleaseSummary([
      { type: 'feat', scope: 'ui', summary: 'add preview' },
      { type: 'fix', summary: 'resolve bug' }
    ]);
    expect(summary.bump).toBe('minor');
    expect(summary.groups).toHaveLength(2);
  });

  it('renders markdown output', () => {
    const summary = buildReleaseSummary(
      ['feat(ui): add preview', 'fix(cli): resolve bug'],
      { version: '1.2.0' }
    );
    const markdown = renderMarkdown(summary);
    expect(markdown).toContain('## Release Notes');
    expect(markdown).toContain('Version bump: minor');
    expect(markdown).toContain('### Features');
    expect(markdown).toContain('- ui: add preview');
    expect(markdown).toContain('Version: 1.2.0');
  });

  it('marks breaking entries in markdown', () => {
    const summary = buildReleaseSummary(['feat(ui)!: replace renderer']);
    const markdown = renderMarkdown(summary);
    expect(markdown).toContain('[breaking]');
  });
});

describe('cli', () => {
  it('prints help output', () => {
    const result = spawnSync('node', [cliPath, '--help'], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(0);
    expect(result.stdout).toContain('release-note-router');
  });

  it('fails without input', () => {
    const result = spawnSync('node', [cliPath], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('Missing required option');
  });

  it.each([
    [['feat: add preview'], 'json', 'minor'],
    [['fix: resolve bug'], 'json', 'patch'],
    [['docs: update README'], 'json', 'none'],
    [['feat(ui)!: replace renderer'], 'json', 'major'],
    [['feat: add preview', 'fix: resolve bug'], 'markdown', 'minor'],
    [['security: rotate token'], 'markdown', 'patch']
  ])('supports format %s for %o', (entries, format, expectedBump) => {
    const tempDir = makeTempDir('release-note-router-');
    const inputPath = path.join(tempDir, format === 'json' ? 'commits.json' : 'commits.txt');

    if (format === 'json') {
      writeJson(inputPath, entries);
    } else {
      fs.writeFileSync(inputPath, entries.join('\n'));
    }

    const result = spawnSync('node', [cliPath, '--input', inputPath, '--format', format], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });

    expect(result.status).toBe(0);
    if (format === 'json') {
      expect(JSON.parse(result.stdout).bump).toBe(expectedBump);
    } else {
      expect(result.stdout).toContain(`Version bump: ${expectedBump}`);
    }
  });

  it('supports the version flag in markdown output', () => {
    const tempDir = makeTempDir('release-note-router-');
    const inputPath = path.join(tempDir, 'commits.txt');
    fs.writeFileSync(inputPath, 'feat: add preview\n');

    const result = spawnSync(
      'node',
      [cliPath, '--input', inputPath, '--format', 'markdown', '--version', '2.0.0'],
      {
        cwd: path.resolve('.'),
        encoding: 'utf8'
      }
    );

    expect(result.status).toBe(0);
    expect(result.stdout).toContain('Version: 2.0.0');
  });

  it('fails for unsupported formats', () => {
    const tempDir = makeTempDir('release-note-router-');
    const inputPath = path.join(tempDir, 'commits.txt');
    fs.writeFileSync(inputPath, 'feat: add preview\n');

    const result = spawnSync('node', [cliPath, '--input', inputPath, '--format', 'xml'], {
      cwd: path.resolve('.'),
      encoding: 'utf8'
    });

    expect(result.status).toBe(1);
    expect(result.stderr).toContain('Unsupported format');
  });
});
