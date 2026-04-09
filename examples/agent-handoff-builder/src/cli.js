#!/usr/bin/env node

import { buildHandoffPacket, renderMarkdown } from './index.js';

function printHelp() {
  process.stdout.write(`agent-handoff-builder

Usage:
  node src/cli.js --state-dir <path> [--format json|markdown] [--patch PATCH-001] [--include-pending-review]

Options:
  --state-dir               path to .t3 or repo root
  --format                  output format, default json
  --patch                   filter to one patch id
  --include-pending-review  include pending-review patches in the queue
  --help                    show this help text
`);
}

function parseArgs(argv) {
  const options = {
    format: 'json',
    includePendingReview: false
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '--help') {
      options.help = true;
    } else if (arg === '--state-dir') {
      options.stateDir = argv[index + 1];
      index += 1;
    } else if (arg === '--format') {
      options.format = argv[index + 1];
      index += 1;
    } else if (arg === '--patch') {
      options.patchId = argv[index + 1];
      index += 1;
    } else if (arg === '--include-pending-review') {
      options.includePendingReview = true;
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

  const packet = buildHandoffPacket(options.stateDir, options);
  if (options.format === 'markdown') {
    process.stdout.write(renderMarkdown(packet));
  } else {
    process.stdout.write(`${JSON.stringify(packet, null, 2)}\n`);
  }
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exit(1);
}
