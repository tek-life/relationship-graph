// 演示数据种子脚本：通过后端 HTTP API 只新增（不改不删）演示联系人/互动/关系，
// 使首页三条示例 NLQ 查询均有结果。可重复运行（同名演示联系人已存在则跳过）。
//
// 运行：RG_SEED_PASSWORD=<主密码> node scripts/seed-demo-data.mjs [baseUrl]
// 默认打到 http://localhost:8790（正式库）。密码仅用于解锁换取 token，不落盘不打印。

const BASE = process.argv[2] ?? process.env.RG_BASE ?? 'http://localhost:8790';
const PASSWORD = process.env.RG_SEED_PASSWORD;

if (!PASSWORD) {
  console.error('缺少主密码：请以 RG_SEED_PASSWORD=<主密码> 环境变量运行');
  process.exit(1);
}

async function req(method, path, body, token) {
  const res = await fetch(`${BASE}${path}`, {
    method,
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = await res.json();
  if (!res.ok) throw new Error(`${method} ${path} ${res.status}: ${JSON.stringify(json)}`);
  return json;
}

const daysAgo = (n) => new Date(Date.now() - n * 24 * 60 * 60 * 1000).toISOString();

// ---- 演示数据定义 ----
// 覆盖三条示例查询（nlq.rs 白名单会同时触发 location/resourceTags/topics/status/strength 过滤）：
// ① 上海+地产+strong/medium，且有含"地产"的互动：张演示、李示例、陈样本
// ② 待跟进 + resourceTags 含"融资" + 互动 topics 含"融资"：李示例、王样本
// ③ resourceTags 含"投标" + 互动 topics 含"懂车帝"/"投标"：赵演示、孙示例
const PERSONS = [
  {
    person: {
      name: '张演示', aliases: [], avatar: null, phone: null, email: null,
      company: '沪上置地集团', title: '投资总监', location: '上海市浦东新区',
      background: '行业峰会认识', relationshipStrength: 'strong',
      resourceTags: ['地产', '园区'], sensitivityLevel: 'low', status: 'active',
      nextStep: '约下月看浦东产业园项目', notes: '演示数据', school: null, projects: [],
    },
    interactions: [
      {
        timestamp: daysAgo(5),
        content: '聊了浦东两个地产项目的进展，他手里有园区招商资源，愿意引荐。',
        summary: '地产项目进展与园区资源对接',
        topics: ['地产', '项目合作'], actionItems: ['整理项目资料发给他'],
      },
    ],
  },
  {
    person: {
      name: '李示例', aliases: [], avatar: null, phone: null, email: null,
      company: 'example 资本', title: '合伙人', location: '上海市静安区',
      background: '朋友介绍', relationshipStrength: 'medium',
      resourceTags: ['地产', '融资'], sensitivityLevel: 'low', status: 'follow-up',
      nextStep: '把融资 BP 更新版发给她', notes: '演示数据', school: null, projects: [],
    },
    interactions: [
      {
        timestamp: daysAgo(9),
        content: '上次聊到 A 轮融资安排，她建议先梳理地产板块的现金流数据再谈估值。',
        summary: '融资节奏与地产板块数据准备',
        topics: ['融资', '地产'], actionItems: ['更新 BP', '补充现金流测算'],
      },
    ],
  },
  {
    person: {
      name: '陈样本', aliases: [], avatar: null, phone: null, email: null,
      company: '样本置业', title: '副总经理', location: '上海市徐汇区',
      background: '老同事', relationshipStrength: 'strong',
      resourceTags: ['地产'], sensitivityLevel: 'low', status: 'active',
      nextStep: null, notes: '演示数据', school: null, projects: [],
    },
    interactions: [
      {
        timestamp: daysAgo(12),
        content: '一起吃饭聊了徐汇滨江地产市场行情，他对旧改项目比较熟。',
        summary: '上海地产行情交流',
        topics: ['地产'], actionItems: [],
      },
    ],
  },
  {
    person: {
      name: '王样本', aliases: [], avatar: null, phone: null, email: null,
      company: '样本创投', title: '投资经理', location: '北京市朝阳区',
      background: '路演认识', relationshipStrength: 'medium',
      resourceTags: ['融资'], sensitivityLevel: 'low', status: 'follow-up',
      nextStep: '跟进融资意向书条款', notes: '演示数据', school: null, projects: [],
    },
    interactions: [
      {
        timestamp: daysAgo(15),
        content: '电话聊了融资意向，他们基金对本轮有兴趣，等我方补充财务数据后再约面谈。',
        summary: '融资意向初步沟通',
        topics: ['融资'], actionItems: ['补充财务数据', '约面谈时间'],
      },
    ],
  },
  {
    person: {
      name: '赵演示', aliases: [], avatar: null, phone: null, email: null,
      company: '演示汽车传媒', title: '商务总监', location: '杭州市西湖区',
      background: '前合作方', relationshipStrength: 'strong',
      resourceTags: ['汽车', '投标'], sensitivityLevel: 'low', status: 'active',
      nextStep: '请他把懂车帝投标的资质清单发过来', notes: '演示数据', school: null, projects: [],
    },
    interactions: [
      {
        timestamp: daysAgo(3),
        content: '聊了懂车帝这次投标的评分规则，他之前中过类似标，愿意帮忙看标书。',
        summary: '懂车帝投标经验交流',
        topics: ['懂车帝', '投标'], actionItems: ['把标书初稿发给他'],
      },
    ],
  },
  {
    person: {
      name: '孙示例', aliases: [], avatar: null, phone: null, email: null,
      company: '示例咨询', title: '项目经理', location: '苏州市工业园区',
      background: '客户介绍', relationshipStrength: 'medium',
      resourceTags: ['投标', '招商'], sensitivityLevel: 'low', status: 'active',
      nextStep: null, notes: '演示数据', school: null, projects: [],
    },
    interactions: [
      {
        timestamp: daysAgo(7),
        content: '请教了懂车帝投标的资质要求，她团队做过汽车平台的标，能提供报价参考。',
        summary: '投标资质与报价咨询',
        topics: ['投标'], actionItems: ['要一份历史报价参考'],
      },
    ],
  },
];

// 仅在新增联系人之间建的演示关系
const RELATIONSHIPS = [
  { from: '张演示', to: '李示例', relationshipType: '合作伙伴', strength: 'medium', description: '地产项目融资合作（演示数据）' },
  { from: '赵演示', to: '孙示例', relationshipType: '同行', strength: 'medium', description: '投标项目相识（演示数据）' },
];

const DEMO_QUERIES = [
  '谁在上海做地产，和我关系比较近？',
  '上次聊过融资的人里，还没跟进的有谁？',
  '这个懂车帝的投标，谁能帮上忙？',
];

// ---- 主流程 ----
const state = await req('GET', '/api/auth/state');
if (!state.initialized) {
  console.error('数据库尚未初始化，为避免误建库，本脚本不会调用 setup，请先在前端完成初始化。');
  process.exit(1);
}
const { token } = await req('POST', '/api/auth/unlock', { password: PASSWORD });
console.log('已解锁数据库');

// 幂等：已有同名联系人则整体跳过（含其互动/关系）
const existing = await req('GET', '/api/persons', undefined, token);
const existingNames = new Map(existing.map((p) => [p.name, p.id]));
console.log(`库内现有联系人: ${existing.length}`);

const nameToId = new Map();
let createdPersons = 0;
let createdInteractions = 0;

for (const { person, interactions } of PERSONS) {
  if (existingNames.has(person.name)) {
    console.log(`跳过（已存在）: ${person.name}`);
    nameToId.set(person.name, existingNames.get(person.name));
    continue;
  }
  const created = await req('POST', '/api/persons', person, token);
  nameToId.set(person.name, created.id);
  createdPersons += 1;
  for (const interaction of interactions) {
    await req('POST', '/api/interactions', { personId: created.id, ...interaction }, token);
    createdInteractions += 1;
  }
  console.log(`新增: ${person.name}（互动 ${interactions.length} 条）`);
}

// 关系仅在两端本次均为新增时创建，避免与既有数据产生关联或重复
let createdRelationships = 0;
if (createdPersons > 0) {
  for (const rel of RELATIONSHIPS) {
    if (existingNames.has(rel.from) || existingNames.has(rel.to)) continue;
    await req('POST', '/api/relationships', {
      fromPersonId: nameToId.get(rel.from),
      toPersonId: nameToId.get(rel.to),
      relationshipType: rel.relationshipType,
      strength: rel.strength,
      description: rel.description,
    }, token);
    createdRelationships += 1;
  }
}

console.log(`\n写入完成: persons=${createdPersons} interactions=${createdInteractions} relationships=${createdRelationships}`);

// ---- 验证三条示例查询 ----
let allPass = true;
for (const query of DEMO_QUERIES) {
  const results = await req('POST', '/api/nlq', { query }, token);
  const names = results.map((r) => r.displayName).join('、');
  console.log(`\n「${query}」→ ${results.length} 条`);
  console.log(`  ${names || '（无结果）'}`);
  if (results.length === 0) allPass = false;
}

if (!allPass) {
  console.error('\n❌ 存在返回为空的示例查询，请检查数据');
  process.exit(1);
}
console.log('\n✅ 三条示例查询均返回非空结果');
