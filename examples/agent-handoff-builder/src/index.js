import {
  buildApproveCommand,
  buildCommonCommands,
  buildRederiveCommand,
  buildUpdateCommand,
  readStateSurface,
  summarizeTriageCounts
} from '../../agent-kit/index.js';
import { uniqueValues } from '../../shared/collections.js';
import { createWorkflow, createWorkflowStage } from '../../shared/workflow.js';

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

export function buildHandoffWorkflow(surface, jobs, options = {}) {
  const agentConfigured = surface.config.agent.provider !== 'none' && Boolean(surface.config.agent.endpoint);
  const approvalCommands = jobs
    .map((job) => job.approvalCommand)
    .filter(Boolean);
  const jobPatchIds = jobs.map((job) => job.patchId);

  return createWorkflow({
    name: 'agent-handoff-loop',
    summary: 'Turn triage state into an agent queue, refresh the migration state, then approve safe patches.',
    automationTargets: ['ci-update-job', 'agent-runner', 'chatops-review-bot'],
    gateConditions: [
      `confidence-threshold >= ${surface.config.agent.confidenceThreshold}`,
      surface.config.sandbox.previewCommand
        ? `preview-command configured: ${surface.config.sandbox.previewCommand}`
        : 'preview-command not configured',
      agentConfigured
        ? `agent endpoint ready: ${surface.config.agent.endpoint}`
        : 'agent endpoint missing'
    ],
    stages: [
      createWorkflowStage({
        id: 'read-triage',
        title: 'Read triage state',
        summary: 'Load the current PatchMD triage surface and build the unresolved job queue.',
        commands: [buildCommonCommands(surface)[0]],
        items: jobPatchIds,
        notes: [
          `from ${surface.triage.fromRef} to ${surface.triage.toRef}`,
          `${jobs.length} queued job(s)`
        ]
      }),
      createWorkflowStage({
        id: 'dispatch-agents',
        title: 'Dispatch agent jobs',
        summary: 'Send each queued patch to the configured agent runner using the provider and endpoint from config.json.',
        status: agentConfigured ? 'ready' : 'requires-configuration',
        commands: agentConfigured ? [surface.config.agent.endpoint] : [],
        items: jobs.map((job) => `${job.patchId}:${job.mode}`),
        notes: [
          `provider=${surface.config.agent.provider}`,
          `max-attempts=${surface.config.agent.maxAttempts}`,
          surface.config.hooks.onConflict
            ? `on-conflict hook=${surface.config.hooks.onConflict}`
            : 'on-conflict hook not configured'
        ]
      }),
      createWorkflowStage({
        id: 'refresh-triage',
        title: 'Refresh triage after agent attempts',
        summary: 'Re-run update to fold agent results back into the sandbox triage state.',
        commands: uniqueValues(jobs.map((job) => job.followUpCommand)),
        items: jobPatchIds,
        notes: [
          surface.config.hooks.preUpdate
            ? `pre-update hook=${surface.config.hooks.preUpdate}`
            : 'pre-update hook not configured',
          surface.config.hooks.postUpdate
            ? `post-update hook=${surface.config.hooks.postUpdate}`
            : 'post-update hook not configured'
        ]
      }),
      createWorkflowStage({
        id: 'approve-ready',
        title: 'Approve safe patches',
        summary: 'Approve patches that have returned to pending-review with acceptable confidence and no unresolved assertions.',
        status: approvalCommands.length > 0 ? 'ready' : 'no-ready-patches',
        commands: approvalCommands,
        items: jobs
          .filter((job) => job.approvalCommand)
          .map((job) => job.patchId),
        notes: [
          surface.config.sandbox.previewCommand
            ? `preview-command=${surface.config.sandbox.previewCommand}`
            : 'preview-command not configured'
        ]
      })
    ]
  });
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
    configuration: {
      agent: {
        provider: surface.config.agent.provider,
        endpoint: surface.config.agent.endpoint,
        confidenceThreshold: surface.config.agent.confidenceThreshold,
        maxAttempts: surface.config.agent.maxAttempts
      },
      sandbox: {
        previewCommand: surface.config.sandbox.previewCommand
      },
      hooks: surface.config.hooks
    },
    jobs,
    workflow: buildHandoffWorkflow(surface, jobs, options),
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

  lines.push('');
  lines.push('Automation loop:');
  for (const stage of packet.workflow.stages) {
    lines.push(`- ${stage.title} [${stage.status}]`);
    lines.push(`  - ${stage.summary}`);
    if (stage.commands.length > 0) {
      for (const command of stage.commands) {
        lines.push(`  - command: ${command}`);
      }
    }
    for (const note of stage.notes) {
      lines.push(`  - note: ${note}`);
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
