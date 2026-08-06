// === 管理后台共享类型 ===
// 与后端 server/src/types.rs 中 camelCase 序列化的结构体对应

/** 数字人 */
export interface DigitalAgent {
  id: string;
  displayName: string;
  mention: string;
  aliases: string[];
  routeMode: string;
  avatarUrl?: string | null;
  description?: string | null;
  skillDescription?: string | null;
  isActive: boolean;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

/** 创建/更新数字人请求体 */
export interface CreateDigitalAgentRequest {
  displayName: string;
  mention: string;
  aliases: string[];
  routeMode?: string;
  avatarUrl?: string | null;
  description?: string | null;
  skillDescription?: string | null;
  isActive?: boolean;
  sortOrder?: number;
}

/** 数字人技能 */
export interface AgentSkill {
  id: string;
  agentId: string;
  skillName: string;
  skillConfigJson: string;
  triggerScenario?: string | null;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
}

/** 创建/更新技能请求体 */
export interface CreateAgentSkillRequest {
  agentId: string;
  skillName: string;
  skillConfigJson: string;
  triggerScenario?: string | null;
  isActive?: boolean;
}

/** QA 指令模块 */
export interface QaInstructionModule {
  id: string;
  name: string;
  description?: string | null;
  systemPrompt: string;
  guidanceText?: string | null;
  sortOrder: number;
  triggerScenario: string;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
}

/** 创建/更新 QA 模块请求体 */
export interface CreateQaInstructionModuleRequest {
  name: string;
  description?: string | null;
  systemPrompt: string;
  guidanceText?: string | null;
  sortOrder?: number;
  triggerScenario?: string;
  isActive?: boolean;
}

/** 邀请令牌 */
export interface InviteToken {
  id: string;
  token: string;
  createdBy: string;
  usedBy?: string | null;
  expiresAt: string;
  createdAt: string;
}

/** 创建邀请响应 */
export interface CreateInviteResponse {
  token: string;
  expiresAt: string;
}

/** 管理后台用户列表项（后端 User 序列化后包含 passwordHash，前端不使用） */
export interface AdminUser {
  id: string;
  username: string;
  displayName?: string | null;
  role: 'admin' | 'user';
  profileCompleted: boolean;
  createdAt: string;
  updatedAt: string;
}

/** 技能触发场景选项 */
export const TRIGGER_SCENARIOS = ['always', 'on_mention', 'manual'] as const;
export type TriggerScenario = (typeof TRIGGER_SCENARIOS)[number];

/** 路由模式选项 */
export const ROUTE_MODES = ['relationship', 'chat'] as const;
export type RouteMode = (typeof ROUTE_MODES)[number];
