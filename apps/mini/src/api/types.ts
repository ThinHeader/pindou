export interface ApiError {
  statusCode?: number
  error?: string
  message?: string
  /** 原始响应体。message 不是字符串时，列表还能从这里把稿件认出来。 */
  data?: unknown
  /** 登录过期已引导去登录页，调用方不必再弹错误。 */
  silent?: boolean
}

export interface UserInfo {
  id: string
  nickname: string
  avatar_url?: string | null
}

export interface LoginResult {
  token: string
  mode: 'dev' | 'wechat' | string
  user: UserInfo
}

export interface CreditsMe {
  balance: number
  generate_cost: number
  ad_reward: number
  ad_claimed_today: boolean
}

export interface CreditsMutation {
  balance: number
  delta: number
  reason: string
}

export interface CreditPack {
  code: string
  name: string
  credits: number
  price_cents: number
  price_label: string
  blurb: string
  popular: boolean
}

export interface VirtualPayParams {
  signData: string
  paySig: string
  signature: string
  mode: string
}

export interface CreditOrder {
  order_id: string
  out_trade_no?: string
  pack_code: string
  credits: number
  price_cents: number
  amount_fen?: number
  price_label: string
  status: string
  mock: boolean
  virtual_payment?: VirtualPayParams | null
  balance?: number
}

export interface LedgerItem {
  id: string
  delta: number
  balance_after: number
  reason: string
  reason_label: string
  created_at: string
}

export interface PaletteColor {
  code: string
  name: string
  hex: string
  count: number
}

export interface DraftCard {
  id: string
  title: string
  board_size: number
  color_count: number
  peg_count: number
  preview_url: string
  is_template: boolean
  is_official: boolean
  favorited: boolean
  badge: string
  author_nickname: string
  created_at: string
  pipeline_tags?: string[]
}

export interface DraftDetail extends DraftCard {
  palette: PaletteColor[]
  cells_b64: string
  palette_pref: string
  work_id: string | null
  hint: string
  pipeline?: string
  provider?: string
  remove_bg?: boolean
  cartoon_strength?: string
  subject_type?: string
  palette_brand?: string
  pipeline_lines?: string[]
  original_url?: string | null
  matted_url?: string | null
  cartoon_url?: string | null
  source_url?: string | null
  source_key?: string | null
}

export interface JobStage {
  id: string
  label: string
  state: 'done' | 'active' | 'pending' | 'fail' | 'skipped' | string
  detail: string
}

export interface JobView {
  job_id: string
  status: string
  progress: number
  message: string
  stages: JobStage[]
  provider: string
  remove_bg: boolean
  cartoon_strength: string
  subject_type: string
  palette_brand: string
  palette_pref: string
  board_size: number
  color_count: number
  original_url?: string | null
  matted_url?: string | null
  cartoon_url?: string | null
  preview_url?: string | null
  draft_id?: string | null
  draft?: DraftDetail | null
  balance?: number | null
  /** 失败原因，给用户看。进行中和成功为空。 */
  error?: string | null
  /**
   * 稳定业务错误码。版权或敏感内容拦截为 `content_policy`，其它失败为空。
   * 展示文案见 `JOB_ERROR_COPY`，不要直接把这个码显示出来。
   */
  error_code?: string | null
  charged: boolean
}

export interface MeProfile {
  user: UserInfo
  stats: { drafts: number; works: number; favorites: number }
  credits: CreditsMe
  recent_drafts: DraftCard[]
}

export interface StoredObject {
  key: string
  url: string
  etag: string
  content_type: string
}
