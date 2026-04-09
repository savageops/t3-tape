import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { parsePatchRegistry } from '../../agent-kit/index.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const exampleRoot = path.resolve(__dirname, '..');
const stateDir = path.join(exampleRoot, '.t3');
const configPath = path.join(stateDir, 'config.json');
const patchMdPath = path.join(stateDir, 'patch.md');
const migrationLogPath = path.join(stateDir, 'migration.log');
const diffPath = path.join(stateDir, 'patches', 'PATCH-001.diff');
const metaPath = path.join(stateDir, 'patches', 'PATCH-001.meta.json');
const reportPath = path.join(stateDir, 'reports', 'example-summary.md');
const appPath = path.join(exampleRoot, 'src', 'app.txt');

const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
const patchMd = fs.readFileSync(patchMdPath, 'utf8');
const registry = parsePatchRegistry(patchMd);
const patch = registry[0];
const meta = JSON.parse(fs.readFileSync(metaPath, 'utf8'));
const diff = fs.readFileSync(diffPath, 'utf8');
const migrationLog = fs.readFileSync(migrationLogPath, 'utf8');
const report = fs.readFileSync(reportPath, 'utf8');
const appText = fs.readFileSync(appPath, 'utf8');

describe('basic-fork filesystem contract', () => {
  const existingPaths = [
    ['state directory exists', stateDir],
    ['config exists', configPath],
    ['patch registry exists', patchMdPath],
    ['migration log exists', migrationLogPath],
    ['patches directory exists', path.join(stateDir, 'patches')],
    ['patch diff exists', diffPath],
    ['patch meta exists', metaPath],
    ['reports directory exists', path.join(stateDir, 'reports')],
    ['report file exists', reportPath],
    ['source fixture exists', appPath]
  ];

  for (const [label, targetPath] of existingPaths) {
    it(label, () => {
      expect(fs.existsSync(targetPath)).toBe(true);
    });
  }
});

describe('basic-fork config contract', () => {
  const topLevelChecks = [
    ['protocol', config.protocol, '0.1.0'],
    ['upstream', config.upstream, 'https://github.com/example/upstream-app']
  ];

  for (const [label, actual, expected] of topLevelChecks) {
    it(`keeps ${label}`, () => {
      expect(actual).toEqual(expected);
    });
  }

  const agentChecks = [
    ['provider', config.agent.provider, ''],
    ['endpoint', config.agent.endpoint, ''],
    ['confidence-threshold', config.agent['confidence-threshold'], 0.8],
    ['max-attempts', config.agent['max-attempts'], 3]
  ];

  for (const [label, actual, expected] of agentChecks) {
    it(`keeps agent ${label}`, () => {
      expect(actual).toEqual(expected);
    });
  }

  const sandboxChecks = [
    ['preview-command', config.sandbox['preview-command'], '']
  ];

  for (const [label, actual, expected] of sandboxChecks) {
    it(`keeps sandbox ${label}`, () => {
      expect(actual).toEqual(expected);
    });
  }

  const hookChecks = [
    ['pre-patch', config.hooks['pre-patch'], ''],
    ['post-patch', config.hooks['post-patch'], ''],
    ['pre-update', config.hooks['pre-update'], ''],
    ['post-update', config.hooks['post-update'], ''],
    ['on-conflict', config.hooks['on-conflict'], '']
  ];

  for (const [label, actual, expected] of hookChecks) {
    it(`keeps hook ${label}`, () => {
      expect(actual).toEqual(expected);
    });
  }
});

describe('basic-fork patch registry contract', () => {
  it('contains one patch entry', () => {
    expect(registry).toHaveLength(1);
  });

  const patchChecks = [
    ['id', patch.id, 'PATCH-001'],
    ['title', patch.title, 'toolbar-settings-button'],
    ['status', patch.status, 'active'],
    ['surface', patch.surface, 'src/app.txt'],
    ['author', patch.author, 'example-user'],
    ['added', patch.added, '2026-04-09']
  ];

  for (const [label, actual, expected] of patchChecks) {
    it(`keeps patch ${label}`, () => {
      expect(actual).toBe(expected);
    });
  }

  const headerLines = [
    '# PatchMD',
    '> project: upstream-app',
    '> upstream: https://github.com/example/upstream-app',
    '> base-ref: abc1234',
    '> protocol: 0.1.0'
  ];

  for (const line of headerLines) {
    it(`includes header line: ${line}`, () => {
      expect(patchMd).toContain(line);
    });
  }

  it('keeps the intent text readable', () => {
    expect(patch.intent).toContain('toolbar affordance visible');
  });

  const behaviorAssertions = [
    'the customized app surface still renders the patched line',
    'the change remains attributable to a named PatchMD record'
  ];

  for (const assertion of behaviorAssertions) {
    it(`captures behavior assertion: ${assertion}`, () => {
      expect(patch.behaviorAssertions).toContain(assertion);
    });
  }

  const scopeLines = [
    '- **files:** ["src/app.txt"]',
    '- **components:** ["AppShell"]',
    '- **entry-points:** ["src/app.txt"]',
    '- **requires:** []',
    '- **conflicts-with:** []'
  ];

  for (const line of scopeLines) {
    it(`includes scope or dependency line: ${line}`, () => {
      expect(patchMd).toContain(line);
    });
  }

  it('keeps the compact fixture note', () => {
    expect(patchMd).toContain('This fixture is illustrative and intentionally compact.');
  });
});

