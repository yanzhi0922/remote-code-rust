import { describe, expect, it } from 'vitest';
import {
  getTaskStatusColor,
  getTaskStatusLabel,
  getTaskStatusIcon,
  formatTaskDuration,
} from './TaskStatusUtils';

describe('TaskStatusUtils', () => {
  describe('getTaskStatusColor', () => {
    it('returns green for running', () => {
      expect(getTaskStatusColor('running')).toBe('text-green-500');
    });

    it('returns green-600 for completed', () => {
      expect(getTaskStatusColor('completed')).toBe('text-green-600');
    });

    it('returns red for failed', () => {
      expect(getTaskStatusColor('failed')).toBe('text-red-500');
    });

    it('returns grey for pending', () => {
      expect(getTaskStatusColor('pending')).toBe('text-slate-400');
    });

    it('returns grey for unknown', () => {
      expect(getTaskStatusColor('unknown')).toBe('text-slate-400');
    });
  });

  describe('getTaskStatusLabel', () => {
    it('returns correct labels', () => {
      expect(getTaskStatusLabel('running')).toBe('Running');
      expect(getTaskStatusLabel('completed')).toBe('Completed');
      expect(getTaskStatusLabel('failed')).toBe('Failed');
      expect(getTaskStatusLabel('pending')).toBe('Pending');
    });

    it('returns raw status for unknown', () => {
      expect(getTaskStatusLabel('custom')).toBe('custom');
    });
  });

  describe('getTaskStatusIcon', () => {
    it('returns correct icon names', () => {
      expect(getTaskStatusIcon('running')).toBe('Loader2');
      expect(getTaskStatusIcon('completed')).toBe('CheckCircle2');
      expect(getTaskStatusIcon('failed')).toBe('XCircle');
      expect(getTaskStatusIcon('pending')).toBe('Clock');
    });

    it('returns Clock for unknown', () => {
      expect(getTaskStatusIcon('other')).toBe('Clock');
    });
  });

  describe('formatTaskDuration', () => {
    it('formats milliseconds', () => {
      expect(formatTaskDuration(500)).toBe('500ms');
    });

    it('formats seconds', () => {
      expect(formatTaskDuration(3000)).toBe('3s');
    });

    it('formats minutes and seconds', () => {
      expect(formatTaskDuration(125000)).toBe('2m 5s');
    });

    it('formats hours and minutes', () => {
      expect(formatTaskDuration(3700000)).toBe('1h 1m');
    });
  });
});
