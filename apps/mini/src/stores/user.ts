import { defineStore } from 'pinia'
import { authApi } from '../api/http'

const TOKEN_KEY = 'pindou_token'
const USER_KEY = 'pindou_user'

export interface UserInfo {
  id: string
  nickname: string
  avatar_url?: string | null
}

export const useUserStore = defineStore('user', {
  state: () => ({
    token: '' as string,
    user: null as UserInfo | null,
  }),
  getters: {
    isLoggedIn: (s) => Boolean(s.token),
  },
  actions: {
    hydrate() {
      try {
        this.token = uni.getStorageSync(TOKEN_KEY) || ''
        const raw = uni.getStorageSync(USER_KEY)
        if (raw) this.user = typeof raw === 'string' ? JSON.parse(raw) : raw
      } catch {
        /* ignore */
      }
    },
    persist() {
      uni.setStorageSync(TOKEN_KEY, this.token)
      uni.setStorageSync(USER_KEY, JSON.stringify(this.user))
    },
    /** 演示登录（不接真微信） */
    async loginDemo() {
      try {
        const data = await authApi.wxLogin('demo-code')
        this.token = data.token
        this.user = data.user
        this.persist()
      } catch {
        this.token = 'dev-token'
        this.user = { id: 'local', nickname: '拼豆玩家' }
        this.persist()
      }
    },
    logout() {
      this.token = ''
      this.user = null
      uni.removeStorageSync(TOKEN_KEY)
      uni.removeStorageSync(USER_KEY)
    },
  },
})
