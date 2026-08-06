import {
  fetchDigitalAgents,
  getCachedDigitalAgents,
  type DigitalAgent,
} from './digitalAgents';

// CouncilMember 保持接口兼容，供 langgraph.ts、AgentWorkspace.tsx 等使用
export interface CouncilMember {
  id: string;
  name: string;
  role: string;
  skill: string;
  description: string;
}

// 将数字人转换为 CouncilMember 格式：技能描述优先作为 role/skill
function agentToCouncilMember(agent: DigitalAgent): CouncilMember {
  const role = agent.skillDescription || agent.description || '';
  return {
    id: agent.id,
    name: agent.displayName,
    role,
    skill: agent.skillDescription || '',
    description: agent.description || '',
  };
}

/**
 * 异步获取 Council（= 所有激活的数字人转换而来的成员列表）。
 * Council 概念现已合并为数字人列表：每个激活的数字人即对应一个 Council 成员。
 */
export async function fetchCouncil(): Promise<CouncilMember[]> {
  const agents = await fetchDigitalAgents();
  return agents.filter((a) => a.isActive).map(agentToCouncilMember);
}

/**
 * 同步获取 Council 成员列表。基于数字人缓存（需先调用 fetchDigitalAgents 预热）；
 * 缓存未预热时使用内置默认数字人，保证向后兼容。
 */
export function getCouncilMembers(): CouncilMember[] {
  return getCachedDigitalAgents()
    .filter((a) => a.isActive)
    .map(agentToCouncilMember);
}

export function getCouncilMember(id: string | null | undefined): CouncilMember | null {
  if (!id) return null;
  return getCouncilMembers().find((m) => m.id === id) ?? null;
}
