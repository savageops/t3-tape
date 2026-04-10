import fs from 'node:fs';
import path from 'node:path';

import { uniqueValues } from '../shared/collections.js';

function readJsonFile(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function readTextFile(filePath) {
  return fs.readFileSync(filePath, 'utf8');
}

function quoteArg(value) {
  const rendered = String(value);
  if (!/[\s"]/u.test(rendered)) {
    return rendered;
  }

  return `"${rendered.replace(/"/g, '\\"')}"`;
}

function pushOption(parts, flag, value) {
  if (value === undefined || value === null || value === false || value === '') {
    return;
  }

  parts.push(flag);
  if (value !== true) {
    parts.push(quoteArg(value));
  }
}

function escapeRegExp(value) {
  return value.replace(/[|\\{}()[\]^$+*?.]/g, '\\$&');
}

function normalizeParagraph(value) {
  return value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .join(' ')
    .trim();
}

function extractField(block, name) {
  const match = block.match(new RegExp(String.raw`\*\*${escapeRegExp(name)}:\*\*\s*(.+)$`, 'm'));
  return match ? match[1].trim() : null;
}

function extractSection(block, name) {
  const match = block.match(
    new RegExp(`^### ${escapeRegExp(name)}\\r?\\n([\\s\\S]*?)(?=^### |^---$|\\Z)`, 'm')
  );
  return match ? match[1].trim() : '';
}

function normalizePatchEntry(id, title, block) {
  const behaviorContract = extractSection(block, 'Behavior Contract');

  return {
    id,
    title: title.trim(),
    status: extractField(block, 'status') ?? 'unknown',
    surface: extractField(block, 'surface'),
    author: extractField(block, 'author'),
    added: extractField(block, 'added'),
    intent: normalizeParagraph(extractSection(block, 'Intent')),
    behaviorAssertions: behaviorContract
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line.startsWith('- '))
      .map((line) => line.slice(2).trim())
  };
}

export function parsePatchRegistry(markdown) {
  const headers = [];
  const headerPattern = /^## \[(PATCH-[A-Z0-9-]+)\] (.+)$/gm;
  let match;

  while ((match = headerPattern.exec(markdown)) !== null) {
    headers.push({
      id: match[1],
      title: match[2],
      index: match.index
    });
  }

  return headers.map((header, index) => {
    const blockEnd = headers[index + 1]?.index ?? markdown.length;
    const block = markdown.slice(header.index, blockEnd);
    return normalizePatchEntry(header.id, header.title, block);
  });
}

export function resolveProviderKind(agent = {}) {
  if (agent.provider) {
    return agent.provider;
  }

  if (!agent.endpoint) {
    return 'none';
  }

  if (/^https?:\/\//u.test(agent.endpoint)) {
    return 'http';
  }

  return 'exec';
}

function normalizeConfig(rawConfig) {
  const agent = rawConfig.agent ?? {};
  const sandbox = rawConfig.sandbox ?? {};
  const hooks = rawConfig.hooks ?? {};

  return {
    protocol: rawConfig.protocol ?? '0.1.0',
    upstream: rawConfig.upstream ?? '',
    agent: {
      provider: resolveProviderKind(agent),
      endpoint: agent.endpoint ?? '',
      confidenceThreshold: agent['confidence-threshold'] ?? 0.8,
      maxAttempts: agent['max-attempts'] ?? 3
    },
    sandbox: {
      previewCommand: sandbox['preview-command'] ?? ''
    },
    hooks: {
      prePatch: hooks['pre-patch'] ?? '',
      postPatch: hooks['post-patch'] ?? '',
      preUpdate: hooks['pre-update'] ?? '',
      postUpdate: hooks['post-update'] ?? '',
      onConflict: hooks['on-conflict'] ?? ''
    }
  };
}

function normalizePreview(preview) {
  if (!preview) {
    return null;
  }

  return {
    command: preview.command,
    exitCode: preview['exit-code'],
    stdoutPath: preview['stdout-path'],
    stderrPath: preview['stderr-path']
  };
}

function normalizeTriagePatch(rawPatch) {
  return {
    id: rawPatch.id,
    title: rawPatch.title,
    detectedStatus: rawPatch['detected-status'],
    triageStatus: rawPatch['triage-status'],
    mergedUpstreamCandidate: Boolean(rawPatch['merged-upstream-candidate']),
    applyStderr: rawPatch['apply-stderr'] ?? '',
    confidence: rawPatch.confidence ?? null,
    agentMode: rawPatch['agent-mode'] ?? null,
    notes: rawPatch.notes ?? null,
    unresolved: rawPatch.unresolved ?? [],
    resolvedDiffPath: rawPatch['resolved-diff-path'] ?? null,
    notesPath: rawPatch['notes-path'] ?? null,
    rawResponsePath: rawPatch['raw-response-path'] ?? null,
    applyCommit: rawPatch['apply-commit'] ?? null,
    approved: Boolean(rawPatch.approved),
    scopeUpdate: rawPatch['scope-update'] ?? null
  };
}

function normalizeTriage(rawTriage) {
  return {
    schemaVersion: rawTriage['schema-version'],
    fromRef: rawTriage['from-ref'],
    toRef: rawTriage['to-ref'],
    toRefResolved: rawTriage['to-ref-resolved'],
    upstream: rawTriage.upstream,
    timestamp: rawTriage.timestamp,
    sandbox: {
      path: rawTriage.sandbox.path,
      worktreeBranch: rawTriage.sandbox['worktree-branch'],
      worktreePath: rawTriage.sandbox['worktree-path']
    },
    patches: (rawTriage.patches ?? []).map(normalizeTriagePatch),
    preview: normalizePreview(rawTriage.preview)
  };
}

export function resolveStateDir(inputPath) {
  const absolute = path.resolve(inputPath);
  return path.basename(absolute) === '.t3' ? absolute : path.join(absolute, '.t3');
}

export function readStateSurface(inputPath) {
  const stateDir = resolveStateDir(inputPath);
  const pluginRoot = path.join(stateDir, 'patch');
  const configPath = path.join(pluginRoot, 'config.json');
  const triagePath = path.join(pluginRoot, 'triage.json');
  const patchMdPath = path.join(stateDir, 'patch.md');
  const migrationLogPath = path.join(pluginRoot, 'migration.log');
  const patchMd = readTextFile(patchMdPath);
  const patchRegistry = parsePatchRegistry(patchMd);
  const patchIndex = new Map(patchRegistry.map((entry) => [entry.id, entry]));

  return {
    stateDir,
    pluginRoot,
    paths: {
      pluginRoot,
      config: configPath,
      triage: triagePath,
      patchMd: patchMdPath,
      migrationLog: migrationLogPath
    },
    config: normalizeConfig(readJsonFile(configPath)),
    triage: normalizeTriage(readJsonFile(triagePath)),
    patchMd,
    migrationLog: readTextFile(migrationLogPath),
    patchRegistry,
    patchIndex
  };
}

function buildCommand(parts, options = {}) {
  const command = ['t3-tape'];
  pushOption(command, '--repo-root', options.repoRoot);
  pushOption(command, '--state-dir', options.stateDir);
  command.push(...parts);
  return command.join(' ');
}

export function buildUpdateCommand(options) {
  const parts = ['update'];
  pushOption(parts, '--ref', options.ref);
  pushOption(parts, '--ci', options.ci === true);
  pushOption(parts, '--confidence-threshold', options.confidenceThreshold);
  return buildCommand(parts, options);
}

export function buildTriageCommand(options = {}) {
  const parts = ['triage'];
  pushOption(parts, '--json', options.json === true);
  return buildCommand(parts, options);
}

export function buildApproveCommand(options) {
  return buildCommand(['triage', 'approve', options.patchId], options);
}

export function buildRederiveCommand(options) {
  return buildCommand(['rederive', options.patchId], options);
}

export function summarizeTriageCounts(triage) {
  const counts = {
    clean: 0,
    conflict: 0,
    missingSurface: 0,
    pendingReview: 0,
    needsYou: 0
  };

  for (const patch of triage.patches) {
    if (patch.triageStatus === 'CLEAN') {
      counts.clean += 1;
    } else if (patch.triageStatus === 'CONFLICT') {
      counts.conflict += 1;
    } else if (patch.triageStatus === 'MISSING-SURFACE') {
      counts.missingSurface += 1;
    } else if (patch.triageStatus === 'pending-review') {
      counts.pendingReview += 1;
    } else if (patch.triageStatus === 'NEEDS-YOU') {
      counts.needsYou += 1;
    }
  }

  return counts;
}

export function buildCommonCommands(surface) {
  return uniqueValues([
    buildTriageCommand({ stateDir: surface.stateDir, json: true }),
    surface.triage.toRef
      ? buildUpdateCommand({
          stateDir: surface.stateDir,
          ref: surface.triage.toRef,
          ci: true,
          confidenceThreshold: surface.config.agent.confidenceThreshold
        })
      : null
  ]);
}
