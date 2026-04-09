#!/usr/bin/env node

import fs from 'node:fs';
import { parseArgs } from 'node:util';
import { pathToFileURL } from 'node:url';

import { buildDoctorReport, formatDoctorReport } from './index.js';

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function helpText() {
  return `dev-env-doctor

Usage:
  dev-env-doctor --profile <path> [--snapshot <path>] [--json]

Options:
  --profile   JSON profile describing the required environment
  --snapshot  JSON snapshot describing the current machine state
  --json      Print JSON instead of a text report
  --help      Show this message
`;
}

export function run(argv = process.argv.slice(2), io = process) {
  const { values } = parseArgs({
    args: argv,
    options: {
      profile: { type: 'string' },
      snapshot: { type: 'string' },
      json: { type: 'boolean' },
      help: { type: 'boolean' }
    },
    allowPositionals: false
  });

  if (values.help) {
    io.stdout.write(helpText());
    return 0;
  }

  if (!values.profile) {
    io.stderr.write('Missing required option: --profile\n');
    return 1;
  }

  try {
    const profile = readJson(values.profile);
    const snapshot = values.snapshot
      ? readJson(values.snapshot)
      : { tools: {}, env: process.env, files: [], services: {} };
    const report = buildDoctorReport(profile, snapshot);

    if (values.json) {
      io.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
    } else {
      io.stdout.write(formatDoctorReport(report));
    }

    return report.status === 'blocked' ? 2 : 0;
  } catch (error) {
    io.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    return 1;
  }
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = run();
}
