import type { AgentChatMessage } from '../types';
import { nlqConfirm } from './db';
import { runLangGraphWorkflow, type LangGraphAgentOptions } from './langgraph';

export async function submitAgentQuery(query: string, options: LangGraphAgentOptions = {}): Promise<AgentChatMessage> {
  const result = await runLangGraphWorkflow(query, options);

  return {
    id: `assistant-${Date.now()}`,
    role: 'assistant',
    content: result.assistantReply,
    artifact: result.artifact ?? undefined,
    workflowTrace: result.workflowTrace,
    status: 'success',
  };
}

export async function confirmAgentDraft(intentType: string, data: Record<string, unknown>): Promise<unknown> {
  return nlqConfirm(intentType, data);
}
