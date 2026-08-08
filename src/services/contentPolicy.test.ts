import { describe, expect, it } from 'vitest';
import { ATTACHMENT_MIN, COLLAPSE_MIN, resolveTextDisplay } from './contentPolicy';

describe('resolveTextDisplay', () => {
  it('短文本（<400 字）直接内联渲染', () => {
    expect(resolveTextDisplay('x'.repeat(COLLAPSE_MIN - 1)).mode).toBe('inline');
  });

  it('400 字进入气泡内可折叠区间', () => {
    expect(resolveTextDisplay('x'.repeat(COLLAPSE_MIN)).mode).toBe('collapsible');
  });

  it('400–1499 字保持可折叠', () => {
    expect(resolveTextDisplay('x'.repeat(ATTACHMENT_MIN - 1)).mode).toBe('collapsible');
  });

  it('1500 字进入 FilePanel 附件并生成摘要', () => {
    const decision = resolveTextDisplay('x'.repeat(ATTACHMENT_MIN));
    expect(decision.mode).toBe('attachment');
    expect(decision.summary).toBeDefined();
  });

  it('空白字符不计入长度', () => {
    expect(resolveTextDisplay(' '.repeat(ATTACHMENT_MIN)).mode).toBe('inline');
  });

  it('摘要取首段并截断到 140 字加省略号', () => {
    const decision = resolveTextDisplay(`${'a'.repeat(200)}\n\n${'b'.repeat(ATTACHMENT_MIN)}`);
    expect(decision.summary).toBe(`${'a'.repeat(140)}…`);
  });

  it('首段较短时摘要原样保留', () => {
    const decision = resolveTextDisplay(`hello\n\n${'x'.repeat(ATTACHMENT_MIN)}`);
    expect(decision.summary).toBe('hello');
  });
});
