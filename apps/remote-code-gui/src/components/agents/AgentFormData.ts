/** Agent 编辑表单数据 */
export interface AgentFormData {
  name: string;
  description: string;
  model: string | null;
  color: string;
  system_prompt: string;
  tools: string[];
  disabled: boolean;
}

/** Agent 完整信息（含内置标记） */
export interface AgentInfo extends AgentFormData {
  is_builtin: boolean;
}

/** 模型选项 */
export interface ModelOption {
  id: string;
  name: string;
  provider: string;
}
