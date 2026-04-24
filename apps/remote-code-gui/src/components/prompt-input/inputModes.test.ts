import { describe, expect, it } from 'vitest';
import {
  prependModeCharacterToInput,
  getModeFromInput,
  getValueFromInput,
  isInputModeCharacter,
} from './inputModes';

describe('inputModes', () => {
  describe('prependModeCharacterToInput', () => {
    it('bash 模式添加 ! 前缀', () => {
      expect(prependModeCharacterToInput('ls', 'bash')).toBe('!ls');
    });

    it('bash 模式不重复添加 ! 前缀', () => {
      expect(prependModeCharacterToInput('!ls', 'bash')).toBe('!ls');
    });

    it('prompt 模式不添加前缀', () => {
      expect(prependModeCharacterToInput('hello', 'prompt')).toBe('hello');
    });
  });

  describe('getModeFromInput', () => {
    it('以 ! 开头返回 bash', () => {
      expect(getModeFromInput('!ls -la')).toBe('bash');
    });

    it('不以 ! 开头返回 prompt', () => {
      expect(getModeFromInput('hello')).toBe('prompt');
    });

    it('空字符串返回 prompt', () => {
      expect(getModeFromInput('')).toBe('prompt');
    });
  });

  describe('getValueFromInput', () => {
    it('去除 ! 前缀', () => {
      expect(getValueFromInput('!ls')).toBe('ls');
    });

    it('无前缀时原样返回', () => {
      expect(getValueFromInput('hello')).toBe('hello');
    });

    it('空字符串原样返回', () => {
      expect(getValueFromInput('')).toBe('');
    });
  });

  describe('isInputModeCharacter', () => {
    it('! 是模式切换字符', () => {
      expect(isInputModeCharacter('!')).toBe(true);
    });

    it('/ 是模式切换字符', () => {
      expect(isInputModeCharacter('/')).toBe(true);
    });

    it('其他字符不是模式切换字符', () => {
      expect(isInputModeCharacter('a')).toBe(false);
    });

    it('空字符串不是模式切换字符', () => {
      expect(isInputModeCharacter('')).toBe(false);
    });

    it('多字符不是模式切换字符', () => {
      expect(isInputModeCharacter('!ls')).toBe(false);
    });
  });
});
