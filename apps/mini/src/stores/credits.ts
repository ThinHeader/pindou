import { defineStore } from 'pinia'
import { creditsApi, errorOf } from '../api/http'
import { useUserStore } from './user'

export const useCreditsStore = defineStore('credits', {
  state: () => ({
    balance: 0,
    generateCost: 1,
    adReward: 1,
    adClaimed: false,
    showEmptySheet: false,
    loading: false,
  }),
  getters: {
    canGenerate: (s) => s.balance >= s.generateCost,
  },
  actions: {
    apply(data: {
      balance: number
      generate_cost?: number
      ad_reward?: number
      ad_claimed_today?: boolean
    }) {
      this.balance = data.balance
      if (data.generate_cost != null) this.generateCost = data.generate_cost
      if (data.ad_reward != null) this.adReward = data.ad_reward
      if (data.ad_claimed_today != null) this.adClaimed = data.ad_claimed_today
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
    async claimAdReward() {
      const data = await creditsApi.adReward()
      this.balance = data.balance
      this.adClaimed = true
      this.closeEmptySheet()
      return data
    },
  },
})
