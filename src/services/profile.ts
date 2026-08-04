export interface UserProfileContext {
  persona: string;
  values: string[];
  constraints: string[];
  preferences: string[];
}

const DEFAULT_PROFILE: UserProfileContext = {
  persona: '用户偏好以长期关系维护、低摩擦记录和高质量跟进为主。',
  values: ['注重长期关系', '重视跟进时机', '优先保护高敏感联系人'],
  constraints: ['写入前必须经过确认', '高敏感信息优先脱敏', '优先使用已确认的人脉关系'],
  preferences: ['偏好简洁摘要', '偏好按关系强度排序', '更重视下一步行动'],
};

export function getDefaultProfile(): UserProfileContext {
  return DEFAULT_PROFILE;
}
