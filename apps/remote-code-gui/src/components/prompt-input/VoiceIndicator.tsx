import { Mic, MicOff } from 'lucide-react';
import { cn } from '../../lib/utils';

/** VoiceIndicator 组件属性 */
export interface VoiceIndicatorProps {
  /** 是否正在监听 */
  isListening: boolean;
  /** 是否支持语音 */
  isSupported: boolean;
  /** 切换监听回调 */
  onToggle: () => void;
  /** 额外 CSS 类名 */
  className?: string;
}

/**
 * 语音输入指示器按钮。
 * isSupported=false 时返回 null，isListening 时显示红色脉冲动画。
 */
export function VoiceIndicator({
  isListening,
  isSupported,
  onToggle,
  className,
}: VoiceIndicatorProps) {
  if (!isSupported) return null;

  return (
    <button
      type="button"
      onClick={onToggle}
      className={cn(
        'relative rounded-md p-1.5 transition-colors',
        isListening
          ? 'text-red-500 hover:text-red-600'
          : 'text-slate-400 hover:bg-slate-100 hover:text-slate-600 dark:hover:bg-slate-700 dark:hover:text-slate-300',
        className,
      )}
      data-testid="voice-indicator"
      aria-label={isListening ? '停止语音输入' : '开始语音输入'}
    >
      {isListening && (
        <span className="absolute inset-0 animate-ping rounded-md bg-red-200 opacity-50 dark:bg-red-800" />
      )}
      {isListening ? (
        <Mic className="relative h-4 w-4" />
      ) : (
        <MicOff className="h-4 w-4" />
      )}
    </button>
  );
}
