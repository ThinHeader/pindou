/** 微信用户信息。当前基础库的 getUserProfile 常返回匿名占位，不能当真实资料保存。 */
export interface WechatUserInfo {
  nickName?: string
  avatarUrl?: string
}

const ANONYMOUS_NAMES = new Set(['微信用户', 'WeChat User', 'Weixin User'])
/** 官方示例里的默认灰头像。 */
const DEFAULT_AVATAR_MARK = 'icTdbqWNOwNRna42FI242Lcia07jQodd2F'

export function realWechatProfile(
  info: WechatUserInfo | null | undefined,
): { nickName: string; avatarUrl: string } | null {
  if (!info) return null
  const nickName = (info.nickName || '').trim()
  const avatarUrl = (info.avatarUrl || '').trim()
  if (!nickName || ANONYMOUS_NAMES.has(nickName)) return null
  if ([...nickName].length > 20) return null
  if (!avatarUrl || avatarUrl.includes(DEFAULT_AVATAR_MARK)) return null
  if (!/^https:\/\//i.test(avatarUrl)) return null
  return { nickName, avatarUrl }
}

/**
 * 必须在点击回调里同步发起。基础库 2.27.1 起通常不再弹授权窗，并返回「微信用户」。
 * 没有回调或接口不存在时返回 null，由登录页改走头像昵称填写。
 */
export function requestWechatProfile(): Promise<WechatUserInfo | null> {
  return new Promise((resolve) => {
    let settled = false
    const done = (value: WechatUserInfo | null) => {
      if (settled) return
      settled = true
      resolve(value)
    }
    const api = (uni as unknown as { getUserProfile?: (opts: object) => void }).getUserProfile
    if (typeof api !== 'function') {
      done(null)
      return
    }
    api({
      desc: '用于在「我的」展示当前账号',
      lang: 'zh_CN',
      success: (res: { userInfo?: WechatUserInfo }) => done(res?.userInfo ?? null),
      fail: () => done(null),
    })
  })
}

export function downloadWechatAvatar(url: string): Promise<string> {
  return new Promise((resolve, reject) => {
    uni.downloadFile({
      url,
      success: (res) => {
        if (res.statusCode === 200 && res.tempFilePath) {
          resolve(res.tempFilePath)
          return
        }
        reject({ message: '头像下载失败' })
      },
      fail: () => reject({ message: '头像下载失败' }),
    })
  })
}
