// SKILL Markdown 文档工具：frontmatter 解析/序列化与必填校验（纯函数，无外部依赖）

export interface ParsedSkillDoc {
  /** frontmatter 平铺键值对（值为去除首尾引号后的字符串） */
  meta: Record<string, string>;
  /** frontmatter 之后的正文 */
  body: string;
}

/** 解析以 `---` 分隔的 frontmatter（平铺 key: value）；无 frontmatter 时 meta 为空 */
export function parseFrontmatter(md: string): ParsedSkillDoc {
  const text = md.replace(/^\uFEFF/, '');
  const lines = text.split(/\r?\n/);
  if ((lines[0] ?? '').trim() !== '---') return { meta: {}, body: text };
  const meta: Record<string, string> = {};
  for (let i = 1; i < lines.length; i++) {
    if (lines[i].trim() === '---') {
      return { meta, body: lines.slice(i + 1).join('\n').replace(/^\n+/, '') };
    }
    const idx = lines[i].indexOf(':');
    if (idx > 0) {
      const key = lines[i].slice(0, idx).trim();
      if (key) meta[key] = lines[i].slice(idx + 1).trim().replace(/^["']|["']$/g, '');
    }
  }
  // 未找到闭合 `---`：视为无 frontmatter，整体作为正文
  return { meta: {}, body: text };
}

/** 将 meta 与正文序列化为完整 SKILL Markdown 文档；meta 为空时不输出 frontmatter */
export function serializeFrontmatter(meta: Record<string, string>, body: string): string {
  const entries = Object.entries(meta).filter(([, v]) => v.trim());
  if (entries.length === 0) return body.replace(/^\n+/, '');
  const fm = `---\n${entries.map(([k, v]) => `${k}: ${v}`).join('\n')}\n---\n\n`;
  return fm + body.replace(/^\n+/, '');
}

/** 校验 frontmatter 必填字段（name/description）；返回中文错误原因，通过时返回 null */
export function validateFrontmatter(md: string): string | null {
  if ((md.trimStart().split(/\r?\n/)[0] ?? '').trim() !== '---') {
    return 'SKILL 文档缺少 frontmatter 头部（以 --- 开始）';
  }
  const { meta } = parseFrontmatter(md);
  if (!meta.name) return 'frontmatter 缺少必填字段 name';
  if (!meta.description) return 'frontmatter 缺少必填字段 description';
  return null;
}

/** 从 SKILL Markdown 的 frontmatter 提取 description（用于列表摘要），无则返回空串 */
export function extractSkillDescription(md?: string | null): string {
  if (!md) return '';
  return parseFrontmatter(md).meta.description ?? '';
}

/** 新建技能时的一键模板（Claude 风格：frontmatter + 用途/步骤章节） */
export const SKILL_TEMPLATE = `---
name: 技能名称
description: 一句话说明该技能的用途与触发时机
---

# 技能名称

## 用途

说明该技能解决什么问题、适用于哪些场景。

## 步骤

1. 第一步：理解用户输入并确认意图。
2. 第二步：调用相应能力或组织回复。
3. 第三步：输出结果并给出后续建议。

## 注意事项

- 补充边界条件、失败兜底与敏感数据处理约定。
`;
