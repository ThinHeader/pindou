import { notifySessionExpired, refreshSessionSingleFlight, sessionRefreshBound } from './session'
import type {
  ApiError,
  CreditOrder,
  CreditPack,
  VirtualPayParams,
  WxPayParams,
  CreditsMe,
  CreditsMutation,
  DraftCard,
  DraftDetail,
  JobView,
  LedgerItem,
  LoginResult,
  MeProfile,
  StoredObject,
} from './types'
const TOKEN_KEY = 'pindou_token'

export const API_BASE = (
  (import.meta.env.VITE_API_BASE as string | undefined) || 'http://127.0.0.1:3000'
).replace(/\/$/, '')

function authHeader(): Record<string, string> {
  try {
    const token = uni.getStorageSync(TOKEN_KEY) as string
    return token ? { Authorization: `Bearer ${token}` } : {}
  } catch {
    return {}
  }
}

type Method = 'GET' | 'POST' | 'PUT' | 'DELETE'

type RequestOpts = {
  retryOn401?: boolean
}

type Ok<T> = { ok: true; data: T }
type Err = { ok: false; hadAuth: boolean; error: ApiError }

function send<T>(path: string, method: Method, data?: object): Promise<Ok<T> | Err> {
  return new Promise((resolve) => {
    const auth = authHeader()
    const hadAuth = Boolean(auth.Authorization)
    const header = {
      'Content-Type': 'application/json',
      ...auth,
    }
    uni.request({
      url: `${API_BASE}${path}`,
      method,
      data: data as UniApp.RequestOptions['data'],
      header,
      success: (res) => {
        const body = res.data as ApiError & T
        if (res.statusCode >= 200 && res.statusCode < 300) {
          resolve({ ok: true, data: body as T })
          return
        }
        resolve({
          ok: false,
          hadAuth,
          error: {
            statusCode: res.statusCode,
            error: body?.error,
            message: body?.message || '请求失败',
          },
        })
      },
      fail: () => {
        resolve({
          ok: false,
          hadAuth,
          error: { message: '无法连接服务器，请确认 API 已启动' },
        })
      },
    })
  })
}

function silent(err: ApiError): ApiError {
  return { ...err, silent: true }
}

async function withAuthRetry<T>(err: ApiError, retry: () => Promise<T>): Promise<T> {
  if (!sessionRefreshBound()) throw err
  const ok = await refreshSessionSingleFlight()
  if (!ok) {
    notifySessionExpired()
    throw silent(err)
  }
  try {
    return await retry()
  } catch (again) {
    const parsed = errorOf(again)
    if (parsed.statusCode === 401) {
      notifySessionExpired()
      throw silent(parsed)
    }
    throw again
  }
}

function request<T>(path: string, method: Method = 'GET', data?: object, opts?: RequestOpts): Promise<T> {
  const allowRetry = opts?.retryOn401 !== false
  return send<T>(path, method, data).then(async (result) => {
    if (result.ok) return result.data
    if (allowRetry && result.hadAuth && result.error.statusCode === 401) {
      return withAuthRetry(result.error, async () => {
        const second = await send<T>(path, method, data)
        if (!second.ok) throw second.error
        return second.data
      })
    }
    throw result.error
  })
}

export const http = {
  get: <T>(path: string) => request<T>(path, 'GET'),
  post: <T>(path: string, data?: object, opts?: RequestOpts) => request<T>(path, 'POST', data, opts),
  put: <T>(path: string, data?: object) => request<T>(path, 'PUT', data),
  del: <T>(path: string) => request<T>(path, 'DELETE'),
}

function uploadOnce(filePath: string): Promise<Ok<StoredObject> | Err> {
  return new Promise((resolve) => {
    const header = authHeader()
    const hadAuth = Boolean(header.Authorization)
    uni.uploadFile({
      url: `${API_BASE}/api/v1/uploads`,
      filePath,
      name: 'file',
      header,
      success: (res) => {
        let data: StoredObject & ApiError
        try {
          data = typeof res.data === 'string' ? JSON.parse(res.data) : (res.data as StoredObject & ApiError)
        } catch {
          resolve({ ok: false, hadAuth, error: { message: '上传响应无法解析' } })
          return
        }
        if (res.statusCode >= 200 && res.statusCode < 300 && data.key) {
          resolve({ ok: true, data })
          return
        }
        resolve({
          ok: false,
          hadAuth,
          error: {
            statusCode: res.statusCode,
            error: data.error,
            message: data.message || '上传失败',
          },
        })
      },
      fail: () => resolve({ ok: false, hadAuth, error: { message: '上传失败，请重试' } }),
    })
  })
}

export function uploadImage(filePath: string): Promise<StoredObject> {
  return uploadOnce(filePath).then(async (result) => {
    if (result.ok) return result.data
    if (result.hadAuth && result.error.statusCode === 401) {
      return withAuthRetry(result.error, async () => {
        const second = await uploadOnce(filePath)
        if (!second.ok) throw second.error
        return second.data
      })
    }
    throw result.error
  })
}

export const authApi = {
  // 续期本身就是这次登录。401 不能再触发续期，否则会套娃。
  wxLogin: (code: string) =>
    http.post<LoginResult>('/api/v1/auth/wx-login', { code }, { retryOn401: false }),
  me: () => http.get<MeProfile>('/api/v1/me'),
  putNickname: (nickname: string) => http.put<MeProfile>('/api/v1/me', { nickname }),
  putProfile: (body: { nickname?: string; avatar_base64?: string }) =>
    http.put<MeProfile>('/api/v1/me', body),
}

