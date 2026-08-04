export interface LocalBridgeQueryResult {
  source: 'local-bridge';
  title: string;
  summary: string;
  details: string[];
  sensitivity: 'high' | 'medium';
  permissions: string[];
  nextAction: string;
  mode: 'readonly' | 'confirm-first';
}

export interface LocalBridgeQueryRequest {
  query: string;
  mode: 'readonly' | 'confirm-first';
}

function buildResponses(mode: LocalBridgeQueryRequest['mode']): LocalBridgeQueryResult[] {
  if (mode === 'confirm-first') {
    return [
      {
        source: 'local-bridge',
        title: '高敏感联系人摘要',
        summary: '本地桥接返回一条脱敏摘要，并标明下一步需要用户确认后再继续。',
        details: ['仅返回摘要，不暴露完整通讯录', '需要用户再次确认后才允许进入写入流程', '默认维持高敏感级别'],
        sensitivity: 'high',
        permissions: ['只读摘要', '二次确认后允许写入草稿'],
        nextAction: '请确认后再生成正式写入草稿。',
        mode,
      },
      {
        source: 'local-bridge',
        title: '待确认本地路径',
        summary: '路径建议已生成，但仍保留在本地桥接层，等待明确确认。',
        details: ['路径节点来自本地关系索引', '会以待确认草稿展示', '不会直接写入长期记忆'],
        sensitivity: 'high',
        permissions: ['只读路径预览', '确认后才进入写入'],
        nextAction: '用户确认后才允许把路径建议转为正式操作。',
        mode,
      },
    ];
  }

  return [
    {
      source: 'local-bridge',
      title: '高敏感联系人摘要',
      summary: '从本地受控桥接中返回一条只读摘要，表明当前请求命中了高敏感上下文。',
      details: ['仅允许白名单只读查询', '不会直接暴露完整通讯录', '结果默认呈现为脱敏摘要'],
      sensitivity: 'high',
      permissions: ['只读查询', '禁止直接导出完整字段'],
      nextAction: '继续以脱敏摘要参与全局排序。',
      mode,
    },
    {
      source: 'local-bridge',
      title: '本地待确认路径',
      summary: '本地桥接为路径预览生成一个待确认草稿，用户确认后才进入写入阶段。',
      details: ['路径节点来自本地关系索引', '需要用户再次确认后才写入', '默认维持高敏感级别'],
      sensitivity: 'high',
      permissions: ['只读预览', '待确认后再进入写入'],
      nextAction: '把路径建议作为草稿保存在消息流中。',
      mode,
    },
  ];
}

export async function queryLocalBridge(request: LocalBridgeQueryRequest): Promise<LocalBridgeQueryResult[]> {
  return new Promise((resolve) => {
    setTimeout(() => {
      const matched = request.query.toLowerCase().includes('高敏感') || request.query.toLowerCase().includes('本地')
        ? buildResponses(request.mode)
        : [buildResponses(request.mode)[0]];
      resolve(matched);
    }, 200);
  });
}
