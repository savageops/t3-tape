import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export interface PlatformTarget {
  nodePlatform: string;
  nodeArch: string;
  targetTriple: string;
  packageName: string;
  packageDir: string;
  binaryName: string;
  binaryPath: string;
}

let cachedTargets: PlatformTarget[] | null = null;

export function platformTargetsPath(metaUrl: string = import.meta.url): string {
  return path.resolve(path.dirname(fileURLToPath(metaUrl)), '..', 'platform-targets.json');
}

export function loadPlatformTargets(metaUrl: string = import.meta.url): PlatformTarget[] {
  if (metaUrl === import.meta.url && cachedTargets) {
    return cachedTargets;
  }

  const targets = JSON.parse(
    fs.readFileSync(platformTargetsPath(metaUrl), 'utf8')
  ) as PlatformTarget[];

  if (metaUrl === import.meta.url) {
    cachedTargets = targets;
  }

  return targets;
}

export function resolveTargetPackage(
  nodePlatform: string = process.platform,
  nodeArch: string = process.arch,
  metaUrl: string = import.meta.url
): PlatformTarget {
  const target = loadPlatformTargets(metaUrl).find(
    (candidate) =>
      candidate.nodePlatform === nodePlatform && candidate.nodeArch === nodeArch
  );

  if (!target) {
    const supported = loadPlatformTargets(metaUrl)
      .map((candidate) => `${candidate.nodePlatform}/${candidate.nodeArch}`)
      .join(', ');
    throw new Error(
      `Unsupported platform/arch ${nodePlatform}/${nodeArch}. Supported targets: ${supported}.`
    );
  }

  return target;
}