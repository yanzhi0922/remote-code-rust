import { Hand } from 'lucide-react';

export function InterruptedByUser() {
  return (
    <div data-testid="interrupted-by-user" className="flex items-center gap-1.5 text-slate-500">
      <Hand className="h-4 w-4" />
      <span className="text-sm">已被用户中断</span>
    </div>
  );
}
