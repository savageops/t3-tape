import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

import { T3_TAPE_BINARY_PATH, readBinaryOverride } from './env.js';
import { resolveTargetPackage, type PlatformTarget } from './platform.js';

export interface ResolveBinaryOptions {
  env?: NodeJS.ProcessEnv;
  platform?: string;
  arch?: string;
  packageRoot?: string;
  metaUrl?: string;
}

export interface ResolvedBinary {
  source: 'env' | 'package';
  binaryPath: string;
  packageName?: string;
  packageRoot?: string;
  target?: PlatformTarget;
}

function defaultPackageRoot(metaUrl: string): string {
  return path.resolve(path.dirname(fileURLToPath(metaUrl)), '..');
}

function packagedManifestPath(
  packageName: string,
  packageRoot: string,
  metaUrl: string,
  useExplicitRoot: boolean
): string {
  if (useExplicitRoot) {
    const manifestPath = path.join(
      packageRoot,
      'node_modules',
      ...packageName.split('/'),
      'package.json'
    );
    if (!fs.existsSync(manifestPath)) {
      throw new Error('missing-manifest');
    }
    return manifestPath;
  }

  const requireFromHere = createRequire(metaUrl);
  return requireFromHere.resolve(`${packageName}/package.json`, {
    paths: [packageRoot]
  });
}

function assertExecutableBinary(binaryPath: string, context: string): string {
  if (!fs.existsSync(binaryPath)) {
    throw new Error(`${context} does not contain the expected binary at ${binaryPath}.`);
  }

  if (process.platform !== 'win32') {
    try {
      fs.accessSync(binaryPath, fs.constants.X_OK);
    } catch (error) {
      throw new Error(
        `${context} is not executable at ${binaryPath}: ${error instanceof Error ? error.message : String(error)}`
      );
    }
  }

  return binaryPath;
}

export function resolveBinaryPath(
  options: ResolveBinaryOptions = {}
): ResolvedBinary {
  const env = options.env ?? process.env;
  const metaUrl = options.metaUrl ?? import.meta.url;
  const override = readBinaryOverride(env);

  if (override) {
    const binaryPath = assertExecutableBinary(
      path.resolve(override),
      `Environment override ${T3_TAPE_BINARY_PATH}`
    );
    return {
      source: 'env',
      binaryPath
    };
  }

  const target = resolveTargetPackage(
    options.platform ?? process.platform,
    options.arch ?? process.arch,
    metaUrl
  );
  const packageRoot = options.packageRoot ?? defaultPackageRoot(metaUrl);

  let manifestPath: string;
  try {
    manifestPath = packagedManifestPath(
      target.packageName,
      packageRoot,
      metaUrl,
      options.packageRoot !== undefined
    );
  } catch {
    throw new Error(
      `Missing packaged target ${target.packageName} for ${target.nodePlatform}/${target.nodeArch}. Likely causes: install ran with --no-optional, node_modules was copied between platforms, the target is unsupported, or the install is incomplete.`
    );
  }

  const installedPackageRoot = path.dirname(manifestPath);
  const binaryPath = assertExecutableBinary(
    path.join(installedPackageRoot, target.binaryPath),
    `Installed target package ${target.packageName}`
  );

  return {
    source: 'package',
    binaryPath,
    packageName: target.packageName,
    packageRoot: installedPackageRoot,
    target
  };
}
