/**
 * 输入模式工具函数。
 * 管理输入模式（prompt / bash）与前缀字符的映射关系。
 */

/** 根据模式在输入前添加对应前缀字符 */
export function prependModeCharacterToInput(
  input: string,
  mode: 'prompt' | 'bash',
): string {
  if (mode === 'bash') {
    const trimmed = input.startsWith('!') ? input.slice(1) : input;
    return `!${trimmed}`;
  }
  return input;
}

/** 从输入内容检测当前模式 */
export function getModeFromInput(input: string): 'prompt' | 'bash' {
  if (input.startsWith('!')) {
    return 'bash';
  }
  return 'prompt';
}

/** 去除模式前缀，获取纯输入值 */
export function getValueFromInput(input: string): string {
  if (input.startsWith('!')) {
    return input.slice(1);
  }
  return input;
}

/** 判断输入是否仅由模式切换字符组成 */
export function isInputModeCharacter(input: string): boolean {
  return input === '!' || input === '/';
}
