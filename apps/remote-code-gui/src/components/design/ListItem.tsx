import { cn } from '../../lib/utils';

export interface ListItemProps {
  label: string;
  description?: string;
  selected?: boolean;
  onClick?: () => void;
  icon?: React.ReactNode;
  className?: string;
}

export function ListItem({ label, description, selected = false, onClick, icon, className }: ListItemProps) {
  const Tag = onClick ? 'button' : 'div';

  return (
    <Tag
      type={onClick ? 'button' : undefined}
      onClick={onClick}
      data-testid="list-item"
      className={cn(
        'flex w-full items-center gap-3 rounded-lg border px-4 py-3 text-left transition-all',
        selected
          ? 'border-blue-500 bg-blue-50 ring-2 ring-blue-200'
          : 'border-slate-200 bg-white hover:border-slate-300',
        onClick && 'cursor-pointer',
        className
      )}
    >
      {icon && (
        <span data-testid="list-item-icon" className="shrink-0 text-slate-500">
          {icon}
        </span>
      )}
      <div className="min-w-0 flex-1">
        <span
          data-testid="list-item-label"
          className={cn(
            'block text-sm font-medium',
            selected ? 'text-blue-700' : 'text-slate-700'
          )}
        >
          {label}
        </span>
        {description && (
          <span data-testid="list-item-description" className="block text-xs text-slate-500">
            {description}
          </span>
        )}
      </div>
      {selected && (
        <span data-testid="list-item-selected" className="shrink-0 text-blue-600">✓</span>
      )}
    </Tag>
  );
}