describe('basic-fork meta contract', () => {
  const metaChecks = [
    ['id', meta.id, 'PATCH-001'],
    ['title', meta.title, 'toolbar-settings-button'],
    ['status', meta.status, 'active'],
    ['base-ref', meta['base-ref'], 'abc1234'],
    ['current-ref', meta['current-ref'], 'abc1234'],
    ['diff-file', meta['diff-file'], 'patches/PATCH-001.diff'],
    ['apply-confidence', meta['apply-confidence'], 1],
    ['last-applied', meta['last-applied'], '2026-04-09T10:00:00Z'],
    ['last-checked', meta['last-checked'], '2026-04-09T10:00:00Z'],
    ['agent-attempts', meta['agent-attempts'], 0],
    ['surface-hash', meta['surface-hash'], 'example-surface-hash']
  ];

  for (const [label, actual, expected] of metaChecks) {
    it(`keeps meta ${label}`, () => {
      expect(actual).toEqual(expected);
    });
  }

  it('stores two behavior assertions in metadata', () => {
    expect(meta['behavior-assertions']).toHaveLength(2);
  });

  for (const assertion of meta['behavior-assertions']) {
    it(`matches meta assertion in patch registry: ${assertion}`, () => {
      expect(patch.behaviorAssertions).toContain(assertion);
    });
  }
});

describe('basic-fork diff and source parity', () => {
  const diffLines = [
    'diff --git a/src/app.txt b/src/app.txt',
    '--- a/src/app.txt',
    '+++ b/src/app.txt',
    '@@ -1,2 +1,2 @@',
    ' alpha',
    '-base',
    '+patched'
  ];

  for (const line of diffLines) {
    it(`includes diff line: ${line}`, () => {
      expect(diff).toContain(line);
    });
  }

  const appLines = ['alpha', 'patched'];
  for (const line of appLines) {
    it(`includes app line: ${line}`, () => {
      expect(appText).toContain(line);
    });
  }

  it('does not keep the pre-patch line in the source fixture', () => {
    expect(appText).not.toContain('base');
  });

  it('keeps the patched line in both the diff and source fixture', () => {
    expect(diff).toContain('+patched');
    expect(appText).toContain('patched');
  });
});

describe('basic-fork migration log contract', () => {
  const logLines = [
    '[2026-04-09T10:00:00Z] UPDATE CYCLE',
    'from-ref: abc1234',
    'to-ref:   def5678',
    'patches:  1 active',
    'clean:    1',
    'resolved: 0',
    'rederived: 0',
    'failed:   0',
    'sandbox:  .t3/sandbox/20260409-100000Z/',
    'approved: 2026-04-09T10:05:00Z by example-user',
    'status:   COMPLETE',
    '---'
  ];

  for (const line of logLines) {
    it(`includes migration log line: ${line}`, () => {
      expect(migrationLog).toContain(line);
    });
  }
});

describe('basic-fork foreign report tolerance', () => {
  it('keeps the foreign report readable', () => {
    expect(report).toContain('intentionally foreign to PatchMD ownership');
  });

  it('documents the tolerated report path', () => {
    expect(report).toContain('`.t3/reports/example-summary.md`');
  });

  it('keeps the README explanation aligned with the fixture intent', () => {
    const readme = fs.readFileSync(path.join(exampleRoot, 'README.md'), 'utf8');
    expect(readme).toContain('committed example of the canonical PatchMD store shape');
  });

  it('keeps the README mention of tolerated foreign reports', () => {
    const readme = fs.readFileSync(path.join(exampleRoot, 'README.md'), 'utf8');
    expect(readme).toContain('one tolerated foreign report artifact under `.t3/reports/`');
  });
});
