import { defineStore } from 'pinia'
import { authApi, errorOf, postProfileImage, uploadProfile } from '../api/http'
import type { MeProfile, UserInfo } from '../api/types'
import { copyAvatarForUpload, readImageBytes } from '../utils/local-image'

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
 * 「保存失败，请重试」只在 uni.uploadFile 的 fail 里出现。
 * chooseAvatar 临时路径让这次上传在真机直接失败，中文昵称放进 formData 还会乱码。
 * 先用和登录相同的 wx.request 把 multipart 交给现有的 /me/profile；请求没到达接口时，
 * 再把文件拷到用户目录后 uploadFile，昵称单独走 PUT /me。
 */
async function saveAvatarProfile(filePath: string, nickname: string): Promise<MeProfile> {
  let bytes: Uint8Array | null = null
  try {
    bytes = await readImageBytes(filePath)
  } catch {
    bytes = null
  }
  if (bytes) {
    try {
      return await postProfileImage(bytes, nickname)
    } catch (err) {
      const message = errorOf(err).message
      if (errorOf(err).statusCode === 401 || message === '昵称需要 1 到 20 个字') throw err
    }
  }
  const localPath = await copyAvatarForUpload(filePath)
  const uploaded = await uploadProfile(localPath, '')
  if (!nickname) return uploaded
  return authApi.putNickname(nickname)
}
