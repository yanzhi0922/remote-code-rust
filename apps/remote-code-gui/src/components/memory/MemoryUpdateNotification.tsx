import React from 'react';

export function getRelativeMemoryPath(
  path: string,
  homeDir?: string,
  cwd?: string,
): string {
  const home = homeDir ?? '';
  const currentDir = cwd ?? '';

  const relativeToHome = path.startsWith(home)
    ? '~' + path.slice(home.length)
    : null;
  const relativeToCwd = path.startsWith(currentDir)
    ? './' + path.slice(currentDir.length).replace(/^[/\\]/, '')
    : null;

  if (relativeToHome && relativeToCwd) {
    return relativeToHome.length <= relativeToCwd.length
      ? relativeToHome
      : relativeToCwd;
  }

  return relativeToHome || relativeToCwd || path;
}

type Props = {
  memoryPath: string;
  homeDir?: string;
  cwd?: string;
};

export function MemoryUpdateNotification({
  memoryPath,
  homeDir,
  cwd,
}: Props): React.ReactElement {
  const displayPath = getRelativeMemoryPath(memoryPath, homeDir, cwd);

  return (
    <div data-testid="memory-update-notification" className="flex flex-col">
      <span className="text-sm text-gray-700 dark:text-gray-300">
        Memory updated in {displayPath} · /memory to edit
      </span>
    </div>
  );
}
