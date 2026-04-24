import { X, Info, AlertTriangle, CheckCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface Notification {
  id: string;
  type: 'info' | 'warning' | 'success';
  message: string;
}

export interface NotificationsProps {
  notifications: Notification[];
  onDismiss?: (id: string) => void;
}

const TYPE_CONFIG = {
  info: { icon: Info, color: 'text-blue-600 bg-blue-50 border-blue-200' },
  warning: { icon: AlertTriangle, color: 'text-amber-600 bg-amber-50 border-amber-200' },
  success: { icon: CheckCircle, color: 'text-green-600 bg-green-50 border-green-200' },
};

export function Notifications({ notifications, onDismiss }: NotificationsProps) {
  if (notifications.length === 0) return null;

  return (
    <div data-testid="notifications" className="space-y-1">
      {notifications.map((notification) => {
        const config = TYPE_CONFIG[notification.type];
        const Icon = config.icon;
        return (
          <div
            key={notification.id}
            data-testid={`notification-${notification.id}`}
            className={cn('flex items-center gap-2 rounded border px-3 py-2', config.color)}
          >
            <Icon className="h-4 w-4 shrink-0" />
            <span className="flex-1 text-sm">{notification.message}</span>
            {onDismiss && (
              <button
                type="button"
                className="rounded p-0.5 hover:bg-black/5"
                onClick={() => onDismiss(notification.id)}
                title="关闭"
              >
                <X className="h-3.5 w-3.5" />
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}
