import { describe, expect, it } from 'vitest';
import { getDiffLineColor, getDiffBgColor, DEFAULT_DIFF_COLORS } from './colorDiff';

describe('colorDiff', () => {
  it('returns correct text color for add', () => {
    expect(getDiffLineColor('add')).toBe(DEFAULT_DIFF_COLORS.added);
  });

  it('returns correct text color for delete', () => {
    expect(getDiffLineColor('delete')).toBe(DEFAULT_DIFF_COLORS.deleted);
  });

  it('returns correct text color for context', () => {
    expect(getDiffLineColor('context')).toBe(DEFAULT_DIFF_COLORS.context);
  });

  it('returns correct text color for hunk_header', () => {
    expect(getDiffLineColor('hunk_header')).toBe(DEFAULT_DIFF_COLORS.hunkHeader);
  });

  it('returns correct bg color for add', () => {
    expect(getDiffBgColor('add')).toBe(DEFAULT_DIFF_COLORS.addedBg);
  });

  it('returns correct bg color for delete', () => {
    expect(getDiffBgColor('delete')).toBe(DEFAULT_DIFF_COLORS.deletedBg);
  });

  it('returns empty bg for context', () => {
    expect(getDiffBgColor('context')).toBe('');
  });

  it('returns empty bg for hunk_header', () => {
    expect(getDiffBgColor('hunk_header')).toBe('');
  });
});
