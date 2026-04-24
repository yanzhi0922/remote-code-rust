import { useState } from 'react';
import { Star, Send } from 'lucide-react';

export interface SkillImprovementSurveyProps {
  skillName: string;
  onSubmit?: (rating: number, feedback: string) => void;
  onDismiss?: () => void;
}

export function SkillImprovementSurvey({ skillName, onSubmit, onDismiss }: SkillImprovementSurveyProps) {
  const [rating, setRating] = useState(0);
  const [feedback, setFeedback] = useState('');
  const [submitted, setSubmitted] = useState(false);

  if (submitted) {
    return (
      <div data-testid="skill-survey-thanks" className="rounded-lg border border-green-200 bg-green-50 p-4 text-center">
        <p className="text-sm text-green-700">感谢您的反馈！</p>
      </div>
    );
  }

  function handleSubmit() {
    onSubmit?.(rating, feedback);
    setSubmitted(true);
  }

  return (
    <div data-testid="skill-improvement-survey" className="rounded-lg border border-slate-200 bg-white p-4">
      <div className="mb-3 flex items-center justify-between">
        <h3 className="text-sm font-semibold text-slate-800">技能改进调查</h3>
        {onDismiss && (
          <button type="button" className="text-xs text-slate-400 hover:text-slate-600" onClick={onDismiss}>
            关闭
          </button>
        )}
      </div>
      <p className="mb-3 text-sm text-slate-600">
        您对 <strong>{skillName}</strong> 的体验如何？
      </p>
      <div className="mb-3 flex gap-1">
        {[1, 2, 3, 4, 5].map((star) => (
          <button
            key={star}
            type="button"
            data-testid={`skill-survey-star-${star}`}
            className="p-0.5"
            onClick={() => setRating(star)}
            title={`${star} 星`}
          >
            <Star className={`h-5 w-5 ${star <= rating ? 'fill-yellow-400 text-yellow-400' : 'text-slate-300'}`} />
          </button>
        ))}
      </div>
      <textarea
        data-testid="skill-survey-feedback"
        className="mb-3 w-full rounded border border-slate-200 p-2 text-sm"
        placeholder="请输入改进建议..."
        value={feedback}
        onChange={(e) => setFeedback(e.target.value)}
        rows={3}
      />
      <button
        type="button"
        data-testid="skill-survey-submit"
        className="inline-flex items-center gap-1.5 rounded bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-700 disabled:opacity-50"
        onClick={handleSubmit}
        disabled={rating === 0}
      >
        <Send className="h-3.5 w-3.5" />
        提交
      </button>
    </div>
  );
}
