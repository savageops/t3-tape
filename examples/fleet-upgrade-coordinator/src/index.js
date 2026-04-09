import { buildUpdateCommand } from '../../agent-kit/index.js';

const PRIORITY_RANK = {
  high: 0,
  medium: 1,
  low: 2
};

function splitVersion(value) {
  const rendered = String(value).trim().replace(/^v/i, '');
  const [core, prerelease = ''] = rendered.split('-', 2);

  return {
    coreParts: core.split('.').map((segment) => Number.parseInt(segment, 10) || 0),
    prereleaseParts: prerelease
      ? prerelease.split('.').map((segment) => (/^\d+$/u.test(segment) ? Number(segment) : segment))
      : []
  };
}

export function compareVersions(left, right) {
  const leftVersion = splitVersion(left);
  const rightVersion = splitVersion(right);
  const width = Math.max(leftVersion.coreParts.length, rightVersion.coreParts.length);

  for (let index = 0; index < width; index += 1) {
    const leftPart = leftVersion.coreParts[index] ?? 0;
    const rightPart = rightVersion.coreParts[index] ?? 0;
    if (leftPart > rightPart) {
      return 1;
    }
    if (leftPart < rightPart) {
      return -1;
    }
  }

  if (leftVersion.prereleaseParts.length === 0 && rightVersion.prereleaseParts.length === 0) {
    return 0;
  }

  if (leftVersion.prereleaseParts.length === 0) {
    return 1;
  }

  if (rightVersion.prereleaseParts.length === 0) {
    return -1;
  }

  const prereleaseWidth = Math.max(
    leftVersion.prereleaseParts.length,
    rightVersion.prereleaseParts.length
  );

  for (let index = 0; index < prereleaseWidth; index += 1) {
    const leftPart = leftVersion.prereleaseParts[index];
    const rightPart = rightVersion.prereleaseParts[index];

    if (leftPart === undefined) {
      return -1;
    }

    if (rightPart === undefined) {
      return 1;
    }

    if (typeof leftPart === 'number' && typeof rightPart === 'number') {
      if (leftPart > rightPart) {
        return 1;
      }

      if (leftPart < rightPart) {
        return -1;
      }
      continue;
    }

    if (typeof leftPart === 'number' && typeof rightPart === 'string') {
      return -1;
    }

    if (typeof leftPart === 'string' && typeof rightPart === 'number') {
      return 1;
    }

    const comparison = String(leftPart).localeCompare(String(rightPart));
    if (comparison !== 0) {
      return comparison > 0 ? 1 : -1;
    }
  }

  return 0;
}

export function classifyVersionDelta(currentRef, targetRef) {
  if (compareVersions(currentRef, targetRef) >= 0) {
    return 'same';
  }

  const [currentMajor, currentMinor] = splitVersion(currentRef).coreParts;
  const [targetMajor, targetMinor] = splitVersion(targetRef).coreParts;

  if (targetMajor > currentMajor) {
    return 'major';
  }

  if (targetMinor > currentMinor) {
    return 'minor';
  }

  return 'patch';
}

export function pickTargetRelease(project, releases) {
  const candidates = releases
    .filter((release) => release.upstream === project.upstream)
    .filter((release) => !release.channel || release.channel === project['update-channel'])
    .filter((release) => project['accept-prerelease'] || !String(release.version).includes('-'))
    .filter((release) => compareVersions(release.version, project['current-ref']) > 0)
    .sort((left, right) => compareVersions(right.version, left.version));

  const securityCandidate = candidates.filter((release) => release.security)[0];
  return securityCandidate ?? candidates[0] ?? null;
}

export function classifyProjectAction(project, release) {
  if (!release) {
    return { action: 'skip', reason: 'No newer release matched the project policy.' };
  }

  if (project['has-open-triage']) {
    return { action: 'hold', reason: 'Existing triage is still unresolved.' };
  }

  const delta = classifyVersionDelta(project['current-ref'], release.version);
  if (delta === 'same') {
    return { action: 'skip', reason: 'Project is already up to date.' };
  }

  if (delta === 'major' && !project['allow-breaking']) {
    return { action: 'hold', reason: 'Breaking upgrades are disabled for this fork.' };
  }

  if (release.security) {
    return { action: 'run-now', reason: 'Security release should run immediately.' };
  }

  if (delta === 'patch' && project['auto-run']) {
    return { action: 'run-now', reason: 'Patch upgrade is eligible for immediate automation.' };
  }

  if ((delta === 'patch' || delta === 'minor') && !project['auto-run']) {
    return { action: 'schedule', reason: 'Upgrade is safe enough to schedule but not auto-run.' };
  }

  if (delta === 'minor' && project['auto-run']) {
    return { action: 'schedule', reason: 'Minor upgrades should be staged during the next update window.' };
  }

  if (delta === 'major' && project['allow-breaking']) {
    return { action: 'schedule', reason: 'Breaking upgrades are allowed but should be staged deliberately.' };
  }

  return { action: 'hold', reason: 'Project policy did not allow this release to run automatically.' };
}

export function buildProjectPlan(project, releases) {
  const release = pickTargetRelease(project, releases);
  const classification = classifyProjectAction(project, release);

  return {
    projectId: project.id,
    upstream: project.upstream,
    currentRef: project['current-ref'],
    targetRef: release?.version ?? null,
    action: classification.action,
    reason: classification.reason,
    delta: release ? classifyVersionDelta(project['current-ref'], release.version) : null,
    priority: project.priority ?? 'medium',
    command: release
      ? buildUpdateCommand({
          repoRoot: project['repo-root'],
          stateDir: project['state-dir'],
          ref: release.version,
          ci: true,
          confidenceThreshold: project['confidence-threshold']
        })
      : null
  };
}

export function buildFleetPlan(manifest, releases) {
  const plans = (manifest.projects ?? [])
    .map((project) => buildProjectPlan(project, releases))
    .sort((left, right) => {
      const actionOrder = ['run-now', 'schedule', 'hold', 'skip'];
      return actionOrder.indexOf(left.action) - actionOrder.indexOf(right.action) ||
        PRIORITY_RANK[left.priority] - PRIORITY_RANK[right.priority] ||
        left.projectId.localeCompare(right.projectId);
    });

  return {
    counts: {
      runNow: plans.filter((plan) => plan.action === 'run-now').length,
      schedule: plans.filter((plan) => plan.action === 'schedule').length,
      hold: plans.filter((plan) => plan.action === 'hold').length,
      skip: plans.filter((plan) => plan.action === 'skip').length
    },
    plans
  };
}

export function renderMarkdown(plan) {
  const lines = [
    '# Fleet Upgrade Plan',
    '',
    `Run now: ${plan.counts.runNow}`,
    `Schedule: ${plan.counts.schedule}`,
    `Hold: ${plan.counts.hold}`,
    `Skip: ${plan.counts.skip}`
  ];

  for (const entry of plan.plans) {
    lines.push('');
    lines.push(`## ${entry.projectId}`);
    lines.push(`- action: ${entry.action}`);
    lines.push(`- current: ${entry.currentRef}`);
    lines.push(`- target: ${entry.targetRef ?? 'none'}`);
    lines.push(`- reason: ${entry.reason}`);
    if (entry.command) {
      lines.push(`- command: ${entry.command}`);
    }
  }

  lines.push('');
  return `${lines.join('\n')}\n`;
}
