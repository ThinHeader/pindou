import { defineStore } from 'pinia'

export const useLibraryStore = defineStore('library', {
  state: () => ({
    tab: 'all',
  }),
  actions: {
    open(tab: string) {
      this.tab = tab
    },
  },
})
