export const T3_TAPE_BINARY_PATH = 'T3_TAPE_BINARY_PATH';

export function readBinaryOverride(
  env: NodeJS.ProcessEnv = process.env
): string | null {
  const value = env[T3_TAPE_BINARY_PATH];
  if (!value || value.trim() === '') {
    return null;
  }

  return value.trim();
}