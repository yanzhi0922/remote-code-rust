import { ChevronRight } from 'lucide-react';
import { useState } from 'react';
import { cn } from '../../lib/utils';

interface CollapsibleBlockProps {
  summary: React.ReactNode;
  children: React.ReactNode;
  defaultOpen?: boolean;
  iconColor?: string;
  className?: string;
  contentClassName?: string;
}

export default function CollapsibleBlock({
  summary,
  children,
  defaultOpen = false,
  iconColor = 'text-slate-400',
  className,
  contentClassName,
}: CollapsibleBlockProps) {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  return (
    <div className={cn('overflow-hidden rounded-2xl border border-[#e7e1d6] bg-[#fcfbf8]', className)}>
      <button
        type="button"
        className="flex w-full items-center gap-2 px-3 py-2.5 text-left transition-colors hover:bg-[#f7f3eb]"
        onClick={() => setIsOpen((state) => !state)}
      >
        <ChevronRight
          size={14}
          className={cn(iconColor, 'shrink-0 transition-transform', isOpen && 'rotate-90')}
        />
        <div className="min-w-0 flex-1">{summary}</div>
      </button>

      {isOpen && (
        <div className={cn('border-t border-[#ece5d9] px-3 pb-3 pt-2', contentClassName)}>{children}</div>
      )}
    </div>
  );
}
