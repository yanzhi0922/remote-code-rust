import type {
  ApprovalItemVm,
  SessionBundleVm,
  SessionSummaryVm,
  TimelineItemVm,
} from './contracts';

export interface SessionSubscriptionCallbacks {
  onOpen?: () => void;
  onTimeline?: (item: TimelineItemVm) => void;
  onBundleInvalidated?: () => void;
  onError?: (error: unknown) => void;
  onClose?: () => void;
}

export interface SessionSubscriptionHandle {
  close(): void;
}

export interface CommandAckVm {
  sessionId: string;
  accepted: boolean;
  message: string;
}

export type ApprovalDecisionVm = 'approved' | 'denied' | 'cancelled';

export interface SessionTransport {
  listSessions(): Promise<SessionSummaryVm[]>;
  loadSessionBundle(sessionId: string): Promise<SessionBundleVm>;
  subscribeSession(
    sessionId: string,
    afterCursor: number | null,
    callbacks: SessionSubscriptionCallbacks,
  ): SessionSubscriptionHandle;
  sendPrompt(sessionId: string, content: string): Promise<CommandAckVm>;
  interrupt(sessionId: string): Promise<CommandAckVm>;
  resolveApproval(
    approvalId: string,
    decision: ApprovalDecisionVm,
    note?: string,
  ): Promise<ApprovalItemVm | void>;
}
