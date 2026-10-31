import { defineStore } from 'pinia'
import { draftsApi, uploadImage } from '../api/http'
import type { DraftDetail } from '../api/types'
import { useCreditsStore } from './credits'

export type BoardSize = 52 | 78 | 104
export type PalettePref = 'auto' | 'warm' | 'macaron' | 'contrast'

export const useCreateStore = defineStore('create', {
  state: () => ({
    localPath: '',
    fileName: '',
    imageKey: '',
    imageBase64: '',
    builtin: '',
    boardSize: 52 as BoardSize,
    colorCount: 12,
    palettePref: 'auto' as PalettePref,
    referenceTitle: '',
    generating: false,
    lastDraft: null as DraftDetail | null,
  }),
  getters: {
    hasImage: (s) => Boolean(s.imageKey || s.imageBase64 || s.builtin),
    imageLabel: (s) => {
      if (s.builtin) return `示例图 · ${s.fileName || s.builtin}`
      if (s.fileName) return s.fileName
      if (s.imageKey || s.imageBase64) return '参考图已选'
      return ''
    },
  },
  actions: {
    resetParams() {
      this.boardSize = 52
      this.colorCount = 12
      this.palettePref = 'auto'
    },
    clearImage() {
      this.localPath = ''
      this.fileName = ''
      this.imageKey = ''
      this.imageBase64 = ''
      this.builtin = ''
    },
    useBuiltin(name: 'cat' | 'bear' | 'chick' | 'bunny', title: string) {
      this.clearImage()
      this.builtin = name
      this.fileName = title
    },
    async attachLocal(path: string, name: string) {
      this.clearImage()
      this.localPath = path
      this.fileName = name || '参考图'
      try {
        const stored = await uploadImage(path)
        this.imageKey = stored.key
        return
      } catch {
        const b64 = await readBase64(path)
        this.imageBase64 = b64
      }
    },
    async generate() {
      const credits = useCreditsStore()
      if (credits.balance < credits.generateCost) {
        credits.openEmptySheet()
        return null
      }
      this.generating = true
      try {
        const data = await draftsApi.generate({
          image_ref: this.imageKey || (this.builtin ? `builtin:${this.builtin}` : undefined),
          image_base64: this.imageKey || this.builtin ? undefined : this.imageBase64 || undefined,
          board_size: this.boardSize,
          color_count: this.colorCount,
          palette_pref: this.palettePref,
          title: this.referenceTitle || undefined,
        })
        this.lastDraft = data.draft
        credits.balance = data.balance
        return data
      } finally {
        this.generating = false
      }
    },
  },
})

function readBase64(path: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const fs = typeof uni.getFileSystemManager === 'function' ? uni.getFileSystemManager() : null
    if (!fs) {
      reject(new Error('read'))
      return
    }
    fs.readFile({
      filePath: path,
      encoding: 'base64',
      success: (res) => resolve(String(res.data)),
      fail: reject,
    })
  })
}
