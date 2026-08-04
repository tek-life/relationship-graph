import contactManagerAvatar from '../assets/agents/contact-manager.svg';

export type AgentRouteMode = 'auto' | 'relationship' | 'chat';

export interface DigitalAgent {
  id: string;
  displayName: string;
  mention: string;
  aliases: string[];
  routeMode: Exclude<AgentRouteMode, 'auto'>;
  avatar: string;
}

export const CONTACT_MANAGER_AGENT_ID = 'contact_manager';

export const DIGITAL_AGENTS: DigitalAgent[] = [
  {
    id: CONTACT_MANAGER_AGENT_ID,
    displayName: '联系人管家',
    mention: '@联系人管家',
    aliases: ['@数字管家', '@contact-manager'],
    routeMode: 'relationship',
    avatar: contactManagerAvatar,
  },
];

export function getDigitalAgentById(id: string): DigitalAgent | undefined {
  return DIGITAL_AGENTS.find((agent) => agent.id === id);
}
