import { uniqueValues } from '../../shared/collections.js';
import { createWorkflow, createWorkflowStage } from '../../shared/workflow.js';

const STATUS_RANK = {
  ready: 0,
  warning: 1,
  blocked: 2
};

function cleanVersion(value) {
  if (!value) {
    return [];
  }

  return String(value)
    .trim()
    .replace(/^v/i, '')
    .split('-')[0]
    .split('.')
    .map((segment) => {
      const parsed = Number.parseInt(segment, 10);
      return Number.isNaN(parsed) ? 0 : parsed;
    });
}

export function compareVersions(actual, required) {
  const actualParts = cleanVersion(actual);
  const requiredParts = cleanVersion(required);
  const width = Math.max(actualParts.length, requiredParts.length);

  for (let index = 0; index < width; index += 1) {
    const left = actualParts[index] ?? 0;
    const right = requiredParts[index] ?? 0;
    if (left > right) {
      return 1;
    }
    if (left < right) {
      return -1;
    }
  }

  return 0;
}

function buildResult(kind, name, status, summary, definition, extras = {}) {
  return {
    kind,
    name,
    status,
    summary,
    required: definition.required !== false,
    fixHint: definition.fixHint ?? null,
    reason: definition.reason ?? null,
    ...extras
  };
}

export function evaluateTool(definition, installedTools = {}) {
  const actualVersion = installedTools[definition.name];
  const required = definition.required !== false;

  if (!actualVersion) {
    return buildResult(
      'tool',
      definition.name,
      required ? 'blocked' : 'warning',
      required
        ? `${definition.name} is missing`
        : `${definition.name} is optional and missing`,
      definition,
      { actualVersion: null, minVersion: definition.minVersion ?? null }
    );
  }

  if (definition.minVersion && compareVersions(actualVersion, definition.minVersion) < 0) {
    return buildResult(
      'tool',
      definition.name,
      required ? 'blocked' : 'warning',
      `${definition.name} ${actualVersion} is below ${definition.minVersion}`,
      definition,
      { actualVersion, minVersion: definition.minVersion }
    );
  }

  return buildResult(
    'tool',
    definition.name,
    'ready',
    `${definition.name} ${actualVersion} is ready`,
    definition,
    { actualVersion, minVersion: definition.minVersion ?? null }
  );
}

export function evaluateEnvVar(definition, env = {}) {
  const actualValue = env[definition.name];
  const required = definition.required !== false;

  if (actualValue === undefined || actualValue === null || actualValue === '') {
    return buildResult(
      'env',
      definition.name,
      required ? 'blocked' : 'warning',
      required
        ? `${definition.name} is missing`
        : `${definition.name} is optional and missing`,
      definition,
      { actualValue: null }
    );
  }

  if (
    Array.isArray(definition.allowedValues) &&
    definition.allowedValues.length > 0 &&
    !definition.allowedValues.includes(actualValue)
  ) {
    return buildResult(
      'env',
      definition.name,
      required ? 'blocked' : 'warning',
      `${definition.name} has an unsupported value`,
      definition,
      { actualValue }
    );
  }

  return buildResult(
    'env',
    definition.name,
    'ready',
    `${definition.name} is set`,
    definition,
    { actualValue }
  );
}

export function evaluateFile(definition, files = []) {
  const fileSet = files instanceof Set ? files : new Set(files);
  const required = definition.required !== false;
  const exists = fileSet.has(definition.path);

  if (!exists) {
    return buildResult(
      'file',
      definition.path,
      required ? 'blocked' : 'warning',
      required
        ? `${definition.path} is missing`
        : `${definition.path} is optional and missing`,
      definition,
      { exists }
    );
  }

  return buildResult(
    'file',
    definition.path,
    'ready',
    `${definition.path} is present`,
    definition,
    { exists }
  );
}

export function evaluateService(definition, services = {}) {
  const state = services[definition.name];
  const required = definition.required !== false;

  if (!state) {
    return buildResult(
      'service',
      definition.name,
      required ? 'blocked' : 'warning',
      required
        ? `${definition.name} is not configured`
        : `${definition.name} is optional and not configured`,
      definition,
      { reachable: false, running: false }
    );
  }

  if (state.status !== 'running') {
    return buildResult(
      'service',
      definition.name,
      required ? 'blocked' : 'warning',
      `${definition.name} is not running`,
      definition,
      { reachable: Boolean(state.reachable), running: false }
    );
  }

  if (definition.needReachable && !state.reachable) {
    return buildResult(
      'service',
      definition.name,
      required ? 'blocked' : 'warning',
      `${definition.name} is running but not reachable`,
      definition,
      { reachable: false, running: true }
    );
  }

  return buildResult(
    'service',
    definition.name,
    'ready',
    `${definition.name} is ready`,
    definition,
    { reachable: Boolean(state.reachable), running: true }
  );
}

