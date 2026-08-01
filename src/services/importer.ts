// Excel/CSV 导入：解析、字段映射猜测、清洗归一化与导入 API 调用。
// 文件在浏览器内解析（SheetJS），不落服务端磁盘；服务端只接收结构化行。

import * as XLSX from 'xlsx';
import { apiPost } from './api';
import type { CreatePersonInput, PersonStatus, RelationshipStrength, SensitivityLevel } from '../types';

export interface ParsedSheet {
  headers: string[];
  rows: string[][];
}

export interface RowIssue {
  index: number;
  reason: string;
}

export interface DuplicateInfo {
  index: number;
  matchType: 'exact' | 'name_only';
  source: 'db' | 'batch';
}

export interface ImportPreviewResult {
  total: number;
  valid: number;
  invalid: RowIssue[];
  duplicates: DuplicateInfo[];
}

export interface ImportCommitResult {
  imported: number;
  skipped: number;
  failed: RowIssue[];
  elapsedMs: number;
}

/** 目标字段定义（顺序即映射下拉框顺序） */
export const FIELD_DEFS: { key: string; label: string }[] = [
  { key: 'name', label: '姓名（必填）' },
  { key: 'aliases', label: '别名/昵称' },
  { key: 'phone', label: '电话' },
  { key: 'email', label: '邮箱' },
  { key: 'company', label: '公司/单位' },
  { key: 'title', label: '职位' },
  { key: 'location', label: '城市' },
  { key: 'background', label: '认识背景' },
  { key: 'relationshipStrength', label: '关系强度' },
  { key: 'resourceTags', label: '资源标签' },
  { key: 'sensitivityLevel', label: '敏感级别' },
  { key: 'status', label: '状态' },
  { key: 'nextStep', label: '下一步' },
  { key: 'notes', label: '备注' },
];

export const MAP_TO_NOTES = '__notes__';
export const MAP_IGNORE = '__ignore__';

/** 表头 → 字段自动猜测词典 */
const HEADER_DICTIONARY: Record<string, string[]> = {
  name: ['姓名', '名字', '联系人', 'name'],
  aliases: ['别名', '昵称', '代称'],
  phone: ['电话', '手机', '手机号', '联系电话', '联系方式', 'tel', 'mobile', 'phone'],
  email: ['邮箱', 'email', 'mail'],
  company: ['公司', '单位', '企业', '组织'],
  title: ['职位', '职务', '头衔', '角色', '岗位'],
  location: ['城市', '地区', '地域', '所在地', '地址'],
  background: ['认识背景', '怎么认识', '认识渠道', '来源'],
  relationshipStrength: ['关系强度', '关系', '亲密度', '熟悉程度'],
  resourceTags: ['标签', '资源', '能力', '行业', 'tag'],
  sensitivityLevel: ['敏感', '保密'],
  status: ['状态', '跟进状态'],
  nextStep: ['下一步', '待办', '跟进计划', 'next'],
  notes: ['备注', '说明', '其他', 'note', 'remark'],
};

export async function parseSheetFile(file: File): Promise<ParsedSheet> {
  const buffer = await file.arrayBuffer();
  const workbook = XLSX.read(buffer, { type: 'array' });
  const sheet = workbook.Sheets[workbook.SheetNames[0]];
  const matrix = XLSX.utils.sheet_to_json<unknown[]>(sheet, { header: 1, raw: false, defval: '' });
  if (matrix.length === 0) {
    return { headers: [], rows: [] };
  }
  const headers = (matrix[0] as unknown[]).map((cell) => String(cell ?? '').trim());
  const rows = matrix
    .slice(1)
    .map((row) => headers.map((_, col) => String((row as unknown[])[col] ?? '').trim()))
    .filter((row) => row.some((cell) => cell !== ''));
  return { headers, rows };
}

