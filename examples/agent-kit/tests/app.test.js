import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import {
  buildApproveCommand,
  buildCommonCommands,
  buildRederiveCommand,
  buildTriageCommand,
  buildUpdateCommand,
  parsePatchRegistry,
  readStateSurface,
  resolveProviderKind,
  resolveStateDir,
  summarizeTriageCounts
} from '../index.js';
import { copyDir, makeTempDir, writeJson, writeText } from '../../shared/test-helpers.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '..', '..', '..');
const agentDemoStateDir = path.join(repoRoot, 'examples', 'fixtures', 'agent-demo', '.t3');
const agentDemoRoot = path.dirname(agentDemoStateDir);
const patchMarkdown = fs.readFileSync(path.join(agentDemoStateDir, 'patch.md'), 'utf8');
const triageJson = JSON.parse(
  fs.readFileSync(path.join(agentDemoStateDir, 'patch', 'triage.json'), 'utf8')
);

describe('resolveProviderKind', () => {
  const cases = [
    ['uses explicit exec provider', { provider: 'exec' }, 'exec'],
    ['uses explicit http provider', { provider: 'http' }, 'http'],
    ['uses explicit none provider', { provider: 'none' }, 'none'],
    ['infers http from https endpoint', { endpoint: 'https://agents.example/api' }, 'http'],
    ['infers http from http endpoint', { endpoint: 'http://localhost:4000/resolve' }, 'http'],
    ['infers exec from local command path', { endpoint: './scripts/agent-runner.mjs' }, 'exec'],
    ['infers exec from bare executable name', { endpoint: 'node scripts/runner.js' }, 'exec'],
    ['returns none when endpoint is missing', {}, 'none'],
    ['returns none when agent is undefined', undefined, 'none']
  ];

  for (const [label, value, expected] of cases) {
    it(label, () => {
      expect(resolveProviderKind(value)).toBe(expected);
    });
  }
});

describe('resolveStateDir', () => {
  const cases = [
    ['keeps an explicit .t3 path', agentDemoStateDir, agentDemoStateDir],
    ['adds .t3 for a repo root path', agentDemoRoot, agentDemoStateDir],
    ['resolves dot segments before appending .t3', path.join(agentDemoRoot, '.'), agentDemoStateDir],
    ['normalizes nested relative paths', path.join(agentDemoRoot, '..', 'agent-demo'), agentDemoStateDir],
    [
      'appends .t3 to a relative repo path',
      path.relative(process.cwd(), agentDemoRoot),
      agentDemoStateDir
    ],
    [
      'preserves trailing .t3 on a relative path',
      path.relative(process.cwd(), agentDemoStateDir),
      agentDemoStateDir
    ]
  ];

  for (const [label, inputPath, expected] of cases) {
    it(label, () => {
      expect(resolveStateDir(inputPath)).toBe(path.resolve(expected));
    });
  }
});

