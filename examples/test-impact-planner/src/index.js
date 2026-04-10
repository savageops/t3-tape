import { uniqueValues } from '../../shared/collections.js';
import { createWorkflow, createWorkflowStage } from '../../shared/workflow.js';

const RISK_RANK = {
  none: 0,
  low: 1,
  medium: 2,
  high: 3,
  critical: 4
};

export function normalizeChangedFiles(files = []) {
  return [...new Set(
    files
      .map((file) => String(file).trim().replace(/\\/g, '/').replace(/^\.\//, ''))
      .filter(Boolean)
  )];
}

function escapeRegex(value) {
  return value.replace(/[|\\{}()[\]^$+?.]/g, '\\$&');
}

export function globToRegExp(pattern) {
  let expression = '^';

  for (let index = 0; index < pattern.length; index += 1) {
    const character = pattern[index];

    if (character === '*') {
      const next = pattern[index + 1];
      const afterNext = pattern[index + 2];

      if (next === '*') {
        if (afterNext === '/') {
          expression += '(?:.*/)?';
          index += 2;
        } else {
          expression += '.*';
          index += 1;
        }
      } else {
        expression += '[^/]*';
      }

      continue;
    }

    expression += escapeRegex(character);
  }

  expression += '$';
  return new RegExp(expression);
}

export function pathMatchesPattern(filePath, pattern) {
  return globToRegExp(pattern).test(filePath);
}

function highestRisk(current, incoming) {
  return RISK_RANK[incoming] > RISK_RANK[current] ? incoming : current;
}

function expandRuleMatch(rule, files) {
  const matchedFiles = files.filter((file) =>
    (rule.match ?? []).some((pattern) => pathMatchesPattern(file, pattern))
  );

  if (matchedFiles.length === 0) {
    return null;
  }

  return {
    id: rule.id,
    matchedFiles,
    commands: rule.commands ?? [],
    owners: rule.owners ?? [],
    labels: rule.labels ?? [],
    risk: rule.risk ?? 'low',
    reason: rule.reason ?? rule.id
  };
}

export function buildPlan(manifest, changedFiles) {
  const files = normalizeChangedFiles(changedFiles);
  const ignoredFiles = files.filter((file) =>
    (manifest.ignore ?? []).some((pattern) => pathMatchesPattern(file, pattern))
  );
  const activeFiles = files.filter((file) => !ignoredFiles.includes(file));

  if (files.length === 0) {
    return {
      mode: 'empty',
      changedFiles: [],
      ignoredFiles: [],
      unmatchedFiles: [],
      matchedRules: [],
      commands: [],
      owners: [],
      labels: [],
      reasons: [],
      risk: 'none'
    };
  }

  if (activeFiles.length === 0) {
    return {
      mode: 'docs-only',
      changedFiles: files,
      ignoredFiles,
      unmatchedFiles: [],
      matchedRules: [],
      commands: [],
      owners: [],
      labels: ['docs-only'],
      reasons: ['Only ignored documentation or note files changed.'],
      risk: 'none'
    };
  }

  const fullRunMatches = (manifest.fullRunRules ?? [])
    .map((rule) => expandRuleMatch(rule, activeFiles))
    .filter(Boolean);
  const ruleMatches = (manifest.rules ?? [])
    .map((rule) => expandRuleMatch(rule, activeFiles))
    .filter(Boolean);
  const matchedFiles = uniqueValues([
    ...fullRunMatches.flatMap((rule) => rule.matchedFiles),
    ...ruleMatches.flatMap((rule) => rule.matchedFiles)
  ]);
  const unmatchedFiles = activeFiles.filter((file) => !matchedFiles.includes(file));

  let mode = 'targeted';
  if (fullRunMatches.length > 0) {
    mode = 'full-run';
  } else if (ruleMatches.length === 0) {
    mode = 'fallback';
  }

  const allMatches = [...fullRunMatches, ...ruleMatches];
  const commands = uniqueValues([
    ...allMatches.flatMap((rule) => rule.commands),
    ...(mode === 'fallback' ? manifest.defaultCommands ?? [] : [])
  ]);
  const owners = uniqueValues([
    ...allMatches.flatMap((rule) => rule.owners),
    ...(mode === 'fallback' ? manifest.defaultOwners ?? [] : [])
  ]);
  const labels = uniqueValues(allMatches.flatMap((rule) => rule.labels));
  const reasons = uniqueValues([
    ...allMatches.map((rule) => rule.reason),
    ...(mode === 'fallback' ? ['No specific rule matched. Using the default test plan.'] : [])
  ]);
  const risk = allMatches.reduce((current, rule) => highestRisk(current, rule.risk), mode === 'fallback' ? 'low' : 'none');
  const commandBatches = allMatches.map((rule) => ({
    id: rule.id,
    commands: rule.commands,
    matchedFiles: rule.matchedFiles,
    owners: rule.owners,
    labels: rule.labels,
    risk: rule.risk,
    reason: rule.reason
  }));

  if (mode === 'fallback' && (manifest.defaultCommands ?? []).length > 0) {
    commandBatches.push({
      id: 'default-fallback',
      commands: manifest.defaultCommands ?? [],
      matchedFiles: unmatchedFiles,
      owners: manifest.defaultOwners ?? [],
      labels: ['fallback'],
      risk: 'low',
      reason: 'No explicit rule matched. Running the default validation plan.'
    });
  }

  return {
    mode,
    changedFiles: files,
    ignoredFiles,
    unmatchedFiles,
    matchedRules: allMatches,
    commands,
    owners,
    labels,
    reasons,
    risk,
    commandBatches,
    workflow: createWorkflow({
      name: 'validation-routing-loop',
      summary: 'Classify the change set, run the smallest useful validation batch, then route the result to the right owners.',
      automationTargets: ['ci-router', 'pull-request-bot', 'change-risk-board'],
      gateConditions: [
        `mode=${mode}`,
        `risk=${risk}`
      ],
      stages: [
        createWorkflowStage({
          id: 'classify-change-set',
          title: 'Classify the change set',
          summary: 'Normalize changed files and determine whether the run is docs-only, targeted, full-run, or fallback.',
          items: files,
          notes: [
            `${ignoredFiles.length} ignored file(s)`,
            `${unmatchedFiles.length} unmatched file(s)`
          ]
        }),
        createWorkflowStage({
          id: 'run-validation-batches',
          title: 'Run validation batches',
          summary: 'Execute only the commands selected by the matching rules.',
          status: commands.length > 0 ? 'ready' : 'no-commands',
          commands,
          items: commandBatches.map((batch) => batch.id),
          notes: reasons
        }),
        createWorkflowStage({
          id: 'route-result',
          title: 'Route the result',
          summary: 'Send the run to the correct owners and labels for follow-up.',
          items: owners,
          notes: labels
        })
      ]
    })
  };
}

export function formatPlan(plan) {
  const lines = [
    `Mode: ${plan.mode}`,
    `Risk: ${plan.risk.toUpperCase()}`,
    `Commands: ${plan.commands.length}`,
    `Owners: ${plan.owners.length}`,
    `Labels: ${plan.labels.length}`
  ];

  if (plan.commands.length > 0) {
    lines.push('');
    lines.push('Run:');
    for (const command of plan.commands) {
      lines.push(`- ${command}`);
    }
  }

  if (plan.reasons.length > 0) {
    lines.push('');
    lines.push('Why:');
    for (const reason of plan.reasons) {
      lines.push(`- ${reason}`);
    }
  }

  lines.push('');
  lines.push('Automation loop:');
  for (const stage of plan.workflow.stages) {
    lines.push(`- ${stage.title} [${stage.status}]`);
    lines.push(`  - ${stage.summary}`);
    for (const command of stage.commands) {
      lines.push(`  - command: ${command}`);
    }
    for (const note of stage.notes) {
      lines.push(`  - note: ${note}`);
    }
  }

  return `${lines.join('\n')}\n`;
}
