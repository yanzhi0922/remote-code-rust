import type {
  ArtifactItemVm,
  ApprovalItemVm,
  ComposerVm,
  SessionBundleVm,
  SessionConnectionVm,
  SessionSummaryVm,
  TaskNodeVm,
  TimelineItemVm,
} from './contracts';

export function sortTimelineItems(items: TimelineItemVm[]): TimelineItemVm[] {
  return [...items].sort((left, right) => left.order - right.order);
}

export function sortTaskNodes(tasks: TaskNodeVm[]): TaskNodeVm[] {
  return [...tasks].sort((left, right) => {
    const leftUpdated = left.updatedAt ?? left.createdAt ?? '';
    const rightUpdated = right.updatedAt ?? right.createdAt ?? '';
    return rightUpdated.localeCompare(leftUpdated);
  });
}

export function buildSessionBundleVm(input: {
  session: SessionSummaryVm | null;
  timeline: TimelineItemVm[];
  approvals?: ApprovalItemVm[];
  artifacts?: ArtifactItemVm[];
  taskTree?: TaskNodeVm[];
  connection: SessionConnectionVm;
  composer?: Partial<ComposerVm>;
  latestCursor?: number | null;
}): SessionBundleVm {
  return {
    session: input.session,
    timeline: sortTimelineItems(input.timeline),
    approvals: input.approvals ?? [],
    artifacts: input.artifacts ?? [],
    taskTree: sortTaskNodes(input.taskTree ?? []),
    connection: input.connection,
    composer: {
      value: input.composer?.value ?? '',
      disabled: input.composer?.disabled ?? false,
      busy: input.composer?.busy ?? false,
      placeholder: input.composer?.placeholder ?? null,
    },
    latestCursor: input.latestCursor ?? null,
  };
}
