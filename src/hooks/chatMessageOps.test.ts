import { describe, expect, it } from 'vitest';
import {
  truncateThroughMessage,
  truncateBeforeMessage,
  findPrecedingUserMessage,
  isLastAssistantMessage,
  isGroupStart,
  splitUserAttachments,
} from './chatMessageOps';
import type { ChatDisplayMessage } from './useChat';

/** 快速构造测试消息 */
function msg(id: string, role: ChatDisplayMessage['role'], content = id): ChatDisplayMessage {
  return { id, role, content };
}

describe('truncateThroughMessage', () => {
  const messages = [msg('u1', 'user'), msg('a1', 'assistant'), msg('u2', 'user'), msg('a2', 'assistant')];

  it('截断到（并包含）指定消息为止', () => {
    const result = truncateThroughMessage(messages, 'u2');
    expect(result.map((m) => m.id)).toEqual(['u1', 'a1', 'u2']);
  });

  it('目标是最后一条消息时返回全量副本', () => {
    const result = truncateThroughMessage(messages, 'a2');
    expect(result.map((m) => m.id)).toEqual(['u1', 'a1', 'u2', 'a2']);
    expect(result).not.toBe(messages);
  });

  it('目标是第一条消息时只保留第一条', () => {
    const result = truncateThroughMessage(messages, 'u1');
    expect(result.map((m) => m.id)).toEqual(['u1']);
  });

  it('id 不存在时原样返回副本（不截断）', () => {
    const result = truncateThroughMessage(messages, 'nope');
    expect(result.map((m) => m.id)).toEqual(['u1', 'a1', 'u2', 'a2']);
    expect(result).not.toBe(messages);
  });

  it('空列表返回空数组', () => {
    expect(truncateThroughMessage([], 'u1')).toEqual([]);
  });
});

describe('truncateBeforeMessage', () => {
  const messages = [msg('u1', 'user'), msg('a1', 'assistant'), msg('u2', 'user'), msg('a2', 'assistant')];

  it('截断到指定消息之前（不含该消息）', () => {
    const result = truncateBeforeMessage(messages, 'u2');
    expect(result.map((m) => m.id)).toEqual(['u1', 'a1']);
  });

  it('目标是第一条消息时返回空数组', () => {
    expect(truncateBeforeMessage(messages, 'u1')).toEqual([]);
  });

  it('目标是最后一条消息时保留其之前的全部消息', () => {
    const result = truncateBeforeMessage(messages, 'a2');
    expect(result.map((m) => m.id)).toEqual(['u1', 'a1', 'u2']);
  });

  it('id 不存在时原样返回副本（不截断）', () => {
    const result = truncateBeforeMessage(messages, 'nope');
    expect(result.map((m) => m.id)).toEqual(['u1', 'a1', 'u2', 'a2']);
    expect(result).not.toBe(messages);
  });
});

describe('findPrecedingUserMessage', () => {
  it('取回目标消息之前最近的一条 user 消息（重新生成取回原始 query）', () => {
    const messages = [
      msg('u1', 'user', '第一个问题'),
      msg('a1', 'assistant'),
      msg('u2', 'user', '@联系人管家 谁在上海做地产'),
      msg('a2', 'assistant'),
    ];
    const result = findPrecedingUserMessage(messages, 'a2');
    expect(result?.id).toBe('u2');
    expect(result?.content).toBe('@联系人管家 谁在上海做地产');
  });

  it('中间存在多条 assistant 消息时跳过它们取到 user 消息', () => {
    const messages = [msg('u1', 'user', '问题'), msg('a1', 'assistant'), msg('a2', 'assistant')];
    expect(findPrecedingUserMessage(messages, 'a2')?.id).toBe('u1');
  });

  it('目标之前无 user 消息时返回 null', () => {
    const messages = [msg('a1', 'assistant'), msg('a2', 'assistant')];
    expect(findPrecedingUserMessage(messages, 'a2')).toBeNull();
  });

  it('目标消息不存在时返回 null', () => {
    expect(findPrecedingUserMessage([msg('u1', 'user')], 'nope')).toBeNull();
  });

  it('目标是首条消息时返回 null', () => {
    const messages = [msg('u1', 'user'), msg('a1', 'assistant')];
    expect(findPrecedingUserMessage(messages, 'u1')).toBeNull();
  });
});

