#!/usr/bin/env node

import fs from 'node:fs';
import { parseArgs } from 'node:util';
import { pathToFileURL } from 'node:url';

import { buildPlan, formatPlan } from './index.js';

function helpText() {
  return `test-impact-planner

Usage:
  test-impact-planner --manifest <path> (--changes-file <path> | --changed <file>...) [--json]

Options:
  --manifest      JSON manifest with routing rules
  --changes-file  Text file or JSON array of changed files
  --changed       One or more changed files provided directly
  --json          Print JSON instead of a text report
  --help          Show this message
`;
}

function readManifest(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function readChangedFiles(filePath) {
  const raw = fs.readFileSync(filePath, 'utf8').trim();
  if (!raw) {
    return [];
  }

  if (raw.startsWith('[')) {
    return JSON.parse(raw);
  }

  return raw.split(/\r?\n/).filter(Boolean);
}

export function run(argv = process.argv.slice(2), io = process) {
  const { values } = parseArgs({
    args: argv,
    options: {
      manifest: { type: 'string' },
      'changes-file': { type: 'string' },
      changed: { type: 'string', multiple: true },
      json: { type: 'boolean' },
      help: { type: 'boolean' }
    },
    allowPositionals: false
  });

  if (values.help) {
    io.stdout.write(helpText());
    return 0;
  }

  if (!values.manifest) {
    io.stderr.write('Missing required option: --manifest\n');
    return 1;
  }

  if (!values['changes-file'] && (!values.changed || values.changed.length === 0)) {
    io.stderr.write('Provide --changes-file or at least one --changed value\n');
    return 1;
  }

  try {
    const manifest = readManifest(values.manifest);
    const changedFiles = values['changes-file']
      ? readChangedFiles(values['changes-file'])
      : values.changed;
    const plan = buildPlan(manifest, changedFiles);

    if (values.json) {
      io.stdout.write(`${JSON.stringify(plan, null, 2)}\n`);
    } else {
      io.stdout.write(formatPlan(plan));
    }

    return plan.mode === 'full-run' ? 2 : 0;
  } catch (error) {
    io.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    return 1;
  }
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = run();
}
