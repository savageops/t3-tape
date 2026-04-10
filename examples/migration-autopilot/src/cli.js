#!/usr/bin/env node

import { renderMarkdown, runMigrationAutopilot } from './index.js';

function printHelp() {
  process.stdout.write(`migration-autopilot

Usage:
  node src/cli.js [--binary <path>] [--workspace-root <path>] [--format json|markdown] [--keep-temp] [--no-auto-approve]

Options:
  --binary           explicit t3-tape binary path
  --workspace-root   explicit temp workspace root
  --format           output format, default json
  --keep-temp        keep the generated temp repos for inspection
  --no-auto-approve  stop after update, handoff, and review synthesis
  --help             show this help text
`);
}

function parseArgs(argv) {
  const options = {
    format: 'json',
    autoApproveReady: true,
    keepTemp: false
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '--help') {
      options.help = true;
    } else if (arg === '--binary') {
      options.binaryPath = argv[index + 1];
      index += 1;
    } else if (arg === '--workspace-root') {
      options.workspaceRoot = argv[index + 1];
      index += 1;
    } else if (arg === '--format') {
      options.format = argv[index + 1];
      index += 1;
    } else if (arg === '--keep-temp') {
      options.keepTemp = true;
    } else if (arg === '--no-auto-approve') {
      options.autoApproveReady = false;
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

  if (!['json', 'markdown'].includes(options.format)) {
    throw new Error(`Unsupported format: ${options.format}`);
  }

  const result = runMigrationAutopilot(options);
  if (options.format === 'markdown') {
    process.stdout.write(renderMarkdown(result));
  } else {
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  }
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exit(1);
}
