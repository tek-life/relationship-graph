export type IntentKind = 'chat' | 'relationship' | 'uncertain';

export interface IntentClassification {
  kind: IntentKind;
  reason: string;
  confidence: number;
}

const RELATIONSHIP_HINTS = [
  '联系人',
  '联系',
  '关系',
  '人脉',
  '名片',
  '跟进',
  '互动',
  '备注',
  '记住',
  '更新',
  '添加',
  '删除',
  '关系网',
  '关系网络',
  '介绍',
  '同事',
  '朋友',
  '合作',
  '资源',
  '通讯录',
  '资源标签',
  '最近联系',
  '上次聊',
  '还没跟进',
  '谁在',
  '和我关系',
  '帮我找',
  '查一下',
  '查找',
  '查询',
  '谁能',
  '有什么',
  '有谁',
  '哪个',
  '哪些',
];

const CHAT_HINTS = [
  '什么',
  '为什么',
  '怎么',
  '如何',
  '哪里',
  '天气',
  '写一封',
  '帮我写',
  '解释',
  '总结',
  '建议',
  '计划',
  '帮我想',
  '你觉得',
  '请问',
  '聊聊',
  '最近',
  '今天',
  '明天',
  '以后',
  '能不能',
  '是不是',
];

export function classifyUserIntent(query: string): IntentClassification {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return {
      kind: 'uncertain',
      reason: '请先输入内容，我再帮你判断是聊天还是维护联系人。',
      confidence: 0.2,
    };
  }

  const hasRelationshipHint = RELATIONSHIP_HINTS.some((hint) => normalized.includes(hint.toLowerCase()));
  const hasChatHint = CHAT_HINTS.some((hint) => normalized.includes(hint.toLowerCase()));

  const isShort = normalized.split(/\s+/).filter(Boolean).length <= 3;
  const containsQuestionMark = normalized.includes('?');

  if (hasRelationshipHint && !hasChatHint) {
    return {
      kind: 'relationship',
      reason: '看起来像是在查询或维护联系人信息。',
      confidence: 0.86,
    };
  }

  if (hasChatHint && !hasRelationshipHint) {
    return {
      kind: 'chat',
      reason: '看起来像是在做通用聊天或提问。',
      confidence: 0.81,
    };
  }

  if (isShort && containsQuestionMark) {
    return {
      kind: 'uncertain',
      reason: '这条内容太短，我不确定是聊天还是联系人维护，请你帮我确认一下。',
      confidence: 0.45,
    };
  }

  if (hasRelationshipHint && hasChatHint) {
    return {
      kind: 'uncertain',
      reason: '这条内容同时带有聊天和关系线索，我先请你确认要走哪一路。',
      confidence: 0.58,
    };
  }

  return {
    kind: 'chat',
    reason: '我先按通用聊天处理。',
    confidence: 0.7,
  };
}
