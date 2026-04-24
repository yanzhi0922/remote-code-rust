export interface DiffStatsProps {
  additions: number;
  deletions: number;
  filesChanged: number;
}

export function DiffStats({ additions, deletions, filesChanged }: DiffStatsProps) {
  const total = additions + deletions;
  const addPercent = total > 0 ? (additions / total) * 100 : 50;

  return (
    <div data-testid="diff-stats" className="flex items-center gap-3 text-xs">
      <span className="font-medium text-green-600">+{additions}</span>
      <span className="font-medium text-red-600">-{deletions}</span>
      <span className="text-slate-500">{filesChanged} 个文件</span>
      <div className="h-1.5 w-16 overflow-hidden rounded-full bg-slate-200">
        <div
          className="flex h-full"
          data-testid="diff-stats-bar"
        >
          <div
            className="h-full bg-green-500"
            style={{ width: `${addPercent}%` }}
          />
          <div
            className="h-full bg-red-500"
            style={{ width: `${100 - addPercent}%` }}
          />
        </div>
      </div>
    </div>
  );
}
