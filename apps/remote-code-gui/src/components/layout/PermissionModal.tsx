import { useEffect, useMemo, useState } from 'react';
import { ShieldAlert } from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';

const PROMPT_PREFIX = 'prompt:';

type AllowedPrompt = {
  tool: string;
  prompt: string;
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

export function PermissionModal() {
  const pendingPermission = useAppStore((state) => state.pendingPermission);
  const resolvePermission = useAppStore((state) => state.resolvePermission);
  const [feedback, setFeedback] = useState('');
  const isExitPlanMode = pendingPermission?.tool_name === 'exit_plan_mode';
  const inputRecord = useMemo(() => asRecord(pendingPermission?.input), [pendingPermission?.input]);
  const allowedPrompts = useMemo(
    () => extractAllowedPrompts(pendingPermission?.input),
    [pendingPermission?.input],
  );
  const planText = stringField(inputRecord, 'plan');
  const planFilePath = stringField(inputRecord, 'plan_file_path', 'planFilePath');

  useEffect(() => {
    setFeedback('');
  }, [pendingPermission?.request_id]);

  if (!pendingPermission) return null;

  const trimmedFeedback = feedback.trim();

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-slate-950/35 p-4 backdrop-blur-[2px]">
      <div className="w-full max-w-2xl rounded-[28px] border border-[#e1d8ca] bg-white shadow-[0_28px_80px_rgba(15,23,42,0.22)]">
        <div className="border-b border-[#efe8dd] px-6 py-5">
          <div className="flex items-center gap-3">
            <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-[#fff3f2] text-[#b23a2f]">
              <ShieldAlert size={20} />
            </div>
            <div>
              <div className="text-lg font-semibold text-slate-800">权限确认</div>
              <div className="mt-1 text-sm text-slate-500">
                GUI 已收到一个需要人工确认的工具调用。
              </div>
            </div>
          </div>
        </div>

        <div className="space-y-4 px-6 py-5">
          <div>
            <div className="text-sm font-medium text-slate-700">工具</div>
            <div className="mt-1 text-sm text-slate-600">{pendingPermission.tool_name}</div>
          </div>
          <div>
            <div className="text-sm font-medium text-slate-700">说明</div>
            <div className="mt-1 whitespace-pre-wrap text-sm leading-6 text-slate-600">
              {pendingPermission.description}
            </div>
          </div>
          {isExitPlanMode && planText && (
            <div>
              <div className="text-sm font-medium text-slate-700">计划内容</div>
              <pre className="mt-1 max-h-64 overflow-auto rounded-2xl bg-[#f7f5ef] p-4 text-xs leading-6 text-slate-700">
                {planText}
              </pre>
              {planFilePath && (
                <div className="mt-2 break-all text-xs text-slate-500">{planFilePath}</div>
              )}
            </div>
          )}
          {isExitPlanMode && allowedPrompts.length > 0 && (
            <div>
              <div className="text-sm font-medium text-slate-700">请求的语义权限</div>
              <div className="mt-2 space-y-2 rounded-2xl bg-[#f7f5ef] p-4 text-sm text-slate-700">
                {allowedPrompts.map((prompt, index) => (
                  <div key={`${prompt.tool}-${prompt.prompt}-${index}`}>
                    {prompt.tool}({PROMPT_PREFIX} {prompt.prompt})
                  </div>
                ))}
              </div>
            </div>
          )}
          {pendingPermission.blocked_path && (
            <div>
              <div className="text-sm font-medium text-slate-700">目标路径</div>
              <div className="mt-1 break-all rounded-2xl bg-[#f7f5ef] px-3 py-2 text-sm text-slate-600">
                {pendingPermission.blocked_path}
              </div>
            </div>
          )}
          {pendingPermission.permission_suggestions.length > 0 && (
            <div>
              <div className="text-sm font-medium text-slate-700">权限建议</div>
              <div className="mt-1 space-y-2">
                {pendingPermission.permission_suggestions.map((suggestion, index) => (
                  <pre
                    key={index}
                    className="max-h-40 overflow-auto rounded-2xl bg-[#f7f5ef] p-4 text-xs leading-6 text-slate-700"
                  >
                    {formatInput(suggestion)}
                  </pre>
                ))}
              </div>
            </div>
          )}
          <div>
            <div className="text-sm font-medium text-slate-700">输入参数</div>
            <pre className="mt-1 max-h-64 overflow-auto rounded-2xl bg-[#f7f5ef] p-4 text-xs leading-6 text-slate-700">
              {formatInput(pendingPermission.input)}
            </pre>
          </div>
          {isExitPlanMode && (
            <div>
              <label htmlFor="permission-feedback" className="text-sm font-medium text-slate-700">
                审批反馈
              </label>
              <textarea
                id="permission-feedback"
                value={feedback}
                onChange={(event) => setFeedback(event.target.value)}
                placeholder="可选：补充执行要求或拒绝原因。"
                className="mt-2 min-h-28 w-full rounded-2xl border border-[#e3dbcf] bg-white px-4 py-3 text-sm leading-6 text-slate-700 outline-none transition focus:border-slate-400"
              />
            </div>
          )}
        </div>

        <div className="flex justify-end gap-3 border-t border-[#efe8dd] bg-[#fbfaf7] px-6 py-4">
          <button
            onClick={() => {
              void resolvePermission(
                isExitPlanMode
                  ? {
                      allowed: false,
                      message: trimmedFeedback || null,
                      feedback: trimmedFeedback || null,
                    }
                  : { allowed: false },
              );
            }}
            className="rounded-2xl border border-[#e3dbcf] px-4 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-white"
          >
            拒绝
          </button>
          <button
            onClick={() => {
              void resolvePermission(
                isExitPlanMode
                  ? {
                      allowed: true,
                      feedback: trimmedFeedback || null,
                      permission_updates: buildExitPlanPermissionUpdates(allowedPrompts),
                    }
                  : { allowed: true },
              );
            }}
            className="rounded-2xl bg-[#17181a] px-5 py-2 text-sm font-medium text-white transition-colors hover:bg-[#2b2d31]"
          >
            允许执行
          </button>
        </div>
      </div>
    </div>
  );
}
