import { defineStore } from 'pinia'
import { authApi, errorOf } from '../api/http'
import type { MeProfile, UserInfo } from '../api/types'
import { readAvatarBase64 } from '../utils/local-image'

const TOKEN_KEY = 'pindou_token'
const USER_KEY = 'pindou_user'
const DEV_CODE_KEY = 'pindou_dev_login_code'

export const useUserStore = defineStore('user', {
  state: () => ({
    token: '' as string,
    user: null as UserInfo | null,
    profile: null as MeProfile | null,
  }),
  getters: {
    isLoggedIn: (s) => Boolean(s.token),
    nickname: (s) => s.user?.nickname || '拼豆玩家',
    avatarUrl: (s) => s.user?.avatar_url || '',
    hasProfile: (s) =>
      Boolean(s.user?.avatar_url) || Boolean(s.user?.nickname && s.user.nickname !== '拼豆玩家'),
  },
  actions: {
    hydrate() {
      try {
        this.token = uni.getStorageSync(TOKEN_KEY) || ''
        const raw = uni.getStorageSync(USER_KEY)
        if (raw) this.user = typeof raw === 'string' ? JSON.parse(raw) : raw
      } catch {
        this.token = ''
        this.user = null
      }
    },
    persist() {
      uni.setStorageSync(TOKEN_KEY, this.token)
      uni.setStorageSync(USER_KEY, JSON.stringify(this.user))
    },
    logout() {
      this.token = ''
      this.user = null
      this.profile = null
      uni.removeStorageSync(TOKEN_KEY)
      uni.removeStorageSync(USER_KEY)
    },
    devCode() {
      let code = uni.getStorageSync(DEV_CODE_KEY) as string
      if (!code) {
        code = `dev-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`
        uni.setStorageSync(DEV_CODE_KEY, code)
      }
      return code
    },
    async obtainCode() {
      const useWx = import.meta.env.VITE_WX_LOGIN === '1'
      if (!useWx) return this.devCode()
      return new Promise<string>((resolve) => {
        uni.login({
          provider: 'weixin',
          success: (res) => resolve(res.code || this.devCode()),
          fail: () => resolve(this.devCode()),
        })
      })
    },
    async login() {
      const code = await this.obtainCode()
      const data = await authApi.wxLogin(code)
      this.token = data.token
      this.user = data.user
      this.persist()
      return data
    },
    async refreshProfile() {
      if (!this.token) return null
      try {
        const profile = await authApi.me()
        this.profile = profile
        this.user = profile.user
        this.persist()
        return profile
      } catch (e) {
        if (errorOf(e).statusCode === 401) this.logout()
        throw e
      }
    },
    async saveProfile(input: { nickname?: string; filePath?: string }) {
      const nickname = input.nickname?.trim() || ''
      const profile = input.filePath
        ? await saveAvatarProfile(input.filePath, nickname)
        : await authApi.putNickname(nickname)
      this.profile = profile
      this.user = profile.user
      this.persist()
      return profile
    },
  },
})

/**
 * #14 的两条路在真机上都进了 fail，请求到不了接口：
 * wx.request 发送 ArrayBuffer multipart，以及 wx.uploadFile。
 * 登录用的是 JSON wx.request，所以头像改成 base64 放进 PUT /api/v1/me。
 */
async function saveAvatarProfile(filePath: string, nickname: string): Promise<MeProfile> {
  const avatarBase64 = await readAvatarBase64(filePath)
  const body: { nickname?: string; avatar_base64: string } = { avatar_base64: avatarBase64 }
  if (nickname) body.nickname = nickname
  const profile = await authApi.putProfile(body)
  if (!profile.user.avatar_url) {
    throw { message: '头像没有保存，请更新服务' }
  }
  return profile
}
