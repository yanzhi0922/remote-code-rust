export { PermissionRequestTitle } from './PermissionRequestTitle';
export type { PermissionRequestTitleProps } from './PermissionRequestTitle';

export { PermissionExplanation } from './PermissionExplanation';
export type { PermissionExplanationProps, RiskLevel } from './PermissionExplanation';

export { PermissionDecisionDebugInfo } from './PermissionDecisionDebugInfo';
export type { PermissionDecisionDebugInfoProps } from './PermissionDecisionDebugInfo';

export { PermissionRuleDescription } from './PermissionRuleDescription';
export type { PermissionRuleDescriptionProps, PermissionBehavior } from './PermissionRuleDescription';

export { PermissionRuleInput } from './PermissionRuleInput';
export type { PermissionRuleInputProps } from './PermissionRuleInput';

export { PermissionRuleList } from './PermissionRuleList';
export type { PermissionRuleListProps, PermissionRule, TabType } from './PermissionRuleList';

export { AutoModeOptIn } from './AutoModeOptIn';
export type { AutoModeOptInProps } from './AutoModeOptIn';

export { BypassPermissions } from './BypassPermissions';
export type { BypassPermissionsProps } from './BypassPermissions';

export { FilePermissionOptions } from './FilePermissionOptions';
export type { FilePermissionOptionsProps } from './FilePermissionOptions';

export {
  BashPermissionRequest,
  FileEditPermissionRequest,
  FileWritePermissionRequest,
  McpPermissionRequest,
  GenericPermissionRequest,
} from './ToolPermissionRequests';

export { BashPermissionRequest as BashPermissionRequestV2 } from './BashPermissionRequest';
export type { BashPermissionRequestProps } from './BashPermissionRequest';

export { PowerShellPermissionRequest } from './PowerShellPermissionRequest';
export type { PowerShellPermissionRequestProps } from './PowerShellPermissionRequest';

export { FileEditPermissionRequest as FileEditPermissionRequestV2 } from './FileEditPermissionRequest';
export type { FileEditPermissionRequestProps } from './FileEditPermissionRequest';

export { FileWritePermissionRequest as FileWritePermissionRequestV2 } from './FileWritePermissionRequest';
export type { FileWritePermissionRequestProps } from './FileWritePermissionRequest';

export { FilesystemPermissionRequest } from './FilesystemPermissionRequest';
export type { FilesystemPermissionRequestProps } from './FilesystemPermissionRequest';

export { WebFetchPermissionRequest } from './WebFetchPermissionRequest';
export type { WebFetchPermissionRequestProps } from './WebFetchPermissionRequest';

export { NotebookEditPermissionRequest } from './NotebookEditPermissionRequest';
export type { NotebookEditPermissionRequestProps } from './NotebookEditPermissionRequest';

export { SkillPermissionRequest } from './SkillPermissionRequest';
export type { SkillPermissionRequestProps } from './SkillPermissionRequest';

export { ComputerUseApproval } from './ComputerUseApproval';
export type { ComputerUseApprovalProps } from './ComputerUseApproval';

export { EnterPlanModePermissionRequest } from './EnterPlanModePermissionRequest';
export type { EnterPlanModePermissionRequestProps } from './EnterPlanModePermissionRequest';

export { ExitPlanModePermissionRequest } from './ExitPlanModePermissionRequest';
export type { ExitPlanModePermissionRequestProps } from './ExitPlanModePermissionRequest';

export { PermissionRuleExplanation } from './PermissionRuleExplanation';
export type { PermissionRuleExplanationProps, PermissionRule as PermissionRuleExplanationRule } from './PermissionRuleExplanation';

// ─── Batch 18 新增权限组件 ──────────────────────────────────

export { AddPermissionRules } from './AddPermissionRules';
export type { AddPermissionRulesProps } from './AddPermissionRules';

export { AddWorkspaceDirectory } from './AddWorkspaceDirectory';
export type { AddWorkspaceDirectoryProps } from './AddWorkspaceDirectory';

export { AskUserQuestionPermissionRequest } from './AskUserQuestionPermissionRequest';
export type { AskUserQuestionPermissionRequestProps } from './AskUserQuestionPermissionRequest';

export { FallbackPermissionRequest } from './FallbackPermissionRequest';
export type { FallbackPermissionRequestProps } from './FallbackPermissionRequest';

export { FilePermissionDialog } from './FilePermissionDialog';
export type { FilePermissionDialogProps } from './FilePermissionDialog';

export { FileWriteToolDiff } from './FileWriteToolDiff';
export type { FileWriteToolDiffProps } from './FileWriteToolDiff';

export { MonitorPermissionRequest } from './MonitorPermissionRequest';
export type { MonitorPermissionRequestProps } from './MonitorPermissionRequest';

export { NotebookEditToolDiff } from './NotebookEditToolDiff';
export type { NotebookEditToolDiffProps } from './NotebookEditToolDiff';

export { PermissionDialog } from './PermissionDialog';
export type { PermissionDialogProps } from './PermissionDialog';

export { PermissionPrompt } from './PermissionPrompt';
export type { PermissionPromptProps } from './PermissionPrompt';

export { PermissionRequest } from './PermissionRequest';
export type { PermissionRequestProps } from './PermissionRequest';

export { PreviewBox } from './PreviewBox';
export type { PreviewBoxProps } from './PreviewBox';

export { PreviewQuestionView } from './PreviewQuestionView';
export type { PreviewQuestionViewProps } from './PreviewQuestionView';

export { QuestionNavigationBar } from './QuestionNavigationBar';
export type { QuestionNavigationBarProps } from './QuestionNavigationBar';

export { QuestionView } from './QuestionView';
export type { QuestionViewProps } from './QuestionView';

export { RecentDenialsTab } from './RecentDenialsTab';
export type { RecentDenialsTabProps } from './RecentDenialsTab';

export { RemoveWorkspaceDirectory } from './RemoveWorkspaceDirectory';
export type { RemoveWorkspaceDirectoryProps } from './RemoveWorkspaceDirectory';

export { ReviewArtifactPermissionRequest } from './ReviewArtifactPermissionRequest';
export type { ReviewArtifactPermissionRequestProps } from './ReviewArtifactPermissionRequest';

export { SandboxPermissionRequest } from './SandboxPermissionRequest';
export type { SandboxPermissionRequestProps } from './SandboxPermissionRequest';

export { SedEditPermissionRequest } from './SedEditPermissionRequest';
export type { SedEditPermissionRequestProps } from './SedEditPermissionRequest';

export { SubmitQuestionsView } from './SubmitQuestionsView';
export type { SubmitQuestionsViewProps } from './SubmitQuestionsView';

export { WorkerBadge } from './WorkerBadge';
export type { WorkerBadgeProps } from './WorkerBadge';

export { WorkerPendingPermission } from './WorkerPendingPermission';
export type { WorkerPendingPermissionProps } from './WorkerPendingPermission';

export { WorkspaceTab } from './WorkspaceTab';
export type { WorkspaceTabProps } from './WorkspaceTab';

export * as permissionUtils from './permissionUtils';
