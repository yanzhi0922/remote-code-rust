import { memo } from 'react';
import { cn } from '../../lib/utils';

/** 消息模型组件属性 */
export interface MessageModelProps {
  /** 模型名称 */
  modelName: string;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 消息模型徽章组件。
 * 显示模型名称的小标签样式。
 */
export const MessageModel = memo(function MessageModel({
  modelName,
  className,
}: MessageModelProps) {
  return (
    <span
      data-testid="message-model"
      className={cn(
        'inline-flex items-center rounded-full bg-slate-100 px-2 py-0.5 text-[10px] font-medium leading-none text-slate-500 dark:bg-slate-800 dark:text-slate-400',
        className,
      )}
    >
      {modelName}
    </span>
  );
});
