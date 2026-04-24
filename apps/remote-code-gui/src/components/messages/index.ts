/**
 * 消息子类型组件的统一导出。
 *
 * 包含所有用户消息变体：文本、工具结果、计划、提示、团队成员、资源更新等。
 */

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
