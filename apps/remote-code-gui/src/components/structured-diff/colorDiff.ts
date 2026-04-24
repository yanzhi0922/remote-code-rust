export interface DiffColorConfig {
  added: string;
  deleted: string;
  context: string;
  hunkHeader: string;
  addedBg: string;
  deletedBg: string;
}

export const DEFAULT_DIFF_COLORS: DiffColorConfig = {
  added: 'text-green-700',
  deleted: 'text-red-700',
  context: 'text-slate-700',
  hunkHeader: 'text-blue-600',
  addedBg: 'bg-green-50',
  deletedBg: 'bg-red-50',
};

export function getDiffLineColors(type: 'add' | 'delete' | 'context' | 'hunk_header'): Pick<DiffColorConfig, 'added' | 'deleted' | 'context' | 'hunkHeader'> & { bg: string } {
  switch (type) {
    case 'add':
      return { added: DEFAULT_DIFF_COLORS.added, deleted: '', context: '', hunkHeader: '', bg: DEFAULT_DIFF_COLORS.addedBg };
    case 'delete':
      return { added: '', deleted: DEFAULT_DIFF_COLORS.deleted, context: '', hunkHeader: '', bg: DEFAULT_DIFF_COLORS.deletedBg };
    case 'context':
      return { added: '', deleted: '', context: DEFAULT_DIFF_COLORS.context, hunkHeader: '', bg: '' };
    case 'hunk_header':
      return { added: '', deleted: '', context: '', hunkHeader: DEFAULT_DIFF_COLORS.hunkHeader, bg: '' };
  }
}

export function getDiffLineColor(type: 'add' | 'delete' | 'context' | 'hunk_header'): string {
  switch (type) {
    case 'add':
      return DEFAULT_DIFF_COLORS.added;
    case 'delete':
      return DEFAULT_DIFF_COLORS.deleted;
    case 'context':
      return DEFAULT_DIFF_COLORS.context;
    case 'hunk_header':
      return DEFAULT_DIFF_COLORS.hunkHeader;
  }
}

export function getDiffBgColor(type: 'add' | 'delete' | 'context' | 'hunk_header'): string {
  switch (type) {
    case 'add':
      return DEFAULT_DIFF_COLORS.addedBg;
    case 'delete':
      return DEFAULT_DIFF_COLORS.deletedBg;
    case 'context':
    case 'hunk_header':
      return '';
  }
}
