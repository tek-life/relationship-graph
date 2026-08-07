import { getCachedDigitalAgents } from './digitalAgents';
import type { AgentRouteMode } from './digitalAgents';

export interface AgentMentionParseResult {
  agentId?: string;
  routeMode: AgentRouteMode;
  cleanedQuery: string;
}

const LEADING_MENTION_PATTERN = /^(@[^\s]+)\s*/;

/**
 * 规范化 mention 字符串：不以 "@" 开头时自动补 "@"。
 * 兼容管理员创建数字人时漏填 @ 前缀的数据，保证插入与解析链路始终基于 "@xxx" 形式。
 */
export function normalizeMention(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) return trimmed;
  return trimmed.startsWith('@') ? trimmed : `@${trimmed}`;
}

export function parseAgentMention(rawQuery: string): AgentMentionParseResult {
  const trimmed = rawQuery.trim();
  const mentionMatch = trimmed.match(LEADING_MENTION_PATTERN);
  if (!mentionMatch) {
    return { routeMode: 'auto', cleanedQuery: trimmed };
  }

  const leadingMention = normalizeMention(mentionMatch[1]);
  // 匹配源使用后端动态配置的完整数字人列表（缓存未预热时自动回退默认列表），
  // 保证动态新建数字人的 @ 提及也能被解析；
  // 匹配时对 mention/aliases 同样做 @ 规范化，容忍数据源有/无 @ 两种写法
  const matchedAgent = getCachedDigitalAgents().find(
    (agent) =>
      normalizeMention(agent.mention) === leadingMention ||
      agent.aliases.some((alias) => normalizeMention(alias) === leadingMention),
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
export function withAgentMentionPrefix(rawQuery: string, rawMention: string): string {
  // 规范化入参：数据源 mention 缺 @ 时自动补齐，保证输入框始终显示 "@xxx"
  const mention = normalizeMention(rawMention);
  const trimmed = rawQuery.trim();
  if (!trimmed) {
    return `${mention} `;
  }
  if (trimmed.startsWith(`${mention} `) || trimmed === mention) {
    return rawQuery;
  }

  // 已以其它已注册数字人的 mention/alias 开头 → 替换前缀，正文保留
  // 对候选名同样做 @ 规范化，避免数据源缺 @ 时替换判定失效
  for (const agent of getCachedDigitalAgents()) {
    for (const rawName of [agent.mention, ...agent.aliases]) {
      if (!rawName) continue;
      const name = normalizeMention(rawName);
      if (name === mention) continue;
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