/** 按词典猜测每个表头对应的目标字段；猜不中的默认忽略 */
export function guessMapping(headers: string[]): string[] {
  const used = new Set<string>();
  return headers.map((header) => {
    const normalized = header.toLowerCase();
    for (const [field, keywords] of Object.entries(HEADER_DICTIONARY)) {
      if (used.has(field)) continue;
      if (keywords.some((keyword) => normalized.includes(keyword.toLowerCase()))) {
        used.add(field);
        return field;
      }
    }
    return MAP_IGNORE;
  });
}

const STRENGTH_MAP: Record<string, RelationshipStrength> = {
  strong: 'strong', 强: 'strong', 很熟: 'strong', 铁: 'strong', 密切: 'strong',
  medium: 'medium', 中: 'medium', 一般: 'medium', 还行: 'medium', 熟: 'medium',
  weak: 'weak', 弱: 'weak', 不熟: 'weak', 泛泛: 'weak', 待激活: 'weak',
};

const SENSITIVITY_MAP: Record<string, SensitivityLevel> = {
  high: 'high', 高: 'high', 保密: 'high', 高敏感: 'high',
  medium: 'medium', 中: 'medium', 中敏感: 'medium',
  low: 'low', 低: 'low', 公开: 'low', 低敏感: 'low',
};

const STATUS_MAP: Record<string, PersonStatus> = {
  'follow-up': 'follow-up', 待跟进: 'follow-up', 跟进: 'follow-up', 待联系: 'follow-up',
  active: 'active', 活跃: 'active', 正常: 'active', 已合作: 'active',
  cold: 'cold', 冷却: 'cold', 冷: 'cold', 沉寂: 'cold',
};

function cleanPhone(value: string): string {
  return value.replace(/[\s-]/g, '').replace(/^\+?86/, '');
}

function splitList(value: string): string[] {
  return Array.from(
    new Set(
      value
        .split(/[、，,/;；\s]+/)
        .map((item) => item.trim())
        .filter(Boolean),
    ),
  );
}

/** 将原始行按映射清洗归一化为标准联系人输入 */
export function normalizeRows(rows: string[][], mapping: string[], headers: string[]): CreatePersonInput[] {
  return rows.map((row) => {
    const record: CreatePersonInput = {
      name: '',
      aliases: [],
      resourceTags: [],
      sensitivityLevel: 'low',
    };
    const noteParts: string[] = [];

    mapping.forEach((field, col) => {
      const value = (row[col] ?? '').trim();
      if (!value || field === MAP_IGNORE) return;
      switch (field) {
        case 'name': record.name = value; break;
        case 'aliases': record.aliases = splitList(value); break;
        case 'phone': record.phone = cleanPhone(value); break;
        case 'email': record.email = value; break;
        case 'company': record.company = value; break;
        case 'title': record.title = value; break;
        case 'location': record.location = value; break;
        case 'background': record.background = value; break;
        case 'relationshipStrength': record.relationshipStrength = STRENGTH_MAP[value] ?? null; break;
        case 'resourceTags': record.resourceTags = splitList(value); break;
        case 'sensitivityLevel': record.sensitivityLevel = SENSITIVITY_MAP[value] ?? 'low'; break;
        case 'status': record.status = STATUS_MAP[value]; break;
        case 'nextStep': record.nextStep = value; break;
        case 'notes': noteParts.push(value); break;
        case MAP_TO_NOTES: noteParts.push(`${headers[col]}：${value}`); break;
        default: break;
      }
    });

    if (noteParts.length > 0) {
      record.notes = noteParts.join('\n');
    }
    return record;
  });
}

export async function importPreview(rows: CreatePersonInput[]): Promise<ImportPreviewResult> {
  return apiPost<ImportPreviewResult>('/api/import/preview', { rows });
}

export async function importCommit(rows: CreatePersonInput[], skipIndices: number[]): Promise<ImportCommitResult> {
  return apiPost<ImportCommitResult>('/api/import/persons', { rows, skipIndices });
}
