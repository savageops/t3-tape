import {
  buildApproveCommand,
  buildRederiveCommand,
  buildTriageCommand,
  buildUpdateCommand,
  readStateSurface
} from '../../agent-kit/index.js';
import { uniqueValues } from '../../shared/collections.js';

const PRIORITY_LABELS = {
  1: 'P1',
  2: 'P2',
  3: 'P3'
};

function toAssertionRecord(entry) {
  const failingAssertions = uniqueValues(
    entry['failing-assertions'] ?? entry.failingAssertions ?? []
  );
  const failed = entry.failed ?? failingAssertions.length;
  const passed = entry.passed ?? 0;

  return {
    id: entry.id,
    passed,
    failed,
    failingAssertions,
    status: failed > 0 ? 'failing' : passed > 0 ? 'passing' : 'unknown'
  };
}

export function normalizeAssertionResults(input) {
  if (!input) {
    return {};
  }

  let entries = [];

  if (Array.isArray(input)) {
    entries = input;
  } else if (Array.isArray(input.patches)) {
    entries = input.patches;
  } else {
    entries = Object.entries(input).map(([id, value]) => ({ id, ...value }));
  }

  return Object.fromEntries(entries.map((entry) => [entry.id, toAssertionRecord(entry)]));
}

function buildPatchCommand(surface, patch) {
  if (patch.detectedStatus === 'MISSING-SURFACE') {
    return buildRederiveCommand({ stateDir: surface.stateDir, patchId: patch.id });
  }

  if (patch.triageStatus === 'pending-review' || patch.triageStatus === 'CLEAN') {
    return buildApproveCommand({ stateDir: surface.stateDir, patchId: patch.id });
  }

  return buildUpdateCommand({
    stateDir: surface.stateDir,
    ref: surface.triage.toRef,
    confidenceThreshold: surface.config.agent.confidenceThreshold
  });
}

export function classifyPatchFinding(surface, patch, assertion, options = {}) {
  const threshold = options.threshold ?? surface.config.agent.confidenceThreshold;
  const margin = options.margin ?? 0.05;

  if (['NEEDS-YOU', 'CONFLICT', 'MISSING-SURFACE'].includes(patch.triageStatus)) {
    return {
      patchId: patch.id,
      title: patch.title,
      priority: 1,
      priorityLabel: PRIORITY_LABELS[1],
      summary: `${patch.id} still needs operator help before it can be approved.`,
      command: buildPatchCommand(surface, patch)
    };
  }

  if (patch.triageStatus === 'pending-review' && assertion?.failed > 0) {
    return {
      patchId: patch.id,
      title: patch.title,
      priority: 1,
      priorityLabel: PRIORITY_LABELS[1],
      summary: `${patch.id} is pending review but still has failing behavior assertions.`,
      command: buildTriageCommand({ stateDir: surface.stateDir, json: true })
    };
  }

  if (patch.triageStatus === 'pending-review' && patch.unresolved.length > 0) {
    return {
      patchId: patch.id,
      title: patch.title,
      priority: 2,
      priorityLabel: PRIORITY_LABELS[2],
      summary: `${patch.id} resolved at the diff level but still reports unresolved behavior.`,
      command: buildTriageCommand({ stateDir: surface.stateDir, json: true })
    };
  }

  if (
    patch.triageStatus === 'pending-review' &&
    patch.confidence !== null &&
    patch.confidence < threshold + margin
  ) {
    return {
      patchId: patch.id,
      title: patch.title,
      priority: 2,
      priorityLabel: PRIORITY_LABELS[2],
      summary: `${patch.id} is near the confidence threshold and should be reviewed carefully.`,
      command: buildTriageCommand({ stateDir: surface.stateDir, json: true })
    };
  }

  if (patch.mergedUpstreamCandidate && !patch.approved) {
    return {
      patchId: patch.id,
      title: patch.title,
      priority: 2,
      priorityLabel: PRIORITY_LABELS[2],
      summary: `${patch.id} may already be covered upstream and should be checked before reapproval.`,
      command: buildTriageCommand({ stateDir: surface.stateDir, json: true })
    };
  }

  if (
    ['pending-review', 'CLEAN'].includes(patch.triageStatus) &&
    !patch.approved
  ) {
    return {
      patchId: patch.id,
      title: patch.title,
      priority: 3,
      priorityLabel: PRIORITY_LABELS[3],
      summary: `${patch.id} is ready for a normal approval pass.`,
      command: buildApproveCommand({ stateDir: surface.stateDir, patchId: patch.id })
    };
  }

  return null;
}

