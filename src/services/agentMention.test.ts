import { describe, expect, it } from 'vitest';
import { normalizeMention, parseAgentMention, withAgentMentionPrefix } from './agentMention';

describe('normalizeMention', () => {
  it('缺 @ 前缀时自动补齐', () => {
    expect(normalizeMention('联系人管家')).toBe('@联系人管家');
  });

  it('已有 @ 前缀时原样返回', () => {
    expect(normalizeMention('@联系人管家')).toBe('@联系人管家');
  });

  it('先去除首尾空白再补齐', () => {
    expect(normalizeMention('  abc  ')).toBe('@abc');
  });

  it('空字符串原样返回', () => {
    expect(normalizeMention('   ')).toBe('');
  });
});

describe('parseAgentMention', () => {
  it('无 mention 时回退 auto 路由', () => {
    const result = parseAgentMention('hello');
    expect(result.agentId).toBeUndefined();
    expect(result.routeMode).toBe('auto');
    expect(result.cleanedQuery).toBe('hello');
  });

  it('识别开头 mention 并剥离前缀', () => {
    const result = parseAgentMention('@联系人管家 帮我看看谁在上海');
    expect(result.agentId).toBe('contact_manager');
    expect(result.routeMode).toBe('relationship');
    expect(result.cleanedQuery).toBe('帮我看看谁在上海');
  });

  it('alias 与 mention 等价识别', () => {
    const result = parseAgentMention('@数字管家 查一下');
    expect(result.agentId).toBe('contact_manager');
    expect(result.cleanedQuery).toBe('查一下');
  });

  it('非开头的 mention 不解析', () => {
    const result = parseAgentMention('你好 @联系人管家');
    expect(result.agentId).toBeUndefined();
    expect(result.routeMode).toBe('auto');
    expect(result.cleanedQuery).toBe('你好 @联系人管家');
  });

  it('mention 后无正文时正文为空', () => {
    const result = parseAgentMention('@联系人管家');
    expect(result.agentId).toBe('contact_manager');
    expect(result.cleanedQuery).toBe('');
  });

  it('未注册的 mention 视为普通正文', () => {
    const result = parseAgentMention('@路人甲 你好');
    expect(result.agentId).toBeUndefined();
    expect(result.routeMode).toBe('auto');
    expect(result.cleanedQuery).toBe('@路人甲 你好');
  });
});

describe('withAgentMentionPrefix', () => {
  it('空输入返回 mention 加空格', () => {
    expect(withAgentMentionPrefix('', '@联系人管家')).toBe('@联系人管家 ');
  });

  it('已以同一 mention 开头时幂等', () => {
    const query = '@联系人管家 你好';
    expect(withAgentMentionPrefix(query, '@联系人管家')).toBe(query);
  });

  it('已以其它已注册 mention 开头时替换前缀、保留正文', () => {
    expect(withAgentMentionPrefix('@数字管家 你好', '@联系人管家')).toBe('@联系人管家 你好');
  });

  it('普通正文前插入 mention', () => {
    expect(withAgentMentionPrefix('你好', '@联系人管家')).toBe('@联系人管家 你好');
  });

  it('mention 入参缺 @ 时自动补齐', () => {
    expect(withAgentMentionPrefix('你好', '联系人管家')).toBe('@联系人管家 你好');
  });
});