describe('parsePatchRegistry', () => {
  const registry = parsePatchRegistry(patchMarkdown);

  it('parses every patch header from the registry fixture', () => {
    expect(registry).toHaveLength(3);
  });

  const ids = ['PATCH-001', 'PATCH-002', 'PATCH-003'];
  for (const patchId of ids) {
    it(`includes ${patchId}`, () => {
      expect(registry.some((entry) => entry.id === patchId)).toBe(true);
    });
  }

  const expectedTitles = [
    ['PATCH-001', 'plugin-settings-toolbar-button'],
    ['PATCH-002', 'command-palette-plugin-bridge'],
    ['PATCH-003', 'schema-change-cache-reset']
  ];

  for (const [patchId, title] of expectedTitles) {
    it(`keeps the title for ${patchId}`, () => {
      expect(registry.find((entry) => entry.id === patchId)?.title).toBe(title);
    });
  }

  const expectedStatuses = [
    ['PATCH-001', 'active'],
    ['PATCH-002', 'active'],
    ['PATCH-003', 'active']
  ];

  for (const [patchId, status] of expectedStatuses) {
    it(`keeps the status for ${patchId}`, () => {
      expect(registry.find((entry) => entry.id === patchId)?.status).toBe(status);
    });
  }

  const expectedSurfaces = [
    ['PATCH-001', 'src/toolbar.tsx'],
    ['PATCH-002', 'src/editor/commands.ts'],
    ['PATCH-003', 'scripts/ci-cache-reset.mjs']
  ];

  for (const [patchId, surface] of expectedSurfaces) {
    it(`keeps the surface for ${patchId}`, () => {
      expect(registry.find((entry) => entry.id === patchId)?.surface).toBe(surface);
    });
  }

  it('normalizes multi-line intent text into a single paragraph', () => {
    expect(registry[0].intent).not.toMatch(/\n/u);
    expect(registry[0].intent).toContain('settings button');
  });

  const assertionExpectations = [
    ['PATCH-001', 2],
    ['PATCH-002', 2],
    ['PATCH-003', 2]
  ];

  for (const [patchId, count] of assertionExpectations) {
    it(`captures the behavior assertion count for ${patchId}`, () => {
      expect(registry.find((entry) => entry.id === patchId)?.behaviorAssertions).toHaveLength(count);
    });
  }

  it('returns an empty array when a markdown file contains no patch entries', () => {
    expect(parsePatchRegistry('# PatchMD\n')).toEqual([]);
  });

  it('falls back to unknown status when no status field is present', () => {
    const result = parsePatchRegistry(
      '# PatchMD\n\n## [PATCH-010] sample\n\n### Intent\n\nTest.\n\n### Behavior Contract\n\n- works\n'
    );
    expect(result[0].status).toBe('unknown');
  });

  it('handles blank intent paragraphs without throwing', () => {
    const result = parsePatchRegistry(
      '# PatchMD\n\n## [PATCH-011] sample\n\n**status:** active\n\n### Intent\n\n\n### Behavior Contract\n\n- works\n'
    );
    expect(result[0].intent).toBe('');
  });
});

describe('readStateSurface', () => {
  const surface = readStateSurface(agentDemoStateDir);

  it('resolves the canonical state directory', () => {
    expect(surface.stateDir).toBe(agentDemoStateDir);
  });

  const pathChecks = [
    ['config', path.join('patch', 'config.json')],
    ['triage', path.join('patch', 'triage.json')],
    ['patchMd', 'patch.md'],
    ['migrationLog', path.join('patch', 'migration.log')]
  ];

  for (const [key, suffix] of pathChecks) {
    it(`exposes the ${key} path`, () => {
      expect(surface.paths[key]).toBe(path.join(agentDemoStateDir, suffix));
    });
  }

  it('normalizes the config protocol', () => {
    expect(surface.config.protocol).toBe('0.1.0');
  });

  it('normalizes the agent provider kind', () => {
    expect(surface.config.agent.provider).toBe('exec');
  });

  it('normalizes the confidence threshold', () => {
    expect(surface.config.agent.confidenceThreshold).toBe(0.8);
  });

  it('normalizes the sandbox preview command', () => {
    expect(surface.config.sandbox.previewCommand).toBe('pnpm test');
  });

  it('indexes the patch registry by id', () => {
    expect(surface.patchIndex.get('PATCH-002')?.title).toBe('command-palette-plugin-bridge');
  });

  it('normalizes triage patch count', () => {
    expect(surface.triage.patches).toHaveLength(3);
  });

  const triageChecks = [
    ['PATCH-001', 'pending-review'],
    ['PATCH-002', 'NEEDS-YOU'],
    ['PATCH-003', 'CLEAN']
  ];

  for (const [patchId, triageStatus] of triageChecks) {
    it(`normalizes triage status for ${patchId}`, () => {
      expect(surface.triage.patches.find((entry) => entry.id === patchId)?.triageStatus).toBe(
        triageStatus
      );
    });
  }

  it('normalizes the preview exit code', () => {
    expect(surface.triage.preview.exitCode).toBe(0);
  });

  it('reads the migration log text', () => {
    expect(surface.migrationLog).toContain('UPDATE CYCLE');
  });

  it('supports reading state from the repo root instead of the .t3 path', () => {
    const rootSurface = readStateSurface(agentDemoRoot);
    expect(rootSurface.stateDir).toBe(agentDemoStateDir);
  });

  it('reflects temp fixture edits when files change', () => {
    const tempRoot = makeTempDir('agent-kit-state-');
    const tempFixture = path.join(tempRoot, 'fixture');
    copyDir(agentDemoRoot, tempFixture);
    const tempStateDir = path.join(tempFixture, '.t3');
    const editedConfig = JSON.parse(
      fs.readFileSync(path.join(tempStateDir, 'patch', 'config.json'), 'utf8')
    );
    editedConfig.agent.endpoint = './scripts/rederive.mjs';
    writeJson(path.join(tempStateDir, 'patch', 'config.json'), editedConfig);

    const surfaceWithEdit = readStateSurface(tempFixture);
    expect(surfaceWithEdit.config.agent.provider).toBe('exec');
  });
});

