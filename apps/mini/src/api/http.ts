/** API 客户端 — 指向本地后端 */
export const API_BASE = 'http://127.0.0.1:3000'

export type ApiError = { error: string; message: string }

async function request<T>(
  path: string,
  options: UniApp.RequestOptions = {},
): Promise<T> {
  return new Promise((resolve, reject) => {
    uni.request({
      url: `${API_BASE}${path}`,
      method: options.method || 'GET',
      data: options.data,
      header: {
        'Content-Type': 'application/json',
        ...(options.header || {}),
      },
      success: (res) => {
        if (res.statusCode >= 200 && res.statusCode < 300) {
          resolve(res.data as T)
        } else {
          reject(res.data as ApiError)
        }
      },
      fail: (err) => reject(err),
    })
  })
}

export const http = {
  get: <T>(path: string) => request<T>(path, { method: 'GET' }),
  post: <T>(path: string, data?: unknown) =>
    request<T>(path, { method: 'POST', data }),
}

export interface CreditsMe {
  balance: number
  daily_grant: number
  generate_cost: number
  ad_reward: number
  daily_claimed_today: boolean
}

export interface CreditsMutation {
  balance: number
  delta: number
  reason: string
}

export const creditsApi = {
  me: () => http.get<CreditsMe>('/api/v1/credits/me'),
  dailyClaim: () => http.post<CreditsMutation>('/api/v1/credits/daily-claim'),
  adReward: () => http.post<CreditsMutation>('/api/v1/credits/ad-reward'),
}

export const draftsApi = {
  generate: (body: {
    image_ref?: string
    board_size?: string
    color_count?: number
  }) =>
    http.post<{
      job_id: string
      status: string
      balance: number
      message: string
    }>('/api/v1/drafts/generate', body),
}

export const authApi = {
  wxLogin: (code: string) =>
    http.post<{ token: string; user: { id: string; nickname: string } }>(
      '/api/v1/auth/wx-login',
      { code },
    ),
}
