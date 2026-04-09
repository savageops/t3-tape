import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export function makeTempDir(prefix) {
  return fs.mkdtempSync(path.join(os.tmpdir(), prefix));
}

export function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, JSON.stringify(value, null, 2));
}

export function writeText(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, value);
}

export function copyDir(fromPath, toPath) {
  fs.mkdirSync(path.dirname(toPath), { recursive: true });
  fs.cpSync(fromPath, toPath, { recursive: true });
}
