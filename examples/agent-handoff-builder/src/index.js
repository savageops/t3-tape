import {
  buildApproveCommand,
  buildCommonCommands,
  buildRederiveCommand,
  buildUpdateCommand,
  readStateSurface,
  summarizeTriageCounts
} from '../../agent-kit/index.js';
import { uniqueValues } from '../../shared/collections.js';

function matchesFilter(patch, options) {
  if (options.patchId && patch.id !== options.patchId) {
    return false;
  }

  if (patch.triageStatus === 'pending-review') {
    return options.includePendingReview === true;
  }

  return ['CONFLICT', 'MISSING-SURFACE', 'NEEDS-YOU'].includes(patch.triageStatus);
}

export function resolveAgentMode(patch) {
  if (patch.agentMode) {
    return patch.agentMode;
  }

  if (
    patch.detectedStatus === 'MISSING-SURFACE' ||
    patch.triageStatus === 'MISSING-SURFACE'
  ) {
    return 're-derivation';
  }

  return 'conflict-resolution';
}

export function buildAgentJob(surface, patch) {
  const registryPatch = surface.patchIndex.get(patch.id) ?? null;
  const mode = resolveAgentMode(patch);
  const provider = surface.config.agent.provider;
  const needsConfiguration = provider === 'none' || !surface.config.agent.endpoint;
  const followUpCommand = mode === 're-derivation'
    ? buildRederiveCommand({ stateDir: surface.stateDir, patchId: patch.id })
    : buildUpdateCommand({
        stateDir: surface.stateDir,
        ref: surface.triage.toRef,
        confidenceThreshold: surface.config.agent.confidenceThreshold
      });
  const approvalCommand = patch.triageStatus === 'pending-review'
    ? buildApproveCommand({ stateDir: surface.stateDir, patchId: patch.id })
    : null;

  return {
    patchId: patch.id,
    title: patch.title,
    mode,
    triageStatus: patch.triageStatus,
    detectedStatus: patch.detectedStatus,
    confidence: patch.confidence,
    provider,
    endpoint: surface.config.agent.endpoint,
    needsConfiguration,
    intent: registryPatch?.intent ?? '',
    behaviorAssertions: registryPatch?.behaviorAssertions ?? [],
    surfaceHint: registryPatch?.surface ?? null,
    unresolved: patch.unresolved,
    notes: patch.notes,
    mergedUpstreamCandidate: patch.mergedUpstreamCandidate,
    resolvedDiffPath: patch.resolvedDiffPath,
    notesPath: patch.notesPath,
    rawResponsePath: patch.rawResponsePath,
    followUpCommand,
    approvalCommand,
    reviewCommand: buildCommonCommands(surface)[0] ?? null
  };
}

export function buildHandoffPacket(surfaceOrPath, options = {}) {
  const surface = typeof surfaceOrPath === 'string'
    ? readStateSurface(surfaceOrPath)
    : surfaceOrPath;
  const warnings = [];

  if (options.patchId && !surface.patchIndex.has(options.patchId)) {
    warnings.push(`Missing patch registry entry for ${options.patchId}.`);
  }

  const jobs = surface.triage.patches
    .filter((patch) => matchesFilter(patch, options))
    .map((patch) => {
      if (!surface.patchIndex.has(patch.id)) {
        warnings.push(`Missing patch registry entry for ${patch.id}.`);
      }

      return buildAgentJob(surface, patch);
    });

  return {
    project: surface.config.upstream,
    stateDir: surface.stateDir,
    fromRef: surface.triage.fromRef,
    toRef: surface.triage.toRef,
    provider: surface.config.agent.provider,
    endpoint: surface.config.agent.endpoint,
    confidenceThreshold: surface.config.agent.confidenceThreshold,
    triageCounts: summarizeTriageCounts(surface.triage),
    totalJobs: jobs.length,
    jobsByMode: {
      conflictResolution: jobs.filter((job) => job.mode === 'conflict-resolution').length,
      rederivation: jobs.filter((job) => job.mode === 're-derivation').length
    },
    commands: uniqueValues([
      ...buildCommonCommands(surface),
      ...jobs.flatMap((job) => [job.followUpCommand, job.approvalCommand])
    ]),
    jobs,
    warnings
  };
}

export function renderMarkdown(packet) {
  const lines = [
    '# Agent Handoff Queue',
    '',
    `Project: ${packet.project}`,
    `From: ${packet.fromRef}`,
    `To: ${packet.toRef}`,
    `Provider: ${packet.provider}`,
    `Jobs: ${packet.totalJobs}`
  ];

  if (packet.warnings.length > 0) {
    lines.push('');
    lines.push('Warnings:');
    for (const warning of packet.warnings) {
      lines.push(`- ${warning}`);
    }
  }

  if (packet.commands.length > 0) {
    lines.push('');
    lines.push('Common commands:');
    for (const command of packet.commands) {
      lines.push(`- ${command}`);
    }
  }

  for (const job of packet.jobs) {
    lines.push('');
    lines.push(`## ${job.patchId} ${job.mode}`);
    lines.push(`- title: ${job.title}`);
    lines.push(`- triage: ${job.triageStatus}`);
    lines.push(`- detected: ${job.detectedStatus}`);
    lines.push(`- confidence: ${job.confidence ?? 'n/a'}`);
    lines.push(`- needs configuration: ${job.needsConfiguration ? 'yes' : 'no'}`);
    lines.push(`- follow-up: ${job.followUpCommand}`);

    if (job.approvalCommand) {
      lines.push(`- approval: ${job.approvalCommand}`);
    }

    if (job.intent) {
      lines.push(`- intent: ${job.intent}`);
    }

    if (job.behaviorAssertions.length > 0) {
      lines.push('- assertions:');
      for (const assertion of job.behaviorAssertions) {
        lines.push(`  - ${assertion}`);
      }
    }

    if (job.unresolved.length > 0) {
      lines.push('- unresolved:');
      for (const item of job.unresolved) {
        lines.push(`  - ${item}`);
      }
    }
  }

  lines.push('');
  return `${lines.join('\n')}\n`;
}
