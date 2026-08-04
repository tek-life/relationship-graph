export interface CouncilMember {
  id: string;
  name: string;
  role: string;
  skill: string;
  description: string;
}

export const DEFAULT_COUNCIL: CouncilMember[] = [
  {
    id: 'coordinator',
    name: '推进官',
    role: '项目推进',
    skill: '把关系上下文总结为下一步行动列表',
    description: '适用于项目推进、会议纪要和待办生成。',
  },
  {
    id: 'writer',
    name: '文案助手',
    role: '写作与邮件',
    skill: '根据现有关系上下文起草邮件或消息',
    description: '适用于跟进消息、邮件草稿和简短说明。',
  },
  {
    id: 'analyzer',
    name: '分析师',
    role: '关系与机会分析',
    skill: '根据关系图谱和最近互动给出推荐路径',
    description: '适用于机会分析、关系优先级和路径推荐。',
  },
];

export function getCouncilMembers(): CouncilMember[] {
  return DEFAULT_COUNCIL;
}

export function getCouncilMember(id: string | null | undefined): CouncilMember | null {
  if (!id) return null;
  return DEFAULT_COUNCIL.find((member) => member.id === id) ?? null;
}
