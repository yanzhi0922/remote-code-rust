/**
 * FeedbackDialog — 用户反馈对话框。
 *
 * 模态对话框，包含 3 个评价按钮（赞、踩、Bug）和可选评论输入框。
 * 提交后显示感谢消息。
 */

import { useState } from 'react';
import { ThumbsUp, ThumbsDown, Bug, X } from 'lucide-react';

type Rating = 'thumbs_up' | 'thumbs_down' | 'bug';

export interface FeedbackDialogProps {
  visible: boolean;
  onSubmit: (rating: Rating, comment?: string) => void;
  onClose: () => void;
}

export function FeedbackDialog({ visible, onSubmit, onClose }: FeedbackDialogProps) {
  const [selected, setSelected] = useState<Rating | null>(null);
  const [comment, setComment] = useState('');
  const [submitted, setSubmitted] = useState(false);

  if (!visible) return null;

  const handleSubmit = () => {
    if (!selected) return;
    onSubmit(selected, comment || undefined);
    setSubmitted(true);
  };

  const handleClose = () => {
    setSelected(null);
    setComment('');
    setSubmitted(false);
    onClose();
  };

  const ratingOptions: { value: Rating; icon: React.ReactNode; label: string }[] = [
    { value: 'thumbs_up', icon: <ThumbsUp className="h-5 w-5" />, label: '👍 赞' },
    { value: 'thumbs_down', icon: <ThumbsDown className="h-5 w-5" />, label: '👎 踩' },
    { value: 'bug', icon: <Bug className="h-5 w-5" />, label: '🐛 Bug' },
  ];

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      data-testid="feedback-dialog-overlay"
    >
      <div className="w-full max-w-md rounded-2xl bg-white p-6 shadow-xl" data-testid="feedback-dialog">
        {/* Header */}
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold text-slate-900">发送反馈</h2>
          <button
            onClick={handleClose}
            className="rounded-lg p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
            data-testid="feedback-close"
            aria-label="关闭"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        {submitted ? (
          /* Thank you message */
          <div className="mt-6 text-center" data-testid="feedback-thanks">
            <p className="text-lg font-medium text-slate-900">感谢您的反馈！</p>
            <p className="mt-2 text-sm text-slate-500">您的意见对我们非常重要。</p>
            <button
              onClick={handleClose}
              className="mt-4 rounded-xl bg-slate-900 px-4 py-2 text-sm text-white hover:bg-slate-800"
            >
              关闭
            </button>
          </div>
        ) : (
          <>
            {/* Rating buttons */}
            <div className="mt-4 flex gap-3" data-testid="feedback-ratings">
              {ratingOptions.map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => setSelected(opt.value)}
                  className={`flex flex-1 flex-col items-center gap-1 rounded-xl border-2 px-3 py-3 text-sm transition-colors ${
                    selected === opt.value
                      ? 'border-blue-500 bg-blue-50 text-blue-700'
                      : 'border-slate-200 bg-slate-50 text-slate-600 hover:border-slate-300'
                  }`}
                  data-testid={`rating-${opt.value}`}
                >
                  {opt.icon}
                  {opt.label}
                </button>
              ))}
            </div>

            {/* Comment input */}
            <textarea
              className="mt-4 w-full rounded-xl border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-900 placeholder-slate-400 focus:border-blue-500 focus:outline-none"
              placeholder="添加评论（可选）..."
              rows={3}
              value={comment}
              onChange={(e) => setComment(e.target.value)}
              data-testid="feedback-comment"
            />

            {/* Submit */}
            <button
              onClick={handleSubmit}
              disabled={!selected}
              className="mt-4 w-full rounded-xl bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
              data-testid="feedback-submit"
            >
              提交反馈
            </button>
          </>
        )}
      </div>
    </div>
  );
}
