const GROUP_TITLES = {
  features: 'Features',
  fixes: 'Fixes',
  performance: 'Performance',
  security: 'Security',
  documentation: 'Documentation',
  maintenance: 'Maintenance',
  other: 'Other'
};

const GROUP_ORDER = [
  'features',
  'fixes',
  'performance',
  'security',
  'documentation',
  'maintenance',
  'other'
];

const TYPE_TO_GROUP = {
  feat: 'features',
  fix: 'fixes',
  perf: 'performance',
  sec: 'security',
  security: 'security',
  docs: 'documentation',
  chore: 'maintenance',
  refactor: 'maintenance',
  build: 'maintenance',
  ci: 'maintenance',
  test: 'maintenance'
};

export function parseCommitLine(line) {
  const trimmed = String(line).trim();
  if (!trimmed) {
    return {
      raw: '',
      type: 'other',
      scope: null,
      summary: '',
      breaking: false
    };
  }

  const match = trimmed.match(/^(?<type>[a-z]+)(?:\((?<scope>[^)]+)\))?(?<breaking>!)?:\s*(?<summary>.+)$/i);
  if (!match?.groups) {
    return {
      raw: trimmed,
      type: 'other',
      scope: null,
      summary: trimmed,
      breaking: trimmed.includes('BREAKING CHANGE')
    };
  }

  return {
    raw: trimmed,
    type: match.groups.type.toLowerCase(),
    scope: match.groups.scope ?? null,
    summary: match.groups.summary.trim(),
    breaking: Boolean(match.groups.breaking)
  };
}

export function normalizeCommitEntry(entry) {
  if (typeof entry === 'string') {
    return parseCommitLine(entry);
  }

  const lineLike = [
    entry.type,
    entry.scope ? `(${entry.scope})` : '',
    entry.breaking ? '!' : '',
    ': ',
    entry.summary ?? ''
  ].join('');
  const parsed = parseCommitLine(lineLike);
  const body = String(entry.body ?? entry.notes ?? '');

  return {
    ...parsed,
    raw: entry.raw ?? parsed.raw,
    breaking: parsed.breaking || body.includes('BREAKING CHANGE'),
    summary: entry.summary ?? parsed.summary
  };
}

function describeEntry(entry) {
  return entry.scope ? `${entry.scope}: ${entry.summary}` : entry.summary;
}

function classifyGroup(type) {
  return TYPE_TO_GROUP[type] ?? 'other';
}

export function determineVersionBump(entries) {
  const normalized = entries.map(normalizeCommitEntry);

  if (normalized.some((entry) => entry.breaking)) {
    return 'major';
  }

  if (normalized.some((entry) => classifyGroup(entry.type) === 'features')) {
    return 'minor';
  }

  if (
    normalized.some((entry) =>
      ['fixes', 'performance', 'security'].includes(classifyGroup(entry.type))
    )
  ) {
    return 'patch';
  }

  return 'none';
}

export function buildReleaseSummary(entries, options = {}) {
  const normalized = entries
    .map(normalizeCommitEntry)
    .filter((entry) => entry.summary);
  const groups = new Map(GROUP_ORDER.map((group) => [group, []]));

  for (const entry of normalized) {
    const group = classifyGroup(entry.type);
    groups.get(group).push({
      ...entry,
      line: describeEntry(entry)
    });
  }

  const renderedGroups = GROUP_ORDER
    .map((group) => ({
      key: group,
      title: GROUP_TITLES[group],
      entries: groups.get(group)
    }))
    .filter((group) => group.entries.length > 0);

  return {
    version: options.version ?? null,
    bump: determineVersionBump(normalized),
    totalCommits: normalized.length,
    groups: renderedGroups
  };
}

export function renderMarkdown(summary) {
  const lines = ['## Release Notes', `Version bump: ${summary.bump}`];

  if (summary.version) {
    lines.push(`Version: ${summary.version}`);
  }

  for (const group of summary.groups) {
    lines.push('');
    lines.push(`### ${group.title}`);
    for (const entry of group.entries) {
      lines.push(`- ${entry.line}${entry.breaking ? ' [breaking]' : ''}`);
    }
  }

  return `${lines.join('\n')}\n`;
}
