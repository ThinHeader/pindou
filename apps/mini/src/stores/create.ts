import { defineStore } from 'pinia'
import { draftsApi, uploadImage } from '../api/http'
import type { DraftDetail, JobView } from '../api/types'
import { useCreditsStore } from './credits'

export type BoardSize = 52 | 78 | 104
export type PalettePref = 'auto' | 'warm' | 'macaron' | 'contrast'
export type CartoonStrength = 'light' | 'standard' | 'strong'
export type SubjectType = 'person' | 'pet' | 'object'
export type PaletteBrand = 'generic' | 'mard' | 'perler' | 'shop'

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
    /** 智能去背，默认开。 */
    removeBg: true,
    cartoonStrength: 'standard' as CartoonStrength,
    subjectType: 'person' as SubjectType,
    paletteBrand: 'generic' as PaletteBrand,
    /** 本次出稿要保存的稿件名。空则交给后端默认名。 */
    title: '',
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
      this.removeBg = true
      this.cartoonStrength = 'standard'
      this.subjectType = 'person'
      this.paletteBrand = 'generic'
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
    /** 提交 MVP+ 任务。扣分发生在任务成功之后，这里只负责开跑。 */
    async startJob(): Promise<JobView | null> {
      const credits = useCreditsStore()
      if (credits.balance < credits.generateCost) {
        credits.openEmptySheet()
        return null
      }
      this.generating = true
      try {
        const named = this.title.trim()
        return await draftsApi.generate({
          image_ref: this.imageKey || (this.builtin ? `builtin:${this.builtin}` : undefined),
          image_base64: this.imageKey || this.builtin ? undefined : this.imageBase64 || undefined,
          board_size: this.boardSize,
          color_count: this.colorCount,
          palette_pref: this.palettePref,
          title: named || undefined,
          remove_bg: this.removeBg,
          cartoon_strength: this.cartoonStrength,
          subject_type: this.subjectType,
          palette_brand: this.paletteBrand,
        })
      } finally {
        this.generating = false
      }
    },
    /** 沿用稿件原图和参数，再次出稿。 */
    reuseDraft(draft: DraftDetail) {
      this.clearImage()
      this.imageKey = draft.source_key || ''
      this.localPath = draft.original_url || draft.source_url || ''
      this.fileName = '沿用原图'
      const size = draft.board_size
      this.boardSize = size === 78 || size === 104 ? size : 52
      const pref = draft.palette_pref
      this.palettePref =
        pref === 'warm' || pref === 'macaron' || pref === 'contrast' ? pref : 'auto'
      if (draft.pipeline === 'mvp_plus') {
        this.removeBg = draft.remove_bg !== false
        const strength = draft.cartoon_strength
        this.cartoonStrength =
          strength === 'light' || strength === 'strong' || strength === 'standard'
            ? strength
            : 'standard'
        const subject = draft.subject_type
        this.subjectType = subject === 'pet' || subject === 'object' ? subject : 'person'
        const brand = draft.palette_brand
        this.paletteBrand =
          brand === 'mard' || brand === 'perler' || brand === 'shop' ? brand : 'generic'
      }
      this.title = draft.title === '未命名草稿' ? '' : draft.title
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
