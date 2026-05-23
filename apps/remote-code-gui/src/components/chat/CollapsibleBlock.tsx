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
  iconColor = 'text-rc-text-tertiary',
  className,
  contentClassName,
}: CollapsibleBlockProps) {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  return (
    <div className={cn('overflow-hidden rounded-lg border border-rc-border-secondary bg-rc-bg-secondary', className)}>
      <button
        type="button"
        className="flex w-full items-center gap-2 px-3 py-2.5 text-left transition-colors hover:bg-rc-bg-hover"
        onClick={() => setIsOpen((state) => !state)}
      >
        <ChevronRight
          size={14}
          className={cn(iconColor, 'shrink-0 transition-transform', isOpen && 'rotate-90')}
        />
        <div className="min-w-0 flex-1">{summary}</div>
      </button>

      {isOpen && (
        <div className={cn('border-t border-rc-border-secondary px-3 pb-3 pt-2', contentClassName)}>{children}</div>
      )}
    </div>
  );
}