describe('buildUpdateCommand', () => {
  const cases = [
    ['builds a minimal update command', { ref: 'v1.2.3' }, 't3-tape update --ref v1.2.3'],
    [
      'adds the state dir override',
      { ref: 'v1.2.3', stateDir: 'C:/repo/.t3' },
      't3-tape --state-dir C:/repo/.t3 update --ref v1.2.3'
    ],
    [
      'adds the repo root override',
      { ref: 'v1.2.3', repoRoot: 'C:/repo' },
      't3-tape --repo-root C:/repo update --ref v1.2.3'
    ],
    [
      'adds the ci flag',
      { ref: 'v1.2.3', ci: true },
      't3-tape update --ref v1.2.3 --ci'
    ],
    [
      'adds the confidence threshold',
      { ref: 'v1.2.3', confidenceThreshold: 0.9 },
      't3-tape update --ref v1.2.3 --confidence-threshold 0.9'
    ],
    [
      'quotes repo roots with spaces',
      { ref: 'release/2026.04', repoRoot: 'C:/Workspaces/T3 Tape Repo' },
      't3-tape --repo-root "C:/Workspaces/T3 Tape Repo" update --ref release/2026.04'
    ],
    [
      'quotes refs with spaces',
      { ref: 'release candidate', stateDir: 'C:/repo/.t3' },
      't3-tape --state-dir C:/repo/.t3 update --ref "release candidate"'
    ],
    [
      'omits falsey ci values',
      { ref: 'v1.2.3', ci: false },
      't3-tape update --ref v1.2.3'
    ]
  ];

  for (const [label, options, expected] of cases) {
    it(label, () => {
      expect(buildUpdateCommand(options)).toBe(expected);
    });
  }
});

describe('triage, approve, and rederive command builders', () => {
  const triageCases = [
    ['builds a plain triage command', {}, 't3-tape triage'],
    ['adds json output to triage', { json: true }, 't3-tape triage --json'],
    [
      'adds state dir to triage',
      { stateDir: 'C:/repo/.t3', json: true },
      't3-tape --state-dir C:/repo/.t3 triage --json'
    ],
    [
      'adds repo root to triage',
      { repoRoot: 'C:/repo', json: true },
      't3-tape --repo-root C:/repo triage --json'
    ]
  ];

  for (const [label, options, expected] of triageCases) {
    it(label, () => {
      expect(buildTriageCommand(options)).toBe(expected);
    });
  }

  const approveCases = [
    [
      'builds approve for a patch id',
      { patchId: 'PATCH-001' },
      't3-tape triage approve PATCH-001'
    ],
    [
      'adds state dir to approve',
      { patchId: 'PATCH-002', stateDir: 'C:/repo/.t3' },
      't3-tape --state-dir C:/repo/.t3 triage approve PATCH-002'
    ],
    [
      'adds repo root to approve',
      { patchId: 'PATCH-003', repoRoot: 'C:/repo' },
      't3-tape --repo-root C:/repo triage approve PATCH-003'
    ],
    [
      'passes patch ids through directly',
      { patchId: 'PATCH custom', stateDir: 'C:/repo/.t3' },
      't3-tape --state-dir C:/repo/.t3 triage approve PATCH custom'
    ]
  ];

  for (const [label, options, expected] of approveCases) {
    it(label, () => {
      expect(buildApproveCommand(options)).toBe(expected);
    });
  }

  const rederiveCases = [
    ['builds rederive for a patch id', { patchId: 'PATCH-001' }, 't3-tape rederive PATCH-001'],
    [
      'adds state dir to rederive',
      { patchId: 'PATCH-002', stateDir: 'C:/repo/.t3' },
      't3-tape --state-dir C:/repo/.t3 rederive PATCH-002'
    ],
    [
      'adds repo root to rederive',
      { patchId: 'PATCH-003', repoRoot: 'C:/repo' },
      't3-tape --repo-root C:/repo rederive PATCH-003'
    ],
    [
      'passes patch ids through directly for rederive',
      { patchId: 'PATCH custom', repoRoot: 'C:/repo root' },
      't3-tape --repo-root "C:/repo root" rederive PATCH custom'
    ]
  ];

  for (const [label, options, expected] of rederiveCases) {
    it(label, () => {
      expect(buildRederiveCommand(options)).toBe(expected);
    });
  }
});

