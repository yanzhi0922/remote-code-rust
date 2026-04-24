export interface AgentDefinition {
  name: string;
  description?: string;
  model?: string;
  prompt?: string;
  tools?: string[];
}

export interface ValidationResult {
  valid: boolean;
  errors: string[];
  warnings: string[];
}

export function validateAgent(agent: Partial<AgentDefinition>): ValidationResult {
  const errors: string[] = [];
  const warnings: string[] = [];

  if (!agent.name || agent.name.trim().length === 0) {
    errors.push('名称不能为空');
  }

  if (agent.name && agent.name.length > 100) {
    errors.push('名称不能超过100个字符');
  }

  if (agent.name && !/^[a-zA-Z0-9_\-\u4e00-\u9fa5\s]+$/.test(agent.name)) {
    errors.push('名称包含非法字符');
  }

  if (!agent.prompt || agent.prompt.trim().length === 0) {
    warnings.push('建议添加提示词');
  }

  if (agent.tools && agent.tools.length === 0) {
    warnings.push('未指定工具，Agent将无法执行操作');
  }

  return {
    valid: errors.length === 0,
    errors,
    warnings,
  };
}
