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
