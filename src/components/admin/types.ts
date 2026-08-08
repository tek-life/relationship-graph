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
  /** SKILL Markdown 文档全文（含 frontmatter）；旧 JSON 形态技能为空 */
  skillMarkdown?: string | null;
  triggerScenario?: string | null;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
}

/** 创建/更新技能请求体 */
export interface CreateAgentSkillRequest {
  agentId: string;
  skillName: string;
  /** Markdown 形态技能可传 "{}" 或不传 */
  skillConfigJson?: string;
  /** SKILL Markdown 文档全文（含 frontmatter） */
  skillMarkdown?: string | null;
  triggerScenario?: string | null;
  isActive?: boolean;
}

/** 内观画像指令模块 */
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

/** 创建/更新内观画像指令模块请求体 */
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

/** 技能包文件 */
export interface SkillPackageFile {
  id: string;
  packageId: string;
  relPath: string;
  content: string;
  sizeChars: number;
}

/** 技能包（多文件） */
export interface SkillPackage {
  id: string;
  slug: string;
  displayName: string;
  description: string | null;
  sourceKind: 'inline' | 'imported';
  totalChars: number;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
  /** GET /:id 时携带；列表接口不返回文件内容 */
  files?: SkillPackageFile[];
}

/** 创建技能包请求体（内联） */
export interface CreateSkillPackageRequest {
  displayName: string;
  description?: string;
  files: { relPath: string; content: string }[];
}

/** 导入技能包请求体 */
export interface ImportSkillPackageRequest {
  name?: string;
  /** relPath → content 的映射 */
  files: Record<string, string>;
}

/** 导入技能包响应 */
export interface ImportSkillPackageResponse {
  package: SkillPackage;
  report: { fileCount: number; totalChars: number; overBudget: boolean };
}

/** 数字人↔技能包绑定 */
export interface SkillBinding {
  agentId: string;
  packageId: string;
  sortOrder: number;
  packageDisplayName: string;
}

/** 全量替换绑定请求体 */
export interface PutSkillBindingsRequest {
  bindings: { packageId: string; sortOrder: number }[];
}

/** 技能触发场景选项 */
export const TRIGGER_SCENARIOS = ['always', 'on_mention', 'manual'] as const;
export type TriggerScenario = (typeof TRIGGER_SCENARIOS)[number];

/** 路由模式选项 */
export const ROUTE_MODES = ['relationship', 'chat'] as const;
export type RouteMode = (typeof ROUTE_MODES)[number];

/** 云端 API Key 生效来源（优先级 env > file > db）；未配置为 null */
export type CloudApiKeySource = 'env' | 'file' | 'db';

/** 云端 API Key 配置摘要（只回掩码，服务端绝不回传明文） */
export interface CloudApiKeyStatus {
  /** 是否有生效 Key（任一层来源命中） */
  configured: boolean;
  /** 生效来源；未配置为 null */
  source: CloudApiKeySource | null;
  /** 掩码展示（如 sk-…abcd）；未配置为 null */
  mask: string | null;
  /** settings 表（db 层）是否已保存 Key */
  dbConfigured: boolean;
}

/** GET /api/admin/config 响应 */
export interface SystemConfig {
  cloudApiKey: CloudApiKeyStatus;
}

/** PUT /api/admin/config/cloud-api-key 请求体 */
export interface UpdateCloudApiKeyRequest {
  apiKey: string;
}
