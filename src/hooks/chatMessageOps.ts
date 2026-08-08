/**
 * 聊天消息列表纯逻辑操作（「重新生成」与「编辑重发」共用）
 *
 * 全部为纯函数、无副作用，便于 vitest 单测；
 * UI（ChatView）与状态层（useChat）共同依赖此模块。
 */
import type { ChatDisplayMessage } from './useChat';

/**
 * 截断消息列表至（并包含）指定 id 的消息。
 * 用于「重新生成」：保留目标 user 消息及之前的消息，移除其后的旧回复。
 * id 不存在时返回原列表副本（不做截断）。
 */
export function truncateThroughMessage(
  messages: ChatDisplayMessage[],
  messageId: string,
): ChatDisplayMessage[] {
  const index = messages.findIndex((m) => m.id === messageId);
  if (index < 0) return [...messages];
  return messages.slice(0, index + 1);
}

/**
 * 截断消息列表至指定 id 的消息之前（不含该消息）。
 * 用于「编辑重发」：移除目标 user 消息及其之后的所有消息，
 * 随后由发送链路用新文本重新追加。
 * id 不存在时返回原列表副本（不做截断）。
 */
export function truncateBeforeMessage(
  messages: ChatDisplayMessage[],
  messageId: string,
): ChatDisplayMessage[] {
  const index = messages.findIndex((m) => m.id === messageId);
  if (index < 0) return [...messages];
  return messages.slice(0, index);
}

/**
 * 取某条消息之前最近的一条 user 消息（不含自身）。
 * 「重新生成」用它取回产生目标回复的原始 user 消息（含 @mention 前缀的展示文本）。
 * 目标消息不存在或其之前无 user 消息时返回 null。
 */
export function findPrecedingUserMessage(
  messages: ChatDisplayMessage[],
  messageId: string,
): ChatDisplayMessage | null {
  const index = messages.findIndex((m) => m.id === messageId);
  if (index < 0) return null;
  for (let i = index - 1; i >= 0; i -= 1) {
    if (messages[i].role === 'user') return messages[i];
  }
  return null;
}

/**
 * 判断某条消息是否为「同角色连续消息组」的首条。
 * 用于消息分组渲染：仅组首条渲染头像与「你/助理」标签，
 * 后续同角色消息收紧间距、省略头像，降低视觉噪声。
 * index 越界时返回 false。
 */
export function isGroupStart(
  messages: ChatDisplayMessage[],
  index: number,
): boolean {
  if (index < 0 || index >= messages.length) return false;
  if (index === 0) return true;
  return messages[index].role !== messages[index - 1].role;
}

/**
 * 判断某条消息是否为列表中最后一条 assistant 消息。
 * 用于只在最后一条 assistant 消息上展示「重新生成」入口。
 */
export function isLastAssistantMessage(
  messages: ChatDisplayMessage[],
  messageId: string,
): boolean {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    if (messages[i].role === 'assistant') return messages[i].id === messageId;
  }
  return false;
}

/** 用户消息正文拆分结果：纯净正文 + 附件文件名列表 */
export interface UserContentParts {
  /** 去除附件行后的正文（已 trim） */
  body: string;
  /** 从正文中识别出的附件文件名 */
  attachments: string[];
}

/**
 * 匹配历史遗留的附件行：「📎 文件名」（旧版拼接格式，含尾随空格）
 * 或「附件：文件名」纯文本前缀（全角/半角冒号均兼容）。
 */
const ATTACHMENT_LINE_RE = /^(?:📎\s*|附件[：:]\s*)(.+)$/;

/**
 * 将用户消息正文拆分为纯净正文与附件文件名列表。
 * 用于渲染层对已落库历史消息（旧版将「📎 文件名」拼进正文持久化）做归一展示：
 * 附件行改由 Paperclip 图标渲染，不改写数据库内容；
 * 新版消息正文不含附件行时原样返回（attachments 为空）。
 */
export function splitUserAttachments(content: string): UserContentParts {
  const bodyLines: string[] = [];
  const attachments: string[] = [];
  for (const line of content.split('\n')) {
    const matched = ATTACHMENT_LINE_RE.exec(line.trim());
    if (matched) {
      const name = matched[1].trim();
      if (name) attachments.push(name);
    } else {
      bodyLines.push(line);
    }
  }
  return { body: bodyLines.join('\n').trim(), attachments };
}
