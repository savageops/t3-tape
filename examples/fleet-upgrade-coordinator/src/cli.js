#!/usr/bin/env node

import fs from 'node:fs';

import { buildFleetPlan, renderMarkdown } from './index.js';

function printHelp() {
  process.stdout.write(`fleet-upgrade-coordinator

Usage:
  node src/cli.js --manifest <path> --releases <path> [--format json|markdown]

Options:
  --manifest  fleet manifest JSON
  --releases  release feed JSON
  --format    output format, default json
  --help      show this help text
`);
}

function parseArgs(argv) {
  const options = { format: 'json' };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '--help') {
      options.help = true;
    } else if (arg === '--manifest') {
      options.manifestPath = argv[index + 1];
      index += 1;
    } else if (arg === '--releases') {
      options.releasesPath = argv[index + 1];
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

  if (!options.manifestPath) {
    throw new Error('Missing required option: --manifest');
  }

  if (!options.releasesPath) {
    throw new Error('Missing required option: --releases');
  }

  if (!['json', 'markdown'].includes(options.format)) {
    throw new Error(`Unsupported format: ${options.format}`);
  }

  const manifest = JSON.parse(fs.readFileSync(options.manifestPath, 'utf8'));
  const releases = JSON.parse(fs.readFileSync(options.releasesPath, 'utf8'));
  const plan = buildFleetPlan(manifest, releases);

  if (options.format === 'markdown') {
    process.stdout.write(renderMarkdown(plan));
  } else {
    process.stdout.write(`${JSON.stringify(plan, null, 2)}\n`);
  }
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exit(1);
}
