import { defineStore } from 'pinia'
import { creditsApi, draftsApi } from '../api/http'

const STORAGE_KEY = 'pindou_credits_balance'

export const useCreditsStore = defineStore('credits', {
  state: () => ({
    balance: 2,
    dailyGrant: 3,
    generateCost: 1,
    adReward: 1,
    showEmptySheet: false,
    loading: false,
  }),
  getters: {
    canGenerate: (s) => s.balance >= s.generateCost,
  },
  actions: {
    hydrate() {
      try {
        const raw = uni.getStorageSync(STORAGE_KEY)
        if (raw !== '' && raw !== undefined && raw !== null) {
          this.balance = Number(raw)
        }
      } catch {
        /* ignore */
      }
    },
    persist() {
      uni.setStorageSync(STORAGE_KEY, this.balance)
    },
    async refreshFromApi() {
      this.loading = true
      try {
        const data = await creditsApi.me()
        this.balance = data.balance
        this.dailyGrant = data.daily_grant
        this.generateCost = data.generate_cost
        this.adReward = data.ad_reward
        this.persist()
      } catch {
        // 离线：保持本地余额
      } finally {
        this.loading = false
      }
    },
    openEmptySheet() {
      this.showEmptySheet = true
    },
    closeEmptySheet() {
      this.showEmptySheet = false
    },
    /** 本地演示：日领 +3（无网时） */
    claimDailyLocal() {
      this.balance += this.dailyGrant
      this.persist()
    },
    async claimDaily() {
      try {
        const data = await creditsApi.dailyClaim()
        this.balance = data.balance
        this.persist()
      } catch {
        this.claimDailyLocal()
      }
    },
    /** 本地演示：广告 +1 */
    adRewardLocal() {
      this.balance += this.adReward
      this.persist()
      this.closeEmptySheet()
    },
    async claimAdReward() {
      try {
        const data = await creditsApi.adReward()
        this.balance = data.balance
        this.persist()
        this.closeEmptySheet()
      } catch {
        this.adRewardLocal()
      }
    },
    /** 尝试出稿扣分；不足则弹层 */
    async tryGenerate(payload?: {
      board_size?: string
      color_count?: number
    }): Promise<boolean> {
      if (this.balance < this.generateCost) {
        this.openEmptySheet()
        return false
      }
      try {
        const data = await draftsApi.generate(payload || { board_size: '52x52', color_count: 24 })
        this.balance = data.balance
        this.persist()
        uni.showToast({ title: '出稿任务已提交', icon: 'success' })
        return true
      } catch (e: unknown) {
        const err = e as { error?: string }
        if (err?.error === 'insufficient_credits') {
          this.openEmptySheet()
          return false
        }
        // 离线本地扣
        this.balance -= this.generateCost
        this.persist()
        uni.showToast({ title: '已扣 1 积分（离线）', icon: 'none' })
        return true
      }
    },
  },
})