describe('isGroupStart', () => {
  it('首条消息永远是组首', () => {
    expect(isGroupStart([msg('u1', 'user')], 0)).toBe(true);
  });

  it('角色切换处为组首', () => {
    const messages = [msg('u1', 'user'), msg('a1', 'assistant'), msg('u2', 'user')];
    expect(isGroupStart(messages, 1)).toBe(true);
    expect(isGroupStart(messages, 2)).toBe(true);
  });

  it('同角色连续消息只有首条为组首', () => {
    const messages = [
      msg('a1', 'assistant'),
      msg('a2', 'assistant'),
      msg('a3', 'assistant'),
      msg('u1', 'user'),
      msg('u2', 'user'),
    ];
    expect(isGroupStart(messages, 0)).toBe(true);
    expect(isGroupStart(messages, 1)).toBe(false);
    expect(isGroupStart(messages, 2)).toBe(false);
    expect(isGroupStart(messages, 3)).toBe(true);
    expect(isGroupStart(messages, 4)).toBe(false);
  });

  it('system 消息参与分组判定（前后角色不同即为组首）', () => {
    const messages = [msg('a1', 'assistant'), msg('s1', 'system'), msg('a2', 'assistant')];
    expect(isGroupStart(messages, 1)).toBe(true);
    expect(isGroupStart(messages, 2)).toBe(true);
  });

  it('index 越界时返回 false', () => {
    const messages = [msg('u1', 'user')];
    expect(isGroupStart(messages, -1)).toBe(false);
    expect(isGroupStart(messages, 1)).toBe(false);
    expect(isGroupStart([], 0)).toBe(false);
  });
});

describe('isLastAssistantMessage', () => {
  it('最后一条 assistant 消息返回 true', () => {
    const messages = [msg('u1', 'user'), msg('a1', 'assistant'), msg('a2', 'assistant')];
    expect(isLastAssistantMessage(messages, 'a2')).toBe(true);
  });

  it('非最后一条 assistant 消息返回 false', () => {
    const messages = [msg('u1', 'user'), msg('a1', 'assistant'), msg('a2', 'assistant')];
    expect(isLastAssistantMessage(messages, 'a1')).toBe(false);
  });

  it('最后一条 assistant 之后还有 user 消息时仍识别为最后一条 assistant', () => {
    const messages = [msg('a1', 'assistant'), msg('u1', 'user')];
    expect(isLastAssistantMessage(messages, 'a1')).toBe(true);
  });

  it('user 消息 id 返回 false', () => {
    const messages = [msg('u1', 'user'), msg('a1', 'assistant')];
    expect(isLastAssistantMessage(messages, 'u1')).toBe(false);
  });

  it('无 assistant 消息时返回 false', () => {
    expect(isLastAssistantMessage([msg('u1', 'user')], 'u1')).toBe(false);
  });

  it('空列表返回 false', () => {
    expect(isLastAssistantMessage([], 'a1')).toBe(false);
  });
});

describe('splitUserAttachments', () => {
  it('旧版「📎 文件名」尾部行被剥离为附件，正文保持干净', () => {
    const { body, attachments } = splitUserAttachments('帮我看看这个文件\n📎 文档测试.pdf\n📎 备注.txt');
    expect(body).toBe('帮我看看这个文件');
    expect(attachments).toEqual(['文档测试.pdf', '备注.txt']);
  });

  it('「附件：文件名」纯文本前缀（全角/半角冒号）同样归一为附件', () => {
    expect(splitUserAttachments('正文\n附件：报告.docx').attachments).toEqual(['报告.docx']);
    expect(splitUserAttachments('正文\n附件:报告.docx').attachments).toEqual(['报告.docx']);
  });

  it('无附件行的正文原样返回，attachments 为空', () => {
    const { body, attachments } = splitUserAttachments('@联系人管家 谁在上海做地产？');
    expect(body).toBe('@联系人管家 谁在上海做地产？');
    expect(attachments).toEqual([]);
  });

  it('正文中夹带的附件行也能被识别（渲染层归一不依赖行位置）', () => {
    const { body, attachments } = splitUserAttachments('开头\n📎 a.md\n结尾');
    expect(body).toBe('开头\n结尾');
    expect(attachments).toEqual(['a.md']);
  });

  it('仅含附件行时正文为空字符串', () => {
    const { body, attachments } = splitUserAttachments('📎 only.pdf');
    expect(body).toBe('');
    expect(attachments).toEqual(['only.pdf']);
  });

  it('空字符串安全返回空结果', () => {
    expect(splitUserAttachments('')).toEqual({ body: '', attachments: [] });
  });
});
