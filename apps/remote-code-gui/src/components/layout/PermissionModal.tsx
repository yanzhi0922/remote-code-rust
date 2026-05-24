import { useEffect, useMemo, useState } from 'react';
import { ShieldAlert } from 'lucide-react';
import { formatSensitivePath, redactSensitivePathsForDisplay } from '../../lib/utils';
import { useAppStore } from '../../stores/useAppStore';

const PROMPT_PREFIX = 'prompt:';

type AllowedPrompt = {
  tool: string;
  prompt: string;
};

type CodexQuestion = {
  id: string;
  header?: string;
  question?: string;
  options?: { label: string; description?: string }[] | null;
};

function formatInput(input: unknown): string {
  try {
    return JSON.stringify(input, null, 2);
  } catch {
    return String(input);
  }
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function stringField(record: Record<string, unknown> | null, ...keys: string[]): string | null {
  for (const key of keys) {
    const value = record?.[key];
    if (typeof value === 'string' && value.trim().length > 0) {
      return value;
    }
  }
  return null;
}

function extractAllowedPrompts(input: unknown): AllowedPrompt[] {
  const record = asRecord(input);
  const rawPrompts = record?.allowedPrompts;
  if (!Array.isArray(rawPrompts)) return [];

  return rawPrompts
    .map((item) => {
      const prompt = asRecord(item);
      const tool = typeof prompt?.tool === 'string' ? prompt.tool.trim() : '';
      const description = typeof prompt?.prompt === 'string' ? prompt.prompt.trim() : '';
      if (!tool || !description) return null;
      return { tool, prompt: description };
    })
    .filter((item): item is AllowedPrompt => Boolean(item));
}

function buildExitPlanPermissionUpdates(allowedPrompts: AllowedPrompt[]): unknown[] | undefined {
  if (allowedPrompts.length === 0) {
    return undefined;
  }

  return [
    {
      type: 'addRules',
      destination: 'session',
      behavior: 'allow',
      rules: allowedPrompts.map((prompt) => ({
        tool_name: prompt.tool,
        rule_content: `${PROMPT_PREFIX} ${prompt.prompt.trim()}`,
      })),
    },
  ];
}

function extractCodexQuestions(input: unknown): CodexQuestion[] {
  const record = asRecord(input);
  const rawQuestions = record?.questions;
  if (!Array.isArray(rawQuestions)) return [];

  const questions: CodexQuestion[] = [];
  for (const item of rawQuestions) {
    const question = asRecord(item);
    const id = typeof question?.id === 'string' ? question.id : '';
    if (!id) continue;

    const options: NonNullable<CodexQuestion['options']> = [];
    if (Array.isArray(question?.options)) {
      for (const option of question.options) {
        const optionRecord = asRecord(option);
        const label = typeof optionRecord?.label === 'string' ? optionRecord.label : '';
        if (!label) continue;
        options.push({
          label,
          description:
            typeof optionRecord?.description === 'string' ? optionRecord.description : undefined,
        });
      }
    }

    questions.push({
      id,
      header: typeof question?.header === 'string' ? question.header : undefined,
      question: typeof question?.question === 'string' ? question.question : undefined,
      options: options.length > 0 ? options : null,
    });
  }
  return questions;
}

function defaultCodexAnswers(questions: CodexQuestion[]): Record<string, { answers: string[] }> {
  return Object.fromEntries(
    questions.map((question) => [
      question.id,
      { answers: question.options?.[0]?.label ? [question.options[0].label] : [] },
    ]),
  );
}

function parseJsonOrText(value: string): unknown {
  const trimmed = value.trim();
  if (!trimmed) return null;
  try {
    return JSON.parse(trimmed);
  } catch {
    return trimmed;
  }
}

export function PermissionModal() {
  const pendingPermission = useAppStore((state) => state.pendingPermission);
  const resolvePermission = useAppStore((state) => state.resolvePermission);
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);
  const [feedback, setFeedback] = useState('');
  const [codexJsonResponse, setCodexJsonResponse] = useState('');
  const [codexTextResponse, setCodexTextResponse] = useState('');
  const isExitPlanMode = pendingPermission?.tool_name === 'exit_plan_mode';
  const isCodexToolUserInput = pendingPermission?.tool_name === 'tool_user_input';
  const isCodexMcpElicitation = pendingPermission?.tool_name === 'mcp_elicitation';
  const isCodexDynamicTool = pendingPermission?.tool_name === 'dynamic_tool';
  const inputRecord = useMemo(() => asRecord(pendingPermission?.input), [pendingPermission?.input]);
  const rooAsk = stringField(inputRecord, 'ask');
  const isRooFollowup = pendingPermission?.tool_name === 'ask_followup_question' || rooAsk === 'followup';
  const isRooCompletion = pendingPermission?.tool_name === 'attempt_completion' || rooAsk === 'completion_result';
  const isRooMistakeLimit =
    pendingPermission?.tool_name === 'mistake_limit_reached' || rooAsk === 'mistake_limit_reached';
  const isRooTextInteraction = isRooFollowup || isRooCompletion || isRooMistakeLimit;
  const rooQuestionRecord = useMemo(() => asRecord(inputRecord?.question), [inputRecord]);
  const allowedPrompts = useMemo(
    () => extractAllowedPrompts(pendingPermission?.input),
    [pendingPermission?.input],
  );
  const codexQuestions = useMemo(
    () => extractCodexQuestions(pendingPermission?.input),
    [pendingPermission?.input],
  );
  const planText = stringField(inputRecord, 'plan');
  const planFilePath = stringField(inputRecord, 'plan_file_path', 'planFilePath');
  const displayedInput = useMemo(
    () => redactSensitivePathsForDisplay(pendingPermission?.input, privacyMode),
    [pendingPermission?.input, privacyMode],
  );

  useEffect(() => {
    setFeedback('');
    setCodexTextResponse('');
    if (pendingPermission?.tool_name === 'tool_user_input') {
      setCodexJsonResponse(
        JSON.stringify({ answers: defaultCodexAnswers(extractCodexQuestions(pendingPermission.input)) }, null, 2),
      );
    } else if (pendingPermission?.tool_name === 'mcp_elicitation') {
      setCodexJsonResponse(JSON.stringify({ action: 'accept', content: {}, _meta: null }, null, 2));
    } else {
      setCodexJsonResponse('');
    }
  }, [pendingPermission?.request_id]);

  if (!pendingPermission) return null;

  const trimmedFeedback = feedback.trim();
  const rooQuestionText = stringField(rooQuestionRecord, 'question') ?? stringField(inputRecord, 'question');
  const rooCompletionText = stringField(inputRecord, 'result');
  const rooResponseLabel = isRooFollowup
    ? 'Roo 回复'
    : isRooCompletion
    ? 'Roo 完成反馈'
    : 'Roo 继续反馈';

  function denyPermission() {
    if (isCodexMcpElicitation) {
      void resolvePermission({
        allowed: false,
        codex_response: { action: 'decline', content: null, _meta: null },
      });
      return;
    }
    if (isExitPlanMode || isRooTextInteraction) {
      void resolvePermission({
        allowed: false,
        message: trimmedFeedback || null,
        feedback: trimmedFeedback || null,
      });
      return;
    }
    void resolvePermission({ allowed: false });
  }

  function allowPermission() {
    if (isCodexToolUserInput || isCodexMcpElicitation) {
      void resolvePermission({
        allowed: true,
        codex_response: parseJsonOrText(codexJsonResponse),
      });
      return;
    }
    if (isCodexDynamicTool) {
      void resolvePermission({
        allowed: true,
        codex_response: {
          contentItems: [
            {
              type: 'inputText',
              text: codexTextResponse.trim() || 'Approved by user.',
            },
          ],
          success: true,
        },
      });
      return;
    }
    if (isRooTextInteraction) {
      void resolvePermission({
        allowed: true,
        message: trimmedFeedback || null,
        feedback: trimmedFeedback || null,
      });
      return;
    }
    void resolvePermission(
      isExitPlanMode
        ? {
            allowed: true,
            feedback: trimmedFeedback || null,
            permission_updates: buildExitPlanPermissionUpdates(allowedPrompts),
          }
        : { allowed: true },
    );
  }

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-rc-bg-overlay p-4">
      <div className="flex max-h-[88vh] w-full max-w-2xl flex-col overflow-hidden rounded-md border border-rc-border-primary bg-rc-bg-surface shadow-xl">
        <div className="shrink-0 border-b border-rc-border-secondary px-5 py-4">
          <div className="flex items-center gap-3">
            <div className="flex h-9 w-9 items-center justify-center rounded-md bg-rc-accent-error-bg text-rc-accent-error">
              <ShieldAlert size={18} />
            </div>
            <div>
              <div className="text-sm font-semibold text-rc-text-primary">权限确认</div>
              <div className="mt-1 text-xs text-rc-text-secondary">{pendingPermission.title}</div>
            </div>
          </div>
        </div>

        <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-5 py-4">
          <div>
            <div className="text-sm font-medium text-rc-text-primary">工具</div>
            <div className="mt-1 text-sm text-rc-text-secondary">{pendingPermission.tool_name}</div>
          </div>
          <div>
            <div className="text-sm font-medium text-rc-text-primary">说明</div>
            <div className="mt-1 whitespace-pre-wrap text-sm leading-6 text-rc-text-secondary">
              {pendingPermission.description}
            </div>
          </div>
          {isExitPlanMode && planText && (
            <div>
              <div className="text-sm font-medium text-rc-text-primary">计划内容</div>
              <pre className="mt-1 max-h-64 overflow-auto rounded-md bg-rc-bg-secondary p-4 text-xs leading-6 text-rc-text-primary">
                {planText}
              </pre>
              {planFilePath && (
                <div className="mt-2 break-all text-xs text-rc-text-tertiary">
                  {formatSensitivePath(planFilePath, privacyMode)}
                </div>
              )}
            </div>
          )}
          {isExitPlanMode && allowedPrompts.length > 0 && (
            <div>
              <div className="text-sm font-medium text-rc-text-primary">请求的语义权限</div>
              <div className="mt-2 space-y-2 rounded-md bg-rc-bg-secondary p-4 text-sm text-rc-text-primary">
                {allowedPrompts.map((prompt, index) => (
                  <div key={`${prompt.tool}-${prompt.prompt}-${index}`}>
                    {prompt.tool}({PROMPT_PREFIX} {prompt.prompt})
                  </div>
                ))}
              </div>
            </div>
          )}
          {isCodexToolUserInput && (
            <div className="space-y-3 rounded-md border border-rc-border-secondary bg-rc-bg-surface p-4">
              <div className="text-sm font-medium text-rc-text-primary">Codex 用户输入请求</div>
              {codexQuestions.map((question) => (
                <div key={question.id} className="rounded-md bg-rc-bg-surface p-3 text-sm text-rc-text-primary">
                  <div className="font-medium">{question.header || question.id}</div>
                  {question.question && <div className="mt-1 text-rc-text-secondary">{question.question}</div>}
                  {question.options && question.options.length > 0 && (
                    <div className="mt-2 space-y-1 text-xs text-rc-text-tertiary">
                      {question.options.map((option) => (
                        <div key={option.label}>
                          {option.label}
                          {option.description ? ` - ${option.description}` : ''}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              ))}
              <label htmlFor="codex-user-input-response" className="text-sm font-medium text-rc-text-primary">
                官方 ToolRequestUserInputResponse JSON
              </label>
              <textarea
                id="codex-user-input-response"
                value={codexJsonResponse}
                onChange={(event) => setCodexJsonResponse(event.target.value)}
                className="min-h-40 w-full rounded-md border border-rc-border-secondary bg-rc-bg-surface px-4 py-3 font-mono text-xs leading-5 text-rc-text-primary outline-none transition focus:border-rc-border-focus"
              />
            </div>
          )}
          {isCodexMcpElicitation && (
            <div className="space-y-3 rounded-md border border-rc-border-secondary bg-rc-bg-surface p-4">
              <div className="text-sm font-medium text-rc-text-primary">Codex MCP elicitation</div>
              <div className="text-sm text-rc-text-secondary">
                填写官方 `McpServerElicitationRequestResponse`。拒绝按钮会返回 decline。
              </div>
              <textarea
                aria-label="MCP elicitation response"
                value={codexJsonResponse}
                onChange={(event) => setCodexJsonResponse(event.target.value)}
                className="min-h-40 w-full rounded-md border border-rc-border-secondary bg-rc-bg-surface px-4 py-3 font-mono text-xs leading-5 text-rc-text-primary outline-none transition focus:border-rc-border-focus"
              />
            </div>
          )}
          {isCodexDynamicTool && (
            <div className="space-y-3 rounded-md border border-rc-border-secondary bg-rc-bg-surface p-4">
              <label htmlFor="codex-dynamic-tool-output" className="text-sm font-medium text-rc-text-primary">
                Codex dynamic tool 输出
              </label>
              <textarea
                id="codex-dynamic-tool-output"
                value={codexTextResponse}
                onChange={(event) => setCodexTextResponse(event.target.value)}
                placeholder="返回给官方 DynamicToolCallResponse 的文本。"
                className="min-h-28 w-full rounded-md border border-rc-border-secondary bg-rc-bg-surface px-4 py-3 text-sm leading-6 text-rc-text-primary outline-none transition focus:border-rc-border-focus"
              />
            </div>
          )}
          {isRooFollowup && rooQuestionText && (
            <div className="rounded-md border border-rc-border-secondary bg-rc-bg-surface p-4 text-sm leading-6 text-rc-text-primary">
              {rooQuestionText}
            </div>
          )}
          {isRooCompletion && rooCompletionText && (
            <pre className="max-h-64 overflow-auto rounded-md bg-rc-bg-secondary p-4 text-sm leading-6 text-rc-text-primary">
              {rooCompletionText}
            </pre>
          )}
          {isRooTextInteraction && (
            <div>
              <label htmlFor="roo-permission-feedback" className="text-sm font-medium text-rc-text-primary">
                {rooResponseLabel}
              </label>
              <textarea
                id="roo-permission-feedback"
                value={feedback}
                onChange={(event) => setFeedback(event.target.value)}
                placeholder={
                  isRooCompletion
                    ? '留空表示接受结果；填写内容会作为反馈继续执行。'
                    : '填写要返回给 Roo 的补充信息。'
                }
                className="mt-2 min-h-28 w-full rounded-md border border-rc-border-secondary bg-rc-bg-surface px-4 py-3 text-sm leading-6 text-rc-text-primary outline-none transition focus:border-rc-border-focus"
              />
            </div>
          )}
          {pendingPermission.blocked_path && (
            <div>
              <div className="text-sm font-medium text-rc-text-primary">目标路径</div>
              <div className="mt-1 break-all rounded-md bg-rc-bg-secondary px-3 py-2 text-sm text-rc-text-secondary">
                {formatSensitivePath(pendingPermission.blocked_path, privacyMode)}
              </div>
            </div>
          )}
          {pendingPermission.permission_suggestions.length > 0 && (
            <div>
              <div className="text-sm font-medium text-rc-text-primary">权限建议</div>
              <div className="mt-1 space-y-2">
                {pendingPermission.permission_suggestions.map((suggestion, index) => (
                  <pre
                    key={`suggestion-${index}-${String(suggestion).slice(0, 32)}`}
                    className="max-h-40 overflow-auto rounded-md bg-rc-bg-secondary p-4 text-xs leading-6 text-rc-text-primary"
                  >
                    {formatInput(redactSensitivePathsForDisplay(suggestion, privacyMode))}
                  </pre>
                ))}
              </div>
            </div>
          )}
          <div>
            <div className="text-sm font-medium text-rc-text-primary">输入参数</div>
            <pre className="mt-1 max-h-64 overflow-auto rounded-md bg-rc-bg-secondary p-4 text-xs leading-6 text-rc-text-primary">
              {formatInput(displayedInput)}
            </pre>
          </div>
          {isExitPlanMode && (
            <div>
              <label htmlFor="permission-feedback" className="text-sm font-medium text-rc-text-primary">
                审批反馈
              </label>
              <textarea
                id="permission-feedback"
                value={feedback}
                onChange={(event) => setFeedback(event.target.value)}
                placeholder="可选：补充执行要求或拒绝原因。"
                className="mt-2 min-h-28 w-full rounded-md border border-rc-border-secondary bg-rc-bg-surface px-4 py-3 text-sm leading-6 text-rc-text-primary outline-none transition focus:border-rc-border-focus"
              />
            </div>
          )}
        </div>

        <div className="shrink-0 flex justify-end gap-3 border-t border-rc-border-secondary bg-rc-bg-secondary px-6 py-4">
          <button
            onClick={denyPermission}
            className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover"
          >
            拒绝
          </button>
          <button
            onClick={allowPermission}
            className="rounded-md bg-rc-accent-primary px-5 py-2 text-sm font-medium text-rc-text-inverse transition-colors hover:bg-rc-accent-primary-hover"
          >
            允许执行
          </button>
        </div>
      </div>
    </div>
  );
}
