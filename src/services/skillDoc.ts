// SKILL Markdown 文档工具：frontmatter 解析/序列化与必填校验（纯函数，无外部依赖）；
// 另含技能包（多文件）清单构造与导入预检纯函数。

// ==================== 技能包清单辅助（纯函数） ====================

/** 后端强制的校验上限（前端提示用） */
export const PACKAGE_LIMITS = {
  /** 文件数上限 */
  maxFiles: 50,
  /** 单文件字符数上限（200KB 按字符估算） */
  maxSingleFileChars: 200_000,
  /** 整包总字符数上限 */
  maxTotalChars: 1_000_000,
} as const;

/** zip 导入防护上限（zip 炸弹防护，与后端 body limit 口径协调） */
export const ZIP_LIMITS = {
  /** zip 文件本身大小上限（选文件时超限直接报错） */
  maxZipBytes: 20 * 1024 * 1024, // 20MB
  /** 解压后整包原始字节上限（按条目累计，超限中止） */
  maxUncompressedBytes: 8 * 1024 * 1024, // 8MB
} as const;

/** 视为文本文件的扩展名（zip 导入时只取这些文件，跳过二进制） */
export const TEXT_FILE_EXTENSIONS = [
  '.md',
  '.markdown',
  '.txt',
  '.json',
  '.yaml',
  '.yml',
  '.toml',
  '.csv',
  '.xml',
  '.html',
  '.css',
  '.js',
  '.ts',
  '.mjs',
  '.py',
  '.sh',
] as const;

/** 判断相对路径是否为文本文件（按扩展名白名单，大小写不敏感） */
export function isTextFilePath(relPath: string): boolean {
  const lower = relPath.toLowerCase();
  return TEXT_FILE_EXTENSIONS.some((ext) => lower.endsWith(ext));
}

/** 解压结果中的单个条目（content 为已解码文本） */
export interface DecompressedEntry {
  relPath: string;
  content: string;
}

/**
 * 从解压结果构造技能包文件清单：
 * - 跳过目录条目（以 / 结尾或 content 为 null/undefined）
 * - 归一化路径（去掉开头的 ./ 与多余 /）
 * - 只保留文本文件扩展名，过滤二进制
 * - 同名路径去重（保留后者），按 relPath 字典序排序
 */
export function extractPackageFileList(
  entries: Array<DecompressedEntry | { relPath: string; content: string | null }>,
): { relPath: string; content: string }[] {
  const map = new Map<string, string>();
  for (const entry of entries) {
    if (entry.content === null || entry.content === undefined) continue;
    const relPath = normalizeRelPath(entry.relPath);
    if (!relPath || relPath.endsWith('/')) continue;
    if (!isTextFilePath(relPath)) continue;
    map.set(relPath, entry.content);
  }
  return [...map.entries()]
    .map(([relPath, content]) => ({ relPath, content }))
    .sort((a, b) => (a.relPath < b.relPath ? -1 : a.relPath > b.relPath ? 1 : 0));
}

/** 路径归一化：去 BOM 安全前缀 ./、折叠重复 / */
export function normalizeRelPath(relPath: string): string {
  let p = relPath.replace(/\\/g, '/').trim();
  while (p.startsWith('./')) p = p.slice(2);
  p = p.replace(/\/{2,}/g, '/');
  return p;
}

/** 所有根 SKILL.md 候选路径（文件名大小写不敏感、任意深度、去重） */
export function rootSkillMdCandidates(relPaths: string[]): string[] {
  const seen = new Set<string>();
  const candidates: string[] = [];
  for (const relPath of relPaths) {
    const p = normalizeRelPath(relPath);
    if (!p || p.endsWith('/')) continue;
    const base = p.split('/').pop() ?? '';
    if (base.toLowerCase() !== 'skill.md') continue;
    if (!seen.has(p)) {
      seen.add(p);
      candidates.push(p);
    }
  }
  return candidates;
}

/** 从候选中取最浅层（'/' 计数最小）唯一入口；同层多个视为歧义返回 null */
function pickRootSkillMd(candidates: string[]): string | null {
  if (candidates.length === 0) return null;
  const depth = (p: string) => p.split('/').length - 1;
  const shallowest = Math.min(...candidates.map(depth));
  const hits = candidates.filter((p) => depth(p) === shallowest);
  return hits.length === 1 ? hits[0] : null;
}

/**
 * 定位根 SKILL.md（与后端口径对齐）：
 * - 文件名大小写不敏感（Skill.md / skill.md 均命中）
 * - 取任意深度中最浅层（'/' 计数最小）
 * - 最浅层存在多个候选时视为歧义，返回 null（由预检报错）
 */
export function findRootSkillMd(relPaths: string[]): string | null {
  return pickRootSkillMd(rootSkillMdCandidates(relPaths));
}

/** 技能包导入预检结果 */
export interface SkillPackagePreview {
  fileCount: number;
  totalChars: number;
  /** 根 SKILL.md 相对路径；未找到或歧义时为 null */
  rootSkillMdRelPath: string | null;
  hasRootSkillMd: boolean;
  /** 最浅层存在多个根 SKILL.md 候选（歧义，需人工处理） */
  rootSkillMdAmbiguous: boolean;
  /** 根 SKILL.md frontmatter 校验错误（name/description 非空）；通过为 null */
  frontmatterError: string | null;
  /** 超过文件数上限 */
  overFileLimit: boolean;
  /** 超过总字符上限 */
  overCharLimit: boolean;
  /** 超过单文件上限的文件路径列表 */
  oversizedFiles: string[];
  /** 任一预检失败 */
  hasBlockingIssue: boolean;
}

/** 对文件清单做导入预检：统计文件数/总字符、定位根 SKILL.md、标记超限 */
export function analyzeSkillPackage(
  files: { relPath: string; content: string }[],
): SkillPackagePreview {
  const totalChars = files.reduce((sum, f) => sum + f.content.length, 0);
  const oversizedFiles = files
    .filter((f) => f.content.length > PACKAGE_LIMITS.maxSingleFileChars)
    .map((f) => f.relPath);
  const candidates = rootSkillMdCandidates(files.map((f) => f.relPath));
  const rootSkillMdRelPath = pickRootSkillMd(candidates);
  const rootSkillMdAmbiguous = rootSkillMdRelPath === null && candidates.length > 0;
  let frontmatterError: string | null = null;
  if (rootSkillMdRelPath !== null) {
    const rootFile = files.find((f) => normalizeRelPath(f.relPath) === rootSkillMdRelPath);
    if (rootFile) frontmatterError = validateFrontmatter(rootFile.content);
  }
  const overFileLimit = files.length > PACKAGE_LIMITS.maxFiles;
  const overCharLimit = totalChars > PACKAGE_LIMITS.maxTotalChars;
  return {
    fileCount: files.length,
    totalChars,
    rootSkillMdRelPath,
    hasRootSkillMd: rootSkillMdRelPath !== null,
    rootSkillMdAmbiguous,
    frontmatterError,
    overFileLimit,
    overCharLimit,
    oversizedFiles,
    hasBlockingIssue:
      rootSkillMdRelPath === null ||
      overFileLimit ||
      overCharLimit ||
      oversizedFiles.length > 0 ||
      frontmatterError !== null,
  };
}

/** 字符数格式化（千位逗号） */
export function formatCharCount(n: number): string {
  return n.toLocaleString('en-US');
}

// ==================== frontmatter 工具 ====================

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
    // 仅解析非缩进行（平铺 key: value）；缩进行视为嵌套内容，跳过
    if (idx > 0 && !/^\s/.test(lines[i])) {
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
