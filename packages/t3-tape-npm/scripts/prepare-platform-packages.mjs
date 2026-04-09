import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const packageRoot = path.resolve(scriptDir, '..');
const repoRoot = path.resolve(packageRoot, '..', '..');
const targets = JSON.parse(
  await fs.readFile(path.join(packageRoot, 'platform-targets.json'), 'utf8')
);

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith('--')) {
      continue;
    }

    parsed[arg.slice(2)] = argv[index + 1];
    index += 1;
  }

  return parsed;
}

async function findBinary(startDir, target) {
  const entries = await fs.readdir(startDir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(startDir, entry.name);
    if (entry.isDirectory()) {
      const nested = await findBinary(fullPath, target);
      if (nested) {
        return nested;
      }
      continue;
    }

    if (
      entry.isFile() &&
      entry.name === target.binaryName &&
      fullPath.includes(target.targetTriple)
    ) {
      return fullPath;
    }
  }

  return null;
}

async function writeJson(filePath, value) {
  await fs.writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

const args = parseArgs(process.argv.slice(2));
const artifactsDir = path.resolve(args['artifacts-dir'] ?? path.join(repoRoot, 'dist'));
const version =
  args.version ?? JSON.parse(await fs.readFile(path.join(packageRoot, 'package.json'), 'utf8')).version;

const launcherManifestPath = path.join(packageRoot, 'package.json');
const launcherManifest = JSON.parse(await fs.readFile(launcherManifestPath, 'utf8'));
launcherManifest.version = version;
for (const target of targets) {
  launcherManifest.optionalDependencies[target.packageName] = version;
}
await writeJson(launcherManifestPath, launcherManifest);

for (const target of targets) {
  const targetDir = path.resolve(packageRoot, target.packageDir);
  const targetManifestPath = path.join(targetDir, 'package.json');
  const targetManifest = JSON.parse(await fs.readFile(targetManifestPath, 'utf8'));
  targetManifest.version = version;
  targetManifest['t3-tape'] = {
    targetTriple: target.targetTriple,
    binaryPath: target.binaryPath
  };
  await writeJson(targetManifestPath, targetManifest);

  const sourceBinary = await findBinary(artifactsDir, target);
  if (!sourceBinary) {
    throw new Error(
      `Unable to find ${target.binaryName} for ${target.targetTriple} under ${artifactsDir}`
    );
  }

  const destinationBinary = path.join(targetDir, target.binaryPath);
  await fs.mkdir(path.dirname(destinationBinary), { recursive: true });
  await fs.copyFile(sourceBinary, destinationBinary);
  if (process.platform !== 'win32') {
    await fs.chmod(destinationBinary, 0o755);
  }

  console.log(`prepared ${target.packageName} -> ${destinationBinary}`);
}
