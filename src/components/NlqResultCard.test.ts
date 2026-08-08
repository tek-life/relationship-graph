import { describe, expect, it } from 'vitest';
import {
  strengthText,
  statusText,
  sensitivityText,
  strengthVariant,
  statusVariant,
  sensitivityVariant,
} from './NlqResultCard';

describe('strengthText', () => {
  it('strong/weak/medium 分别映射为 强/弱/中', () => {
    expect(strengthText('strong')).toBe('强');
    expect(strengthText('weak')).toBe('弱');
    expect(strengthText('medium')).toBe('中');
  });

  it('空值回落为中', () => {
    expect(strengthText(null)).toBe('中');
    expect(strengthText(undefined)).toBe('中');
  });
});

describe('statusText', () => {
  it('三种状态映射为中文文案', () => {
    expect(statusText('follow-up')).toBe('待跟进');
    expect(statusText('cold')).toBe('冷却');
    expect(statusText('active')).toBe('活跃');
  });
});

describe('sensitivityText', () => {
  it('三个敏感级映射为中文文案', () => {
    expect(sensitivityText('high')).toBe('高敏感');
    expect(sensitivityText('medium')).toBe('中敏感');
    expect(sensitivityText('low')).toBe('低敏感');
  });
});

describe('Badge 变体映射', () => {
  it('关系强度：strong 用 info 强调，其余中性', () => {
    expect(strengthVariant('strong')).toBe('info');
    expect(strengthVariant('medium')).toBe('default');
    expect(strengthVariant('weak')).toBe('default');
    expect(strengthVariant(null)).toBe('default');
  });

  it('状态：待跟进=warning，活跃=success，冷却=default', () => {
    expect(statusVariant('follow-up')).toBe('warning');
    expect(statusVariant('active')).toBe('success');
    expect(statusVariant('cold')).toBe('default');
  });

  it('敏感级：high=danger，medium=warning，low=default', () => {
    expect(sensitivityVariant('high')).toBe('danger');
    expect(sensitivityVariant('medium')).toBe('warning');
    expect(sensitivityVariant('low')).toBe('default');
  });
});
