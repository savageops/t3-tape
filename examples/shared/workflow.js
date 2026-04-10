import { uniqueValues } from './collections.js';

export function createWorkflowStage({
  id,
  title,
  summary,
  status = 'ready',
  commands = [],
  items = [],
  notes = []
}) {
  return {
    id,
    title,
    summary,
    status,
    itemCount: items.length,
    items,
    commands: uniqueValues(commands.filter(Boolean)),
    notes: uniqueValues(notes.filter(Boolean))
  };
}

export function createWorkflow({
  name,
  summary,
  stages = [],
  automationTargets = [],
  gateConditions = []
}) {
  return {
    name,
    summary,
    stageCount: stages.length,
    stages,
    automationTargets: uniqueValues(automationTargets.filter(Boolean)),
    gateConditions: uniqueValues(gateConditions.filter(Boolean))
  };
}
