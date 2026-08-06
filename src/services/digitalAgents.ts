import { apiGet } from './api';
import contactManagerAvatar from '../assets/agents/contact-manager-cartoon.png';

export const CONTACT_MANAGER_AGENT_ID = 'contact_manager';

// 内置数字人的本地卡通头像（后端未配置 avatar_url 时兜底）
const BUILTIN_AVATARS: Record<string, string> = {
  [CONTACT_MANAGER_AGENT_ID]: contactManagerAvatar,
};

// 数字人路由模式：auto 表示由意图分类器决定，relationship/chat 为数字人声明的固定模式
export type AgentRouteMode = 'auto' | 'relationship' | 'chat';

export interface DigitalAgent {
  id: string;
  displayName: string;
  mention: string;
  aliases: string[];
  routeMode: Exclude<AgentRouteMode, 'auto'>;
  avatar: string;
  description?: string;
  skillDescription?: string;
  isActive: boolean;
  sortOrder: number;
}

// 后端 /api/digital-agents 返回的原始结构（serde camelCase 序列化）
interface DigitalAgentDto {
  id: string;
  displayName: string;
  mention: string;
  aliases: string[];
  routeMode: string;
  avatarUrl: string | null;
  description: string | null;
  skillDescription: string | null;
  isActive: boolean;
  sortOrder: number;
}

// 将 routeMode 归一化为前端合法的固定路由模式
function normalizeRouteMode(value: string): Exclude<AgentRouteMode, 'auto'> {
  return value === 'relationship' ? 'relationship' : 'chat';
}

// 将后端 DTO 映射为前端 DigitalAgent（avatarUrl -> avatar，null -> undefined）
function mapDtoToAgent(dto: DigitalAgentDto): DigitalAgent {
  return {
    id: dto.id,
    displayName: dto.displayName,
    mention: dto.mention,
    aliases: dto.aliases,
    routeMode: normalizeRouteMode(dto.routeMode),
    avatar: dto.avatarUrl || BUILTIN_AVATARS[dto.id] || '',
    description: dto.description ?? undefined,
    skillDescription: dto.skillDescription ?? undefined,
    isActive: dto.isActive,
    sortOrder: dto.sortOrder,
  };
}

// 默认 fallback 数据：后端不可用或未登录时使用，保证基础功能可用
function getDefaultAgents(): DigitalAgent[] {
  return [
    {
      id: CONTACT_MANAGER_AGENT_ID,
      displayName: '联系人管家',
      mention: '@联系人管家',
      aliases: ['@数字管家', '@contact-manager'],
      routeMode: 'relationship',
      avatar: contactManagerAvatar,
      description: '管理联系人的增删改查，维护关系网络',
      isActive: true,
      sortOrder: 0,
    },
  ];
}

/**
 * @deprecated 保留作为向后兼容的默认值常量。新代码请使用 fetchDigitalAgents() 异步获取，
 * 该常量仅包含内置的联系人管家，无法反映后端动态配置的数字人。
 */
export const DIGITAL_AGENTS: DigitalAgent[] = getDefaultAgents();

// 数字人缓存：fetchDigitalAgents 首次请求后填充，供同步访问使用
let cachedAgents: DigitalAgent[] | null = null;

/**
 * 从后端获取所有数字人并写入缓存。后端不可用、未登录或返回异常时回退到默认数据。
 * 返回的列表不区分是否激活，调用方可按 isActive 自行过滤。
 */
export async function fetchDigitalAgents(): Promise<DigitalAgent[]> {
  if (cachedAgents) return cachedAgents;
  try {
    const data = await apiGet<DigitalAgentDto[]>('/api/digital-agents');
    cachedAgents = data.map(mapDtoToAgent);
    return cachedAgents;
  } catch {
    // 后端不可用（未解锁、服务未启动等）时回退到默认数字人，不写入缓存以便下次重试
    return getDefaultAgents();
  }
}

/**
 * 清除数字人缓存。当管理员更新后端数字人配置后调用以强制刷新。
 */
export function clearDigitalAgentsCache(): void {
  cachedAgents = null;
}

/**
 * 异步按 ID 获取数字人。
 */
export async function fetchDigitalAgentById(id: string): Promise<DigitalAgent | undefined> {
  const agents = await fetchDigitalAgents();
  return agents.find((a) => a.id === id);
}

/**
 * 同步获取当前缓存的数字人列表。缓存未预热时回退到默认数字人，保证向后兼容。
 * 供需要同步访问数字人列表的调用方（如 council.ts）使用，避免直接访问模块内部缓存。
 */
export function getCachedDigitalAgents(): DigitalAgent[] {
  return cachedAgents ?? DIGITAL_AGENTS;
}

/**
 * 同步按 ID 获取数字人。优先从缓存查找；缓存未预热时回退到默认数字人，保证向后兼容。
 * 建议在应用启动时调用 fetchDigitalAgents() 预热缓存，以获取后端动态配置的完整列表。
 */
export function getDigitalAgentById(id: string): DigitalAgent | undefined {
  return cachedAgents?.find((a) => a.id === id) ?? DIGITAL_AGENTS.find((a) => a.id === id);
}
