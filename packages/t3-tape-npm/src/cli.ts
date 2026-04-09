#!/usr/bin/env node

import { spawn } from 'node:child_process';
import { pathToFileURL } from 'node:url';

import { resolveBinaryPath } from './resolve.js';

export async function run(argv: string[] = process.argv.slice(2)): Promise<number> {
  const resolved = resolveBinaryPath();

  return await new Promise<number>((resolve, reject) => {
    const child = spawn(resolved.binaryPath, argv, {
      env: process.env,
      stdio: 'inherit',
      windowsHide: true
    });

    child.on('error', reject);
    child.on('exit', (code, signal) => {
      if (signal) {
        resolve(1);
        return;
      }

      resolve(code ?? 1);
    });
  });
}

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

async function main(): Promise<void> {
  try {
    process.exitCode = await run();
  } catch (error) {
    console.error(formatError(error));
    process.exitCode = 1;
  }
}

const invokedAsCli =
  typeof process.argv[1] === 'string' &&
  import.meta.url === pathToFileURL(process.argv[1]).href;

if (invokedAsCli) {
  await main();
}