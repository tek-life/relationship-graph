import { DIGITAL_AGENTS } from './digitalAgents';
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
  const matchedAgent = DIGITAL_AGENTS.find(
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

export function withAgentMentionPrefix(rawQuery: string, mention: string): string {
  const trimmed = rawQuery.trim();
  if (!trimmed) {
    return `${mention} `;
  }
  if (trimmed.startsWith(`${mention} `) || trimmed === mention) {
    return rawQuery;
  }
  return `${mention} ${trimmed}`;
}
