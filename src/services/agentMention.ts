import { getCachedDigitalAgents } from './digitalAgents';
import type { AgentRouteMode } from './digitalAgents';

export interface AgentMentionParseResult {
  agentId?: string;
  routeMode: AgentRouteMode;
  cleanedQuery: string;
}

const LEADING_MENTION_PATTERN = /^(@[^\s]+)\s*/;

export function parseAgentMention(rawQuery: string): AgentMentionParseResult {
  const trimmed = rawQuery.trim();
  const mentionMatch = trimmed.match(LEADING_MENTION_PATTERN);
  if (!mentionMatch) {
    return { routeMode: 'auto', cleanedQuery: trimmed };
  }

  const leadingMention = mentionMatch[1];
  // 匹配源使用后端动态配置的完整数字人列表（缓存未预热时自动回退默认列表），
  // 保证动态新建数字人的 @ 提及也能被解析
  const matchedAgent = getCachedDigitalAgents().find(
    (agent) => agent.mention === leadingMention || agent.aliases.includes(leadingMention),
  );
  if (!matchedAgent) {
    return { routeMode: 'auto', cleanedQuery: trimmed };
  }

  const cleanedQuery = trimmed.slice(mentionMatch[0].length).trim();
  return {
    agentId: matchedAgent.id,
    routeMode: matchedAgent.routeMode,
    cleanedQuery,
  };
}

/**
 * 为输入文本补上指定 mention 前缀：
 * - 空输入：返回 `mention `；
 * - 已以同一 mention 开头：幂等，原样返回；
 * - 已以其它已注册数字人的 mention/alias 开头：替换为本次 mention，正文保留；
 * - 其余情况：在正文前插入 mention。
 */
export function withAgentMentionPrefix(rawQuery: string, mention: string): string {
  const trimmed = rawQuery.trim();
  if (!trimmed) {
    return `${mention} `;
  }
  if (trimmed.startsWith(`${mention} `) || trimmed === mention) {
    return rawQuery;
  }

  // 已以其它已注册数字人的 mention/alias 开头 → 替换前缀，正文保留
  for (const agent of getCachedDigitalAgents()) {
    for (const name of [agent.mention, ...agent.aliases]) {
      if (!name || name === mention) continue;
      if (trimmed === name) {
        return `${mention} `;
      }
      if (trimmed.startsWith(`${name} `)) {
        return `${mention} ${trimmed.slice(name.length).trim()}`;
      }
    }
  }

  return `${mention} ${trimmed}`;
}
