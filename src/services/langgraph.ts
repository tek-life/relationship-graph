import { Annotation, END, START, StateGraph } from '@langchain/langgraph';
import { nlqMulti } from './db';
import { queryLocalBridge, type LocalBridgeQueryResult } from './localBridge';
import { getCouncilMember, type CouncilMember } from './council';
import type { UserProfileContext } from './profile';
import type { AgentArtifact, AgentWorkflowTrace, NlqResponse } from '../types';

export interface LangGraphAgentOptions {
  profile?: UserProfileContext;
  councilMemberId?: string | null;
}

export interface LangGraphAgentState {
  query: string;
  profile: UserProfileContext | null;
  councilMember: CouncilMember | null;
  mode: 'relationship' | 'local-bridge';
  response: NlqResponse | null;
  artifact: AgentArtifact | null;
  assistantReply: string;
  workflowTrace: AgentWorkflowTrace;
}

const AgentStateAnnotation = Annotation.Root({
  query: Annotation(),
  profile: Annotation(),
  councilMember: Annotation(),
  mode: Annotation(),
  response: Annotation(),
  artifact: Annotation(),
  assistantReply: Annotation(),
  workflowTrace: Annotation(),
});

function buildRelationshipArtifact(response: NlqResponse, query: string): AgentArtifact {
  switch (response.intentType) {
    case 'searchPeople':
      return {
        id: `artifact-${Date.now()}`,
        kind: 'search',
        title: '联系人检索结果',
        summary: `针对“${query}”返回了 ${response.results.length} 条结果。`,
        results: response.results,
      };
    case 'createPersonDraft':
    case 'updatePersonDraft':
    case 'addInteractionDraft':
      return {
        id: `artifact-${Date.now()}`,
        kind: 'draft',
        title: '待确认草稿',
        summary: '系统已生成一条结构化草稿，等待你确认后再写入。',
        response,
      };
    case 'findPath':
      return {
        id: `artifact-${Date.now()}`,
        kind: 'path',
        title: '路径建议',
        summary: response.path.summary,
        path: response.path,
      };
    default:
      return {
        id: `artifact-${Date.now()}`,
        kind: 'context',
        title: '上下文摘要',
        summary: '系统已生成上下文摘要，继续对话即可补充更多信息。',
        context: query,
      };
  }
}

function buildLocalBridgeArtifact(query: string, results: LocalBridgeQueryResult[]): AgentArtifact {
  const summary = results.map((item) => `${item.title}: ${item.summary}`).join('；');

  return {
    id: `artifact-${Date.now()}`,
    kind: 'context',
    title: '本地桥接结果',
    summary: `针对“${query}”返回了 ${results.length} 条受控摘要。`,
    context: summary,
  };
}

function renderRelationshipReply(response: NlqResponse, query: string): string {
  switch (response.intentType) {
    case 'searchPeople':
      return `我根据“${query}”检索到了 ${response.results.length} 位相关联系人。你可以继续点开详情或进一步缩小范围。`;
    case 'createPersonDraft':
      return '我已生成一个新增联系人草稿，下面可以直接确认或修改字段。';
    case 'updatePersonDraft':
      return '我已生成一个联系人更新草稿，下面可以确认要改动的字段。';
    case 'addInteractionDraft':
      return '我已生成一条互动记录草稿，下面可以校对联系人、摘要和待办。';
    case 'findPath':
      return '我已经帮你规划了一条可行路径，下面是推荐链路和说明。';
    default:
      return '我已经把这条请求归类为可继续处理的上下文，接下来你可以继续补充信息。';
  }
}

function renderLocalBridgeReply(query: string): string {
  return `我把“${query}”交给本地桥接层处理，当前只返回脱敏摘要并保留待确认边界。`;
}

function applyCouncilFraming(reply: string, councilMember: CouncilMember | null): string {
  if (!councilMember) return reply;
  return `【${councilMember.name} · ${councilMember.role}】${reply} ${councilMember.skill}。`;
}

function applyProfileConstraints(reply: string, profile: UserProfileContext | null): string {
  if (!profile || profile.constraints.length === 0) return reply;
  return `${reply}（画像约束：${profile.constraints[0]}）`;
}

function buildWorkflowTrace(mode: LangGraphAgentState['mode'], step: string): AgentWorkflowTrace {
  const policy = mode === 'local-bridge'
    ? '本地桥接默认只读摘要，写入前强制二次确认'
    : '关系查询优先生成草稿和确认流，避免直接写入';

  return {
    mode,
    steps: [step],
    policy,
  };
}

const workflow = new StateGraph(AgentStateAnnotation)
  .addNode('classifyQuery', async (state: LangGraphAgentState) => {
    const text = state.query.toLowerCase();
    const isLocal = text.includes('本地') || text.includes('高敏感') || text.includes('桥接');
    const mode = isLocal ? 'local-bridge' : 'relationship';
    const step = isLocal ? '已识别为本地桥接请求' : '已识别为关系查询请求';
    return {
      mode,
      workflowTrace: buildWorkflowTrace(mode, step),
    };
  })
  .addNode('executeQuery', async (state: LangGraphAgentState) => {
    const trace = state.workflowTrace;
    const nextSteps = [...trace.steps];

    if (state.mode === 'local-bridge') {
      const results = await queryLocalBridge({ query: state.query, mode: 'confirm-first' });
      const reply = applyProfileConstraints(
        applyCouncilFraming(renderLocalBridgeReply(state.query), state.councilMember),
        state.profile,
      );
      nextSteps.push('已执行本地桥接：返回脱敏摘要与待确认边界');
      return {
        artifact: buildLocalBridgeArtifact(state.query, results),
        assistantReply: reply,
        workflowTrace: {
          mode: state.mode,
          steps: nextSteps,
          policy: trace.policy,
        },
      };
    }

    const response = await nlqMulti(state.query, false);
    const reply = applyProfileConstraints(
      applyCouncilFraming(renderRelationshipReply(response, state.query), state.councilMember),
      state.profile,
    );
    nextSteps.push('已执行关系查询：生成检索/草稿/路径结果');
    return {
      response,
      artifact: buildRelationshipArtifact(response, state.query),
      assistantReply: reply,
      workflowTrace: {
        mode: state.mode,
        steps: nextSteps,
        policy: trace.policy,
      },
    };
  })
  .addEdge(START, 'classifyQuery')
  .addEdge('classifyQuery', 'executeQuery')
  .addEdge('executeQuery', END);

const app = workflow.compile();

export async function runLangGraphWorkflow(query: string, options: LangGraphAgentOptions = {}): Promise<LangGraphAgentState> {
  const councilMember = getCouncilMember(options.councilMemberId);
  const result = await app.invoke({
    query,
    profile: options.profile ?? null,
    councilMember,
  });
  return {
    query,
    profile: (result.profile as UserProfileContext | null) ?? null,
    councilMember: (result.councilMember as CouncilMember | null) ?? null,
    mode: (result.mode as LangGraphAgentState['mode']) ?? 'relationship',
    response: result.response as NlqResponse | null,
    artifact: result.artifact as AgentArtifact | null,
    assistantReply: result.assistantReply as string,
    workflowTrace: (result.workflowTrace as AgentWorkflowTrace) ?? {
      mode: 'relationship',
      steps: ['未记录工作流'],
      policy: '默认策略',
    },
  };
}
