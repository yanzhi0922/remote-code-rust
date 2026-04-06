const crypto = require('node:crypto')

class RuntimeProtocol {
  constructor(sessionId) {
    this.sessionId = sessionId
  }

  emit(payload) {
    process.stdout.write(`${JSON.stringify(payload)}\n`)
  }

  base() {
    return {
      uuid: crypto.randomUUID(),
      session_id: this.sessionId,
    }
  }

  emitInit(meta) {
    this.emit({
      type: 'system',
      subtype: 'init',
      apiKeySource: meta.apiKeySource ?? 'user',
      remote_code_version: meta.version ?? 'runtime-headless',
      cwd: meta.cwd,
      tools: [
        'ListDirectory',
        'ReadFile',
        'SearchText',
        'WriteFile',
        'ReplaceInFile',
        'EditFile',
        'Bash',
      ],
      mcp_servers: [],
      model: meta.model ?? null,
      permissionMode: meta.permissionMode,
      slash_commands: [],
      output_style: 'default',
      skills: [],
      plugins: [],
      ...this.base(),
    })
  }

  emitState(state) {
    this.emit({
      type: 'system',
      subtype: 'session_state_changed',
      state,
      ...this.base(),
    })
  }

  emitStatus(status) {
    this.emit({
      type: 'system',
      subtype: 'status',
      status,
      ...this.base(),
    })
  }

  emitAssistant(text) {
    this.emit({
      type: 'assistant',
      message: {
        role: 'assistant',
        content: [{ type: 'text', text }],
      },
      parent_tool_use_id: null,
      ...this.base(),
    })
  }

  emitResult({
    text,
    isError = false,
    usage = {},
    errors = null,
    stopReason = 'end_turn',
  }) {
    this.emit({
      type: 'result',
      subtype: isError ? 'error_during_execution' : 'success',
      duration_ms: 0,
      duration_api_ms: 0,
      is_error: isError,
      num_turns: 1,
      result: text,
      stop_reason: stopReason,
      total_cost_usd: 0,
      usage: {
        input_tokens: usage.input_tokens ?? 0,
        output_tokens: usage.output_tokens ?? 0,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
        service_tier: 'standard',
      },
      modelUsage: usage.modelUsage ?? {},
      permission_denials: [],
      errors: errors ?? (isError ? [text] : undefined),
      ...this.base(),
    })
  }

  emitPermissionRequest(payload) {
    this.emit({
      type: 'control_request',
      request_id: payload.requestId,
      request: {
        subtype: 'can_use_tool',
        tool_name: payload.toolName,
        input: payload.input,
        tool_use_id: payload.toolUseId,
        title: payload.title,
        description: payload.description,
        blocked_path: payload.blockedPath ?? null,
        permission_suggestions: payload.suggestions ?? [],
      },
    })
  }

  emitPermissionCancelled(requestId) {
    this.emit({
      type: 'control_cancel_request',
      request_id: requestId,
    })
  }

  emitToolProgress(toolName, elapsedSeconds) {
    this.emit({
      type: 'tool_progress',
      tool_name: toolName,
      elapsed_time_seconds: elapsedSeconds,
      ...this.base(),
    })
  }
}

module.exports = {
  RuntimeProtocol,
}
