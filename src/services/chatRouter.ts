/**
 * 聊天路由服务
 * 替代 LangGraph 前端编排，提供统一的消息分发逻辑
 */
import { nlqMulti, generalChat } from './db';
import type { NlqResponse, ChatResponse, ChatRouterResponse } from '../types';

/**
 * 统一消息路由：根据 agentId 和 activeAgentIds 决定调用哪个后端接口
 *
 * 路由优先级：
 * 1. contact_manager → NLQ 多意图接口
 * 2. 多智能体协同 → 带上下文前缀的通用聊天
 * 3. 默认 → 通用聊天
 */
export async function routeQuery(
  query: string,
  agentId?: string | null,
  activeAgentIds?: string[],
): Promise<ChatRouterResponse> {
  // 联系人管家模式：调用 NLQ 多意图接口
  if (agentId === 'contact_manager') {
    const result = await nlqMulti(query);
    return formatNlqResponse(result);
  }

  // 多智能体协同模式：在消息中附带多 agent 上下文
  if (activeAgentIds && activeAgentIds.length > 1) {
    const multiAgentQuery = `[Multi-Agent: ${activeAgentIds.join(', ')}] ${query}`;
    const result = await generalChat(multiAgentQuery, agentId ?? undefined);
    return formatChatResponse(result);
  }

  // 默认通用聊天
  const result = await generalChat(query, agentId ?? undefined);
  return formatChatResponse(result);
}

/** 将 NLQ 响应转换为统一路由响应 */
function formatNlqResponse(resp: NlqResponse): ChatRouterResponse {
  switch (resp.intentType) {
    case 'searchPeople': {
      const count = resp.results.length;
      const names = resp.results.slice(0, 5).map((r) => r.displayName).join('、');
      const reply = count > 0
        ? `找到 ${count} 位相关联系人：${names}${count > 5 ? ' 等' : ''}`
        : '未找到匹配的联系人';
      return { type: 'nlq', nlqResponse: resp, reply };
    }

    case 'createPersonDraft': {
      const d = resp.draft;
      const reply = `识别到新联系人草稿：**${d.name}**${d.company ? `（${d.company}）` : ''}，请确认后保存。`;
      return { type: 'nlq', nlqResponse: resp, reply };
    }

    case 'updatePersonDraft': {
      const d = resp.draft;
      const fields = d.changes.map((c) => c.field).join('、');
      const reply = `识别到更新草稿，将修改以下字段：${fields}，请确认。`;
      return { type: 'nlq', nlqResponse: resp, reply };
    }

    case 'addInteractionDraft': {
      const d = resp.draft;
      const reply = `识别到互动记录草稿：与 **${d.personMention}** 的沟通${d.summary ? `——${d.summary}` : ''}，请确认。`;
      return { type: 'nlq', nlqResponse: resp, reply };
    }

    case 'findPath': {
      const reply = resp.path.summary || `找到一条 ${resp.path.hops} 跳的关系路径。`;
      return { type: 'nlq', nlqResponse: resp, reply };
    }
  }
}

/**
 * 将通用聊天响应转换为统一路由响应。
 * 长内容展示不再在此处判断，统一由 contentPolicy.resolveTextDisplay 决定
 * （气泡直出 / 折叠展开 / FilePanel 附件）。
 */
function formatChatResponse(resp: ChatResponse): ChatRouterResponse {
  return { type: 'chat', chatResponse: resp, reply: resp.reply };
}
