import { useState } from 'react';
import { Monitor, X, ArrowRight } from 'lucide-react';

export interface DesktopUpsellStartupProps {
  onDone: () => void;
  onTryDesktop?: () => void;
}

export function DesktopUpsellStartup({ onDone, onTryDesktop }: DesktopUpsellStartupProps) {
  const [visible, setVisible] = useState(true);

  if (!visible) return null;

  function handleTry() {
    setVisible(false);
    if (onTryDesktop) {
      onTryDesktop();
    } else {
      onDone();
    }
  }

  function handleNotNow() {
    setVisible(false);
    onDone();
  }

  function handleNever() {
    setVisible(false);
    onDone();
  }

  return (
    <div data-testid="desktop-upsell-startup" className="rounded-lg border border-purple-200 bg-purple-50 p-6">
      <div className="mb-4 flex items-start justify-between">
        <div className="flex items-center gap-3">
          <Monitor className="h-6 w-6 text-purple-600" />
          <h3 className="text-lg font-semibold text-purple-900">试试桌面版</h3>
        </div>
        <button
          type="button"
          data-testid="desktop-upsell-close"
          className="rounded p-1 hover:bg-purple-100"
          onClick={handleNotNow}
          title="关闭"
        >
          <X className="h-4 w-4 text-purple-400" />
        </button>
      </div>
      <p className="mb-4 text-sm text-purple-700">
        桌面版提供更好的性能和系统集成，包括原生通知、系统托盘和更快的启动速度。
      </p>
      <div className="flex gap-3">
        <button
          type="button"
          data-testid="desktop-upsell-try"
          className="inline-flex items-center gap-1.5 rounded bg-purple-600 px-4 py-2 text-sm font-medium text-white hover:bg-purple-700"
          onClick={handleTry}
        >
          试试桌面版
          <ArrowRight className="h-4 w-4" />
        </button>
        <button
          type="button"
          data-testid="desktop-upsell-not-now"
          className="rounded border border-purple-300 px-4 py-2 text-sm text-purple-700 hover:bg-purple-100"
          onClick={handleNotNow}
        >
          暂不
        </button>
        <button
          type="button"
          data-testid="desktop-upsell-never"
          className="text-sm text-purple-500 hover:text-purple-700"
          onClick={handleNever}
        >
          不再提示
        </button>
      </div>
    </div>
  );
}
