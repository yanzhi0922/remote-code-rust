import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SkillImprovementSurvey } from './SkillImprovementSurvey';

afterEach(() => {
  cleanup();
});

describe('SkillImprovementSurvey', () => {
  it('renders survey', () => {
    render(<SkillImprovementSurvey skillName="CodeReview" />);
    expect(screen.getByTestId('skill-improvement-survey')).toBeInTheDocument();
    expect(screen.getByText(/CodeReview/)).toBeInTheDocument();
  });

  it('sets rating on star click', () => {
    render(<SkillImprovementSurvey skillName="Test" />);
    fireEvent.click(screen.getByTestId('skill-survey-star-4'));
    // Star 4 should be filled
    expect(screen.getByTestId('skill-survey-submit')).not.toBeDisabled();
  });

  it('submits and shows thanks', () => {
    const onSubmit = vi.fn();
    render(<SkillImprovementSurvey skillName="Test" onSubmit={onSubmit} />);
    fireEvent.click(screen.getByTestId('skill-survey-star-5'));
    fireEvent.change(screen.getByTestId('skill-survey-feedback'), { target: { value: 'Great!' } });
    fireEvent.click(screen.getByTestId('skill-survey-submit'));
    expect(onSubmit).toHaveBeenCalledWith(5, 'Great!');
    expect(screen.getByTestId('skill-survey-thanks')).toBeInTheDocument();
  });

  it('submit is disabled when no rating', () => {
    render(<SkillImprovementSurvey skillName="Test" />);
    expect(screen.getByTestId('skill-survey-submit')).toBeDisabled();
  });
});
