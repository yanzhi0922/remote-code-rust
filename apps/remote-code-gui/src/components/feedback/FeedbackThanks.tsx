/**
 * FeedbackThanks — 反馈感谢组件。
 *
 * 显示用户选择的评价和评论（如果有），附带关闭按钮。
 */

import { ThumbsUp, ThumbsDown, Bug } from 'lucide-react';

export interface FeedbackThanksProps {
  rating: string;
  comment?: string;
  onClose: () => void;
}

const ratingLabels: Record<string, { icon: React.ReactNode; label: string }> = {
  thumbs_up: { icon: <ThumbsUp className="h-6 w-6 text-green-500" />, label: '👍 赞' },
  thumbs_down: { icon: <ThumbsDown className="h-6 w-6 text-orange-500" />, label: '👎 踩' },
  bug: { icon: <Bug className="h-6 w-6 text-red-500" />, label: '🐛 Bug' },
};

export function FeedbackThanks({ rating, comment, onClose }: FeedbackThanksProps) {
  const info = ratingLabels[rating] ?? { icon: null, label: rating };

  return (
    <div className="rounded-2xl bg-white p-6 shadow-xl" data-testid="feedback-thanks">
      <h3 className="text-lg font-semibold text-slate-900">感谢您的反馈！</h3>

      <div className="mt-4 flex items-center gap-3 rounded-xl bg-slate-50 px-4 py-3" data-testid="feedback-rating-display">
        {info.icon}
        <span className="text-sm font-medium text-slate-700">{info.label}</span>
      </div>

      {comment && (
        <div className="mt-3 rounded-xl bg-slate-50 px-4 py-3" data-testid="feedback-comment-display">
          <p className="text-sm text-slate-600">{comment}</p>
        </div>
      )}

      <button
        onClick={onClose}
        className="mt-6 w-full rounded-xl bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-800"
        data-testid="feedback-thanks-close"
      >
        关闭
      </button>
    </div>
  );
}