export function buildReviewReport(surfaceOrPath, assertionInput, options = {}) {
  const surface = typeof surfaceOrPath === 'string'
    ? readStateSurface(surfaceOrPath)
    : surfaceOrPath;
  const assertions = normalizeAssertionResults(assertionInput);
  const findings = [];

  if (surface.triage.preview && surface.triage.preview.exitCode !== 0) {
    findings.push({
      patchId: 'PREVIEW',
      title: 'preview',
      priority: 1,
      priorityLabel: PRIORITY_LABELS[1],
      summary: `Sandbox preview failed with exit code ${surface.triage.preview.exitCode}.`,
      command: buildTriageCommand({ stateDir: surface.stateDir, json: true })
    });
  }

  for (const patch of surface.triage.patches) {
    const finding = classifyPatchFinding(surface, patch, assertions[patch.id], options);
    if (finding) {
      findings.push(finding);
    }
  }

  findings.sort((left, right) => left.priority - right.priority || left.patchId.localeCompare(right.patchId));

  const approvalCandidates = surface.triage.patches
    .filter((patch) => ['pending-review', 'CLEAN'].includes(patch.triageStatus))
    .filter((patch) => !patch.approved)
    .filter((patch) => (assertions[patch.id]?.failed ?? 0) === 0)
    .filter((patch) => patch.unresolved.length === 0)
    .filter((patch) => patch.confidence === null || patch.confidence >= (options.threshold ?? surface.config.agent.confidenceThreshold))
    .map((patch) => ({
      patchId: patch.id,
      title: patch.title,
      command: buildApproveCommand({ stateDir: surface.stateDir, patchId: patch.id })
    }));

  const highestPriority = findings[0]?.priority ?? 0;
  const decision = highestPriority === 1
    ? 'request-changes'
    : approvalCandidates.length > 0 || findings.length > 0
      ? 'comment'
      : 'approve';

  return {
    project: surface.config.upstream,
    stateDir: surface.stateDir,
    decision,
    threshold: options.threshold ?? surface.config.agent.confidenceThreshold,
    preview: surface.triage.preview,
    findings,
    reviewComments: findings.map((finding) => ({
      patchId: finding.patchId,
      title: `[${finding.priorityLabel}] ${finding.title}`,
      body: finding.summary,
      command: finding.command
    })),
    approvalCandidates,
    commands: uniqueValues([
      buildTriageCommand({ stateDir: surface.stateDir, json: true }),
      ...approvalCandidates.map((candidate) => candidate.command),
      ...findings.map((finding) => finding.command)
    ])
  };
}

export function renderMarkdown(report) {
  const lines = [
    '# Migration Review Report',
    '',
    `Project: ${report.project}`,
    `Decision: ${report.decision}`,
    `Approval candidates: ${report.approvalCandidates.length}`,
    `Findings: ${report.findings.length}`
  ];

  if (report.approvalCandidates.length > 0) {
    lines.push('');
    lines.push('Approval candidates:');
    for (const candidate of report.approvalCandidates) {
      lines.push(`- ${candidate.patchId} ${candidate.command}`);
    }
  }

  if (report.findings.length > 0) {
    lines.push('');
    lines.push('Findings:');
    for (const finding of report.findings) {
      lines.push(`- ${finding.priorityLabel} ${finding.patchId}: ${finding.summary}`);
    }
  }

  if (report.commands.length > 0) {
    lines.push('');
    lines.push('Commands:');
    for (const command of report.commands) {
      lines.push(`- ${command}`);
    }
  }

  lines.push('');
  return `${lines.join('\n')}\n`;
}