export function buildDoctorReport(profile, snapshot = {}) {
  const tools = (profile.tools ?? []).map((definition) =>
    evaluateTool(definition, snapshot.tools ?? {})
  );
  const env = (profile.env ?? []).map((definition) =>
    evaluateEnvVar(definition, snapshot.env ?? {})
  );
  const files = (profile.files ?? []).map((definition) =>
    evaluateFile(definition, snapshot.files ?? [])
  );
  const services = (profile.services ?? []).map((definition) =>
    evaluateService(definition, snapshot.services ?? {})
  );

  const results = [...tools, ...env, ...files, ...services];
  const status = results.reduce((current, result) => {
    return STATUS_RANK[result.status] > STATUS_RANK[current] ? result.status : current;
  }, 'ready');

  const counts = {
    total: results.length,
    ready: results.filter((result) => result.status === 'ready').length,
    warning: results.filter((result) => result.status === 'warning').length,
    blocked: results.filter((result) => result.status === 'blocked').length
  };

  const nextSteps = uniqueValues(
    results
      .filter((result) => result.status !== 'ready')
      .map((result) => result.fixHint ?? result.summary)
  );
  const blockedResults = results.filter((result) => result.status === 'blocked');
  const warningResults = results.filter((result) => result.status === 'warning');

  return {
    profile: profile.name ?? 'unnamed-profile',
    status,
    counts,
    results,
    nextSteps,
    workflow: createWorkflow({
      name: 'environment-remediation-loop',
      summary: 'Fix blocked checks first, clear warnings second, then rerun the readiness gate.',
      automationTargets: ['local-bootstrap', 'ci-machine-readiness', 'preflight-check'],
      gateConditions: [
        blockedResults.length > 0 ? `${blockedResults.length} blocked check(s) require action` : 'no blocked checks',
        warningResults.length > 0 ? `${warningResults.length} warning check(s) remain` : 'no warning checks'
      ],
      stages: [
        createWorkflowStage({
          id: 'fix-blockers',
          title: 'Fix blockers',
          summary: 'Resolve missing required tools, files, env vars, or services.',
          status: blockedResults.length > 0 ? 'action-required' : 'clear',
          commands: blockedResults.map((result) => result.fixHint),
          items: blockedResults.map((result) => `${result.kind}:${result.name}`),
          notes: blockedResults.map((result) => result.summary)
        }),
        createWorkflowStage({
          id: 'clear-warnings',
          title: 'Clear warnings',
          summary: 'Resolve optional-but-useful setup drift so automation runs stay predictable.',
          status: warningResults.length > 0 ? 'recommended' : 'clear',
          commands: warningResults.map((result) => result.fixHint),
          items: warningResults.map((result) => `${result.kind}:${result.name}`),
          notes: warningResults.map((result) => result.summary)
        }),
        createWorkflowStage({
          id: 'rerun-readiness',
          title: 'Re-run readiness checks',
          summary: 'Run the doctor again after fixes so CI or onboarding can proceed cleanly.',
          notes: [
            `profile=${profile.name ?? 'unnamed-profile'}`,
            `status=${status}`
          ]
        })
      ]
    })
  };
}

export function formatDoctorReport(report) {
  const lines = [
    `Profile: ${report.profile}`,
    `Status: ${report.status.toUpperCase()}`,
    `Ready: ${report.counts.ready}`,
    `Warnings: ${report.counts.warning}`,
    `Blocked: ${report.counts.blocked}`
  ];

  if (report.nextSteps.length > 0) {
    lines.push('');
    lines.push('Next steps:');
    for (const step of report.nextSteps) {
      lines.push(`- ${step}`);
    }
  }

  if (report.results.length > 0) {
    lines.push('');
    lines.push('Checks:');
    for (const result of report.results) {
      lines.push(`[${result.status.toUpperCase()}] ${result.kind} ${result.name}: ${result.summary}`);
    }
  }

  lines.push('');
  lines.push('Automation loop:');
  for (const stage of report.workflow.stages) {
    lines.push(`- ${stage.title} [${stage.status}]`);
    lines.push(`  - ${stage.summary}`);
    for (const note of stage.notes) {
      lines.push(`  - note: ${note}`);
    }
  }

  return `${lines.join('\n')}\n`;
}
