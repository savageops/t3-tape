#!/usr/bin/env node

import fs from 'node:fs';
import { parseArgs } from 'node:util';
import { pathToFileURL } from 'node:url';

import { buildReleaseSummary, renderMarkdown } from './index.js';

function helpText() {
  return `release-note-router

Usage:
  release-note-router --input <path> [--format json|markdown] [--version <value>]

Options:
  --input    Path to a JSON array or newline-delimited commit list
  --format   json or markdown
  --version  Optional version label for markdown output
  --help     Show this message
`;
}

function readEntries(filePath) {
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
      input: { type: 'string' },
      format: { type: 'string' },
      version: { type: 'string' },
      help: { type: 'boolean' }
    },
    allowPositionals: false
  });

  if (values.help) {
    io.stdout.write(helpText());
    return 0;
  }

  if (!values.input) {
    io.stderr.write('Missing required option: --input\n');
    return 1;
  }

  try {
    const summary = buildReleaseSummary(readEntries(values.input), {
      version: values.version
    });
    const format = values.format ?? 'json';

    if (format === 'markdown') {
      io.stdout.write(renderMarkdown(summary));
    } else if (format === 'json') {
      io.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
    } else {
      io.stderr.write(`Unsupported format: ${format}\n`);
      return 1;
    }

    return 0;
  } catch (error) {
    io.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    return 1;
  }
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = run();
}
