export interface ExtractedEntities {
  persons: { mention: string; confidence: number }[];
  topics: string[];
  actionItems: string[];
  summary: string;
}

const FALLBACK_EXTRACTION: ExtractedEntities = {
  persons: [],
  topics: [],
  actionItems: [],
  summary: '',
};

// 实际部署的模型是 qwen2.5:7b（此前写成 qwen2:7b 导致请求 404、提取永远走降级）。
const OLLAMA_MODEL = 'qwen2.5:7b';
const OLLAMA_TIMEOUT_MS = 30_000;

export async function extractFromText(text: string): Promise<ExtractedEntities> {
  const started = performance.now();
  const textLength = text.length;
  console.info('[ollama] extract_start', { textLength, model: OLLAMA_MODEL });

  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), OLLAMA_TIMEOUT_MS);

  try {
    const response = await fetch(`http://${window.location.hostname}:11434/api/generate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      signal: controller.signal,
      body: JSON.stringify({
        model: OLLAMA_MODEL,
        stream: false,
        format: 'json',
        prompt: `请从下面沟通记录中提取信息，只输出 JSON：
{
  "persons": [{"mention": "称呼或姓名", "confidence": 0.9}],
  "topics": ["话题"],
  "actionItems": ["待办"],
  "summary": "一句话摘要"
}

沟通记录：${text}`,
      }),
    });

    if (!response.ok) {
      throw new Error(`Ollama 请求失败：${response.status}`);
    }

    const data = await response.json();
    const parsed = JSON.parse(data.response || '{}');
    const result = {
      persons: Array.isArray(parsed.persons) ? parsed.persons : [],
      topics: Array.isArray(parsed.topics) ? parsed.topics : [],
      actionItems: Array.isArray(parsed.actionItems)
        ? parsed.actionItems
        : Array.isArray(parsed.action_items)
          ? parsed.action_items
          : [],
      summary: typeof parsed.summary === 'string' ? parsed.summary : text.slice(0, 80),
    };
    console.info('[ollama] extract_success', {
      textLength,
      personMentionCount: result.persons.length,
      topicCount: result.topics.length,
      actionItemCount: result.actionItems.length,
      summaryLength: result.summary.length,
      elapsedMs: Math.round(performance.now() - started),
    });
    return result;
  } catch (error) {
    console.warn('[ollama] extract_fallback', {
      textLength,
      elapsedMs: Math.round(performance.now() - started),
      error: error instanceof Error ? error.message : String(error),
    });
    return {
      ...FALLBACK_EXTRACTION,
      summary: text.slice(0, 80),
    };
  } finally {
    clearTimeout(timeoutId);
  }
}
