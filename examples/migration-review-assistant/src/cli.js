#!/usr/bin/env node

import fs from 'node:fs';

import { buildReviewReport, renderMarkdown, normalizeAssertionResults } from './index.js';

function printHelp() {
  process.stdout.write(`migration-review-assistant

Usage:
  node src/cli.js --state-dir <path> [--assertions <path>] [--format json|markdown]

Options:
  --state-dir    path to .t3 or repo root
  --assertions   optional JSON assertion summary
  --format       output format, default json
  --help         show this help text
`);
}

function parseArgs(argv) {
  const options = { format: 'json' };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '--help') {
      options.help = true;
    } else if (arg === '--state-dir') {
      options.stateDir = argv[index + 1];
      index += 1;
    } else if (arg === '--assertions') {
      options.assertionsPath = argv[index + 1];
      index += 1;
    } else if (arg === '--format') {
      options.format = argv[index + 1];
      index += 1;
    } else {
      throw new Error(`Unknown option: ${arg}`);
    }
  }

  return options;
}

try {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    printHelp();
    process.exit(0);
  }

  if (!options.stateDir) {
    throw new Error('Missing required option: --state-dir');
  }

  if (!['json', 'markdown'].includes(options.format)) {
    throw new Error(`Unsupported format: ${options.format}`);
  }

  const assertions = options.assertionsPath
    ? normalizeAssertionResults(JSON.parse(fs.readFileSync(options.assertionsPath, 'utf8')))
    : {};
  const report = buildReviewReport(options.stateDir, assertions);

  if (options.format === 'markdown') {
    process.stdout.write(renderMarkdown(report));
  } else {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  }
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exit(1);
}
