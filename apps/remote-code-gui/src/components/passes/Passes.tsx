import { CreditCard, Check, X, Clock } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface PassInfo {
  id: string;
  name: string;
  status: 'active' | 'expired' | 'cancelled';
  expiresAt: string | null;
  features: string[];
}

export interface PassesProps {
  passes: PassInfo[];
  onSubscribe?: () => void;
  onCancel?: (passId: string) => void;
}

export function Passes({ passes, onSubscribe, onCancel }: PassesProps) {
  return (
    <div data-testid="passes-panel" className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <CreditCard className="h-5 w-5 text-indigo-600" />
          <h2 className="text-sm font-semibold text-slate-800">订阅管理</h2>
        </div>
        {onSubscribe && (
          <button
            type="button"
            data-testid="passes-subscribe"
            className="rounded bg-indigo-600 px-3 py-1.5 text-sm text-white hover:bg-indigo-700"
            onClick={onSubscribe}
          >
            订阅
          </button>
        )}
      </div>

      {passes.length === 0 ? (
        <div data-testid="passes-empty" className="py-8 text-center text-sm text-slate-400">
          暂无订阅
        </div>
      ) : (
        <div className="space-y-2">
          {passes.map((pass) => (
            <div
              key={pass.id}
              data-testid={`pass-item-${pass.id}`}
              className="rounded-lg border border-slate-200 p-3"
            >
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium text-slate-700">{pass.name}</span>
                <span
                  className={cn(
                    'inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium',
                    pass.status === 'active'
                      ? 'bg-green-100 text-green-700'
                      : pass.status === 'expired'
                        ? 'bg-red-100 text-red-700'
                        : 'bg-slate-100 text-slate-600',
                  )}
                >
                  {pass.status === 'active' && <Check className="h-3 w-3" />}
                  {pass.status === 'expired' && <X className="h-3 w-3" />}
                  {pass.status === 'cancelled' && <Clock className="h-3 w-3" />}
                  {pass.status === 'active' ? '活跃' : pass.status === 'expired' ? '已过期' : '已取消'}
                </span>
              </div>
              {pass.expiresAt && (
                <p className="mt-1 text-xs text-slate-500">到期: {pass.expiresAt}</p>
              )}
              {pass.features.length > 0 && (
                <div className="mt-2 flex flex-wrap gap-1">
                  {pass.features.map((feature) => (
                    <span
                      key={feature}
                      className="rounded bg-slate-100 px-1.5 py-0.5 text-xs text-slate-600"
                    >
                      {feature}
                    </span>
                  ))}
                </div>
              )}
              {pass.status === 'active' && onCancel && (
                <button
                  type="button"
                  data-testid={`pass-cancel-${pass.id}`}
                  className="mt-2 text-xs text-red-500 hover:text-red-700"
                  onClick={() => onCancel(pass.id)}
                >
                  取消订阅
                </button>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
