import { defineStore } from 'pinia'
import { creditsApi, errorOf } from '../api/http'
import { useUserStore } from './user'

export const useCreditsStore = defineStore('credits', {
  state: () => ({
    balance: 0,
    dailyGrant: 3,
    generateCost: 1,
    adReward: 1,
    dailyClaimed: false,
    showEmptySheet: false,
    loading: false,
  }),
  getters: {
    canGenerate: (s) => s.balance >= s.generateCost,
  },
  actions: {
    apply(data: {
      balance: number
      daily_grant?: number
      generate_cost?: number
      ad_reward?: number
      daily_claimed_today?: boolean
    }) {
      this.balance = data.balance
      if (data.daily_grant != null) this.dailyGrant = data.daily_grant
      if (data.generate_cost != null) this.generateCost = data.generate_cost
      if (data.ad_reward != null) this.adReward = data.ad_reward
      if (data.daily_claimed_today != null) this.dailyClaimed = data.daily_claimed_today
    },
    async refresh() {
      const user = useUserStore()
      if (!user.isLoggedIn) return
      this.loading = true
      try {
        this.apply(await creditsApi.me())
      } catch (e) {
        if (errorOf(e).statusCode === 401) user.logout()
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
    async claimDaily() {
      const data = await creditsApi.dailyClaim()
      this.balance = data.balance
      this.dailyClaimed = true
      return data
    },
    async claimAdReward() {
      const data = await creditsApi.adReward()
      this.balance = data.balance
      this.closeEmptySheet()
      return data
    },
  },
})
