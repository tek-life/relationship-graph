// 生成 1000 条"手搓风格"联系人伪数据 Excel，用于导入功能测试。
// 特意包含脏数据：空姓名、完全重复行、同名不同电话、电话格式混乱、
// 中文枚举值、多种标签分隔符、缺失字段。
// 运行：node scripts/generate-test-data.mjs

import * as XLSX from 'xlsx';
import { mkdirSync } from 'node:fs';

// 固定种子的伪随机（mulberry32），保证可复现且避免 JS 乘法精度丢失
let seed = 20260731;
function rand() {
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
function pick(list) {
  return list[Math.floor(rand() * list.length)];
}
function maybe(prob, value, fallback = '') {
  return rand() < prob ? value : fallback;
}

const SURNAMES = '张王李赵刘陈杨黄周吴徐孙马朱胡郭何高林罗郑梁谢宋唐许韩冯邓曹彭曾萧田董潘袁蔡蒋余于杜叶程苏魏吕丁任沈姚卢姜崔钟谭陆汪范金石廖贾夏韦付方白邹孟熊秦邱江尹薛闫段雷侯龙史陶黎贺顾毛郝龚邵万钱严覃武戴莫孔向汤'.split('');
const GIVEN = '明伟芳娜敏静丽强磊军洋勇艳杰娟涛超秀兰霞平刚桂英华玉萍红娥玲芬燕彬鹏浩宇轩然博文昊天翔嘉懿煜城建国建军志强志明海峰海燕春梅冬梅国栋家豪雨欣思远'.split('');
const COMPANIES = ['万科集团', '绿地控股', '中建三局', '华为技术', '腾讯科技', '阿里巴巴', '字节跳动', '招商银行', '中信证券', '红杉资本', '高瓴资本', '同济设计院', '上海城建', '融创中国', '龙湖地产', '金地集团', '平安不动产', '普洛斯', '凯德集团', '仲量联行', '戴德梁行', '正大集团', '复星国际', '均瑶集团', ''];
const TITLES = ['总经理', '副总裁', '董事长', '合伙人', '投资总监', '项目总监', '设计总监', '融资部负责人', '招商总监', '区域总', '首席架构师', '秘书长', '主任', '处长', '科长', ''];
const CITIES = ['上海', '北京', '深圳', '广州', '杭州', '苏州', '南京', '成都', '武汉', '西安', '重庆', '天津', '青岛', '厦门', ''];
const CHANNELS = ['朋友介绍', '饭局认识', '行业峰会', '老同学', '老同事', '客户介绍', '商会活动', '高尔夫球局', '校友会', 'EMBA同学', '项目合作认识', ''];
const STRENGTHS = ['强', '中', '弱', '很熟', '一般', '不熟', '铁', '泛泛', 'strong', ''];
const SENSITIVITIES = ['高', '中', '低', '保密', '公开', '', '', ''];
const STATUSES = ['待跟进', '活跃', '冷却', '正常', '跟进', '已合作', '', ''];
const TAGS = ['地产', '融资', '政府资源', '设计', '园区', '招商', '投标', '汽车', '金融', '法律', '媒体', '医疗', '教育', '物流', '基金', '上市公司'];
const SEPARATORS = ['、', '，', ',', '/', ' ', '；'];
const NEXT_STEPS = ['约饭深聊', '推进园区项目', '介绍给老王', '下月回访', '发项目资料', '春节问候', '约打球', ''];
const NOTES = ['人靠谱，说话算数', '喜欢喝茶', '孩子在国外读书', '对数字敏感，谈合作要带数据', '微信联系比电话好使', '有政府背景', ''];

function makePhone() {
  const digits = `1${pick(['3', '5', '7', '8', '9'])}${String(Math.floor(rand() * 1e9)).padStart(9, '0')}`;
  const style = rand();
  if (style < 0.55) return digits;
  if (style < 0.7) return `${digits.slice(0, 3)} ${digits.slice(3, 7)} ${digits.slice(7)}`;
  if (style < 0.82) return `${digits.slice(0, 3)}-${digits.slice(3, 7)}-${digits.slice(7)}`;
  if (style < 0.92) return `+86 ${digits}`;
  return '';
}

function makeName() {
  return pick(SURNAMES) + pick(GIVEN) + maybe(0.4, pick(GIVEN));
}

function makeTags() {
  const count = 1 + Math.floor(rand() * 3);
  const sep = pick(SEPARATORS);
  const set = new Set();
  for (let i = 0; i < count; i++) set.add(pick(TAGS));
  return Array.from(set).join(sep);
}

function makeRow() {
  const name = makeName();
  const pinyinMail = rand() < 0.3 ? `contact${Math.floor(rand() * 9999)}@${pick(['163.com', 'qq.com', 'gmail.com', '126.com'])}` : '';
  return {
    姓名: name,
    手机号: makePhone(),
    邮箱: pinyinMail,
    公司: pick(COMPANIES),
    职务: pick(TITLES),
    城市: pick(CITIES),
    怎么认识的: pick(CHANNELS),
    关系: pick(STRENGTHS),
    行业标签: makeTags(),
    敏感级别: pick(SENSITIVITIES),
    跟进状态: pick(STATUSES),
    下一步: pick(NEXT_STEPS),
    备注: pick(NOTES),
  };
}

const rows = [];
for (let i = 0; i < 955; i++) {
  rows.push(makeRow());
}

// 脏数据注入
// 1) 10 行空姓名
for (let i = 0; i < 10; i++) {
  const row = makeRow();
  row.姓名 = '';
  rows.push(row);
}
// 2) 20 行完全重复（姓名+电话与已有行一致）
for (let i = 0; i < 20; i++) {
  const source = rows[Math.floor(rand() * 900)];
  rows.push({ ...source });
}
// 3) 15 行同名不同电话
for (let i = 0; i < 15; i++) {
  const source = rows[Math.floor(rand() * 900)];
  const row = makeRow();
  row.姓名 = source.姓名;
  rows.push(row);
}

// 打乱顺序（保持前 900 行作为重复源不被打乱到尾部也没关系，整体 shuffle）
for (let i = rows.length - 1; i > 0; i--) {
  const j = Math.floor(rand() * (i + 1));
  [rows[i], rows[j]] = [rows[j], rows[i]];
}

console.log(`总行数: ${rows.length}`);

mkdirSync('test-data', { recursive: true });
const sheet = XLSX.utils.json_to_sheet(rows);
const workbook = XLSX.utils.book_new();
XLSX.utils.book_append_sheet(workbook, sheet, '我的人脉');
XLSX.writeFile(workbook, 'test-data/联系人测试数据_1000.xlsx');
console.log('已生成 test-data/联系人测试数据_1000.xlsx');
