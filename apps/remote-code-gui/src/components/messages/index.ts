/**
 * 消息子类型组件的统一导出。
 *
 * 包含所有用户消息变体：文本、工具结果、计划、提示、团队成员、资源更新等。
 * 包含助手消息变体：文本、思考、工具使用、编辑思考。
 * 包含系统消息变体：文本、API 错误、压缩边界、速率限制。
 * 包含特殊消息：关机、计划审批、Hook 进度、任务分配。
 * 包含辅助组件：分组工具使用、模型徽章、Markdown 渲染、虚拟列表。
 */

// ─── User 消息 ───────────────────────────────────────────

export { UserTextMessage } from './UserTextMessage';
export type { UserTextMessageProps } from './UserTextMessage';

export {
  UserToolSuccessMessage,
  UserToolErrorMessage,
  UserToolRejectMessage,
  UserToolCanceledMessage,
  RejectedToolUseMessage,
  RejectedPlanMessage,
  UserToolResultMessage,
} from './UserToolResultMessage';
export type {
  UserToolSuccessMessageProps,
  UserToolErrorMessageProps,
  UserToolRejectMessageProps,
  UserToolCanceledMessageProps,
  RejectedToolUseMessageProps,
  RejectedPlanMessageProps,
  UserToolResultMessageProps,
} from './UserToolResultMessage';

export { UserPlanMessage } from './UserPlanMessage';
export type { UserPlanMessageProps } from './UserPlanMessage';

export { UserPromptMessage } from './UserPromptMessage';
export type { UserPromptMessageProps, ContextSuggestion } from './UserPromptMessage';

export { UserTeammateMessage } from './UserTeammateMessage';
export type { UserTeammateMessageProps } from './UserTeammateMessage';

export { UserResourceUpdateMessage } from './UserResourceUpdateMessage';
export type {
  UserResourceUpdateMessageProps,
  ResourceUpdateKind,
} from './UserResourceUpdateMessage';

export { UserBashInputMessage } from './UserBashInputMessage';
export type { UserBashInputMessageProps } from './UserBashInputMessage';

export { UserBashOutputMessage } from './UserBashOutputMessage';
export type { UserBashOutputMessageProps } from './UserBashOutputMessage';

export { UserCommandMessage } from './UserCommandMessage';
export type { UserCommandMessageProps } from './UserCommandMessage';

export { UserImageMessage } from './UserImageMessage';
export type { UserImageMessageProps } from './UserImageMessage';

// ─── Assistant 消息 ──────────────────────────────────────

export { AssistantTextMessage } from './AssistantTextMessage';
export type { AssistantTextMessageProps } from './AssistantTextMessage';

export { AssistantThinkingMessage } from './AssistantThinkingMessage';
export type { AssistantThinkingMessageProps } from './AssistantThinkingMessage';

export { AssistantToolUseMessage } from './AssistantToolUseMessage';
export type { AssistantToolUseMessageProps } from './AssistantToolUseMessage';

export { AssistantRedactedThinkingMessage } from './AssistantRedactedThinkingMessage';
export type { AssistantRedactedThinkingMessageProps } from './AssistantRedactedThinkingMessage';

// ─── System 消息 ─────────────────────────────────────────

export { SystemTextMessage } from './SystemTextMessage';
export type { SystemTextMessageProps } from './SystemTextMessage';

export { SystemAPIErrorMessage } from './SystemAPIErrorMessage';
export type { SystemAPIErrorMessageProps } from './SystemAPIErrorMessage';

export { CompactBoundaryMessage } from './CompactBoundaryMessage';
export type { CompactBoundaryMessageProps } from './CompactBoundaryMessage';

export { RateLimitMessage } from './RateLimitMessage';
export type { RateLimitMessageProps } from './RateLimitMessage';

// ─── 特殊消息 ────────────────────────────────────────────

export { ShutdownMessage } from './ShutdownMessage';
export type { ShutdownMessageProps } from './ShutdownMessage';

export { PlanApprovalMessage } from './PlanApprovalMessage';
export type { PlanApprovalMessageProps } from './PlanApprovalMessage';

export { HookProgressMessage } from './HookProgressMessage';
export type { HookProgressMessageProps } from './HookProgressMessage';

export { TaskAssignmentMessage } from './TaskAssignmentMessage';
export type { TaskAssignmentMessageProps } from './TaskAssignmentMessage';

// ─── 辅助组件 ────────────────────────────────────────────

export { GroupedToolUseContent } from './GroupedToolUseContent';
export type { GroupedToolUseContentProps } from './GroupedToolUseContent';

export { MessageModel } from './MessageModel';
export type { MessageModelProps } from './MessageModel';

export { Markdown } from './Markdown';
export type { MarkdownProps } from './Markdown';

export { VirtualMessageList } from './VirtualMessageList';
export type { VirtualMessageListProps } from './VirtualMessageList';
