// 端到端导入验证：模拟前端链路（解析 xlsx → 字段映射 → 清洗 → preview → commit）
// 运行：node scripts/e2e-import-test.mjs [baseUrl]
// 默认打到 http://localhost:8791（独立测试实例，勿指向正式库）

import * as XLSX from 'xlsx';
import { readFileSync } from 'node:fs';

const BASE = process.argv[2] ?? 'http://localhost:8791';
const FILE = 'test-data/联系人测试数据_1000.xlsx';

// ---- 与 src/services/importer.ts 一致的清洗规则 ----
const STRENGTH_MAP = { strong: 'strong', 强: 'strong', 很熟: 'strong', 铁: 'strong', 密切: 'strong', medium: 'medium', 中: 'medium', 一般: 'medium', 还行: 'medium', 熟: 'medium', weak: 'weak', 弱: 'weak', 不熟: 'weak', 泛泛: 'weak', 待激活: 'weak' };
const SENSITIVITY_MAP = { high: 'high', 高: 'high', 保密: 'high', medium: 'medium', 中: 'medium', low: 'low', 低: 'low', 公开: 'low' };
const STATUS_MAP = { 'follow-up': 'follow-up', 待跟进: 'follow-up', 跟进: 'follow-up', active: 'active', 活跃: 'active', 正常: 'active', 已合作: 'active', cold: 'cold', 冷却: 'cold', 冷: 'cold' };
const cleanPhone = (v) => v.replace(/[\s-]/g, '').replace(/^\+?86/, '');
const splitList = (v) => Array.from(new Set(v.split(/[、，,/;；\s]+/).map((s) => s.trim()).filter(Boolean)));

function normalize(record) {
  const get = (key) => String(record[key] ?? '').trim();
  return {
    name: get('姓名'),
    aliases: [],
    phone: cleanPhone(get('手机号')) || null,
    email: get('邮箱') || null,
    company: get('公司') || null,
    title: get('职务') || null,
    location: get('城市') || null,
    background: get('怎么认识的') || null,
    relationshipStrength: STRENGTH_MAP[get('关系')] ?? null,
    resourceTags: splitList(get('行业标签')),
    sensitivityLevel: SENSITIVITY_MAP[get('敏感级别')] ?? 'low',
    status: STATUS_MAP[get('跟进状态')],
    nextStep: get('下一步') || null,
    notes: get('备注') || null,
  };
}

async function post(path, body, token) {
  const res = await fetch(`${BASE}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', ...(token ? { Authorization: `Bearer ${token}` } : {}) },
    body: JSON.stringify(body),
  });
  const json = await res.json();
  if (!res.ok) throw new Error(`${path} ${res.status}: ${JSON.stringify(json)}`);
  return json;
}

// 1. 解析
const workbook = XLSX.read(readFileSync(FILE), { type: 'buffer' });
const sheet = workbook.Sheets[workbook.SheetNames[0]];
const records = XLSX.utils.sheet_to_json(sheet, { raw: false, defval: '' });
const rows = records.map(normalize);
console.log(`解析行数: ${rows.length}`);

// 2. 解锁测试库
const state = await fetch(`${BASE}/api/auth/state`).then((r) => r.json());
const password = 'import-test-123';
const { token } = state.initialized
  ? await post('/api/auth/unlock', { password })
  : await post('/api/auth/setup', { password });
console.log('已解锁测试库');

// 3. 预检
let t0 = Date.now();
const preview = await post('/api/import/preview', { rows }, token);
console.log(`预检耗时: ${Date.now() - t0}ms`);
console.log(`  total=${preview.total} valid=${preview.valid} invalid=${preview.invalid.length} duplicates=${preview.duplicates.length}`);
const exact = preview.duplicates.filter((d) => d.matchType === 'exact');
const nameOnly = preview.duplicates.filter((d) => d.matchType === 'name_only');
console.log(`  exact重复=${exact.length} 同名=${nameOnly.length}`);

// 4. 提交（默认跳过 exact 重复 + 无效行，同前端默认策略）
const skip = new Set();
exact.forEach((d) => skip.add(d.index));
preview.invalid.forEach((d) => skip.add(d.index));
t0 = Date.now();
const commit = await post('/api/import/persons', { rows, skipIndices: Array.from(skip) }, token);
console.log(`提交耗时: ${Date.now() - t0}ms（服务端 ${commit.elapsedMs}ms）`);
console.log(`  imported=${commit.imported} skipped=${commit.skipped} failed=${commit.failed.length}`);

// 5. 校验落库数量
const persons = await fetch(`${BASE}/api/persons`, { headers: { Authorization: `Bearer ${token}` } }).then((r) => r.json());
console.log(`库内联系人总数: ${persons.length}`);

// 6. 断言
const expectImported = rows.length - skip.size;
if (commit.imported !== expectImported) throw new Error(`导入数不符: ${commit.imported} != ${expectImported}`);
if (persons.length < commit.imported) throw new Error('落库数量小于导入数');
if (preview.invalid.length !== 10) console.warn(`警告: 无效行应为 10, 实际 ${preview.invalid.length}`);
console.log('✅ 端到端验证通过');
