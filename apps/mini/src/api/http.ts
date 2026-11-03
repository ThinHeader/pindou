import type {
  ApiError,
  CreditOrder,
  CreditPack,
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

function request<T>(path: string, method: Method = 'GET', data?: object): Promise<T> {
  return new Promise((resolve, reject) => {
    uni.request({
      url: `${API_BASE}${path}`,
      method,
      data: data as UniApp.RequestOptions['data'],
      header: {
        'Content-Type': 'application/json',
        ...authHeader(),
      },
      success: (res) => {
        const data = res.data as ApiError & T
        if (res.statusCode >= 200 && res.statusCode < 300) {
          resolve(data as T)
          return
        }
        reject({
          statusCode: res.statusCode,
          error: data?.error,
          message: data?.message || '请求失败',
        } satisfies ApiError)
      },
      fail: () => {
        reject({
          message: '无法连接服务器，请确认 API 已启动',
        } satisfies ApiError)
      },
    })
  })
}

export const http = {
  get: <T>(path: string) => request<T>(path, 'GET'),
  post: <T>(path: string, data?: object) => request<T>(path, 'POST', data),
  put: <T>(path: string, data?: object) => request<T>(path, 'PUT', data),
  del: <T>(path: string) => request<T>(path, 'DELETE'),
}

export function uploadImage(filePath: string): Promise<StoredObject> {
  return new Promise((resolve, reject) => {
    uni.uploadFile({
      url: `${API_BASE}/api/v1/uploads`,
      filePath,
      name: 'file',
      header: authHeader(),
      success: (res) => {
        let data: StoredObject & ApiError
        try {
          data = typeof res.data === 'string' ? JSON.parse(res.data) : (res.data as StoredObject & ApiError)
        } catch {
          reject({ message: '上传响应无法解析' } satisfies ApiError)
          return
        }
        if (res.statusCode >= 200 && res.statusCode < 300 && data.key) {
          resolve(data)
          return
        }
        reject({
          statusCode: res.statusCode,
          error: data.error,
          message: data.message || '上传失败',
        } satisfies ApiError)
      },
      fail: () => reject({ message: '上传失败，请重试' } satisfies ApiError),
    })
  })
}

export const authApi = {
  wxLogin: (code: string) => http.post<LoginResult>('/api/v1/auth/wx-login', { code }),
  me: () => http.get<MeProfile>('/api/v1/me'),
  patchMe: (nickname: string) => http.put<MeProfile>('/api/v1/me', { nickname }),
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