export const creditsApi = {
  me: () => http.get<CreditsMe>('/api/v1/credits/me'),
  dailyClaim: () => http.post<CreditsMutation>('/api/v1/credits/daily-claim'),
  adReward: () => http.post<CreditsMutation>('/api/v1/credits/ad-reward'),
  ledger: () => http.get<{ items: LedgerItem[] }>('/api/v1/credits/ledger'),
  packs: () =>
    http.get<{ items: CreditPack[]; mock: boolean; pay_mode: string; note: string }>('/api/v1/credits/packs'),
  createOrder: (packCode: string) =>
    http.post<CreditOrder>('/api/v1/credits/orders', { pack_code: packCode }),
  getOrder: (orderId: string) => http.get<CreditOrder>(`/api/v1/credits/orders/${orderId}`),
  mockPay: (orderId: string) =>
    http.post<{ order: CreditOrder; balance: number; delta: number }>(
      `/api/v1/credits/orders/${orderId}/mock-callback`,
    ),
}

interface WxMini {
  requestVirtualPayment?: (options: {
    signData: string
    paySig: string
    signature: string
    mode: string
    success?: () => void
    fail?: (err: { errMsg?: string; errCode?: number }) => void
  }) => void
  canIUse?: (schema: string) => boolean
}

function wxMini(): WxMini | null {
  if (typeof wx !== 'undefined' && wx) return wx
  const holder = globalThis as typeof globalThis & { wx?: WxMini }
  return holder.wx ?? null
}

function compareVersion(left: string, right: string): number {
  const a = left.split('.')
  const b = right.split('.')
  const len = Math.max(a.length, b.length)
  for (let i = 0; i < len; i += 1) {
    const na = Number.parseInt(a[i] || '0', 10)
    const nb = Number.parseInt(b[i] || '0', 10)
    if (na > nb) return 1
    if (na < nb) return -1
  }
  return 0
}

export function virtualPayUnavailable(signData: string): string | null {
  let env: number | null = null
  try {
    const parsed = JSON.parse(signData) as { env?: unknown }
    env = typeof parsed.env === 'number' ? parsed.env : null
  } catch {
    return '支付参数有误'
  }
  let platform = ''
  let sdk = ''
  try {
    const info = uni.getSystemInfoSync() as { platform?: string; SDKVersion?: string }
    platform = info.platform || ''
    sdk = info.SDKVersion || ''
  } catch {
    platform = ''
  }
  if (platform === 'ios' && env === 1) return '苹果设备暂不支持沙箱支付'
  const wx = wxMini()
  const supported =
    (sdk && compareVersion(sdk, '2.19.2') >= 0) || Boolean(wx?.canIUse?.('requestVirtualPayment'))
  if (!supported || !wx?.requestVirtualPayment) return '请升级微信后再支付'
  return null
}

export function requestVirtualPayment(params: VirtualPayParams): Promise<void> {
  return new Promise((resolve, reject) => {
    const wx = wxMini()
    if (!wx?.requestVirtualPayment) {
      reject({ errMsg: 'requestVirtualPayment:fail unsupported', errCode: -1 })
      return
    }
    wx.requestVirtualPayment({
      signData: params.signData,
      paySig: params.paySig,
      signature: params.signature,
      mode: params.mode || 'short_series_goods',
      success: () => resolve(),
      fail: (err) => reject(err),
    })
  })
}

export function requestWxPayment(params: WxPayParams): Promise<void> {
  return new Promise((resolve, reject) => {
    uni.requestPayment({
      provider: 'wxpay',
      orderInfo: params.package,
      timeStamp: params.timeStamp,
      nonceStr: params.nonceStr,
      package: params.package,
      signType: params.signType,
      paySign: params.paySign,
      success: () => resolve(),
      fail: (err) => reject(err),
    })
  })
}

export const draftsApi = {
  generate: (body: {
    image_ref?: string
    image_base64?: string
    board_size?: number
    color_count?: number
    palette_pref?: string
    title?: string
    remove_bg?: boolean
    cartoon_strength?: string
    subject_type?: string
    palette_brand?: string
  }) => http.post<JobView>('/api/v1/drafts/generate', body),
  list: (tab: string, q = '') =>
    http.get<{ items: DraftCard[] }>(
      `/api/v1/drafts?tab=${encodeURIComponent(tab)}&q=${encodeURIComponent(q)}`,
    ),
  detail: (id: string) => http.get<DraftDetail>(`/api/v1/drafts/${id}`),
  rename: (id: string, title: string) => http.put<DraftDetail>(`/api/v1/drafts/${id}`, { title }),
  remove: (id: string) =>
    http.del<{ deleted: boolean; work_deleted: boolean }>(`/api/v1/drafts/${id}`),
  favorite: (id: string) => http.post<{ favorited: boolean }>(`/api/v1/drafts/${id}/favorite`),
}

export const jobsApi = {
  get: (id: string) => http.get<JobView>(`/api/v1/jobs/${id}`),
  cancel: (id: string) => http.post<JobView>(`/api/v1/jobs/${id}/cancel`),
}

export function errorOf(e: unknown): ApiError {
  if (e && typeof e === 'object' && 'message' in e) return e as ApiError
  return { message: '操作失败' }
}