describe('summarizeTriageCounts', () => {
  const counts = summarizeTriageCounts(readStateSurface(agentDemoStateDir).triage);

  const expectedCounts = [
    ['clean', 1],
    ['conflict', 0],
    ['missingSurface', 0],
    ['pendingReview', 1],
    ['needsYou', 1]
  ];

  for (const [key, expected] of expectedCounts) {
    it(`counts ${key}`, () => {
      expect(counts[key]).toBe(expected);
    });
  }

  it('returns zero counts for an empty triage list', () => {
    expect(
      summarizeTriageCounts({
        patches: []
      })
    ).toEqual({
      clean: 0,
      conflict: 0,
      missingSurface: 0,
      pendingReview: 0,
      needsYou: 0
    });
  });
});

describe('buildCommonCommands', () => {
  const surface = readStateSurface(agentDemoStateDir);
  const commands = buildCommonCommands(surface);

  it('includes the triage json command', () => {
    expect(commands[0]).toBe(`t3-tape --state-dir ${surface.stateDir} triage --json`);
  });

  it('includes the update command for the current target ref', () => {
    expect(commands[1]).toContain(`update --ref ${surface.triage.toRef}`);
  });

  it('uses the configured confidence threshold', () => {
    expect(commands[1]).toContain('--confidence-threshold 0.8');
  });

  it('returns unique commands only once', () => {
    expect(new Set(commands).size).toBe(commands.length);
  });

  it('returns two commands when a target ref exists', () => {
    expect(commands).toHaveLength(2);
  });

  it('omits the update command when the triage target ref is blank', () => {
    const tempRoot = makeTempDir('agent-kit-common-');
    const tempFixture = path.join(tempRoot, 'fixture');
    copyDir(agentDemoRoot, tempFixture);
    const tempStateDir = path.join(tempFixture, '.t3');
    const triage = JSON.parse(
      fs.readFileSync(path.join(tempStateDir, 'patch', 'triage.json'), 'utf8')
    );
    triage['to-ref'] = '';
    writeJson(path.join(tempStateDir, 'patch', 'triage.json'), triage);

    const tempCommands = buildCommonCommands(readStateSurface(tempFixture));
    expect(tempCommands).toEqual([`t3-tape --state-dir ${tempStateDir} triage --json`]);
  });
});

describe('readStateSurface failure behavior', () => {
  it('throws when the patch registry file is missing', () => {
    const tempRoot = makeTempDir('agent-kit-missing-');
    const tempFixture = path.join(tempRoot, 'fixture');
    copyDir(agentDemoRoot, tempFixture);
    fs.rmSync(path.join(tempFixture, '.t3', 'patch.md'));

    expect(() => readStateSurface(tempFixture)).toThrow();
  });

  it('throws when triage json is invalid', () => {
    const tempRoot = makeTempDir('agent-kit-invalid-');
    const tempFixture = path.join(tempRoot, 'fixture');
    copyDir(agentDemoRoot, tempFixture);
    writeText(path.join(tempFixture, '.t3', 'patch', 'triage.json'), '{invalid');

    expect(() => readStateSurface(tempFixture)).toThrow();
  });
});
