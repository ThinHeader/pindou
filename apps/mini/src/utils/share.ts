export const SHARE_TITLE = '拼豆手账工坊'
export const HOME_PATH = '/pages/home/index'

export const SHARE_PAGES = {
  home: HOME_PATH,
  create: '/pages/create/index',
  library: '/pages/library/index',
  mine: '/pages/mine/index',
  edit: '/pages/edit/index',
  tutorials: '/pages/tutorials/index',
  tutorialDetail: '/pages/tutorials/detail',
  draftDetail: '/pages/draft/detail',
  preview: '/pages/create/preview',
} as const

const UNTITLED_DRAFT = '未命名草稿'

export interface PageShare {
  title?: string
  /** 以 / 开头的完整路径，可带 query。空着则用当前页。 */
  path?: string
  /** 朋友圈和复制链接的 query，不含 ?。空着则从 path 里取。 */
  query?: string
  imageUrl?: string
}

export interface ShareAppMessage {
  title: string
  path: string
  imageUrl?: string
}

export interface ShareTimeline {
  title: string
  query: string
  imageUrl?: string
}

export interface SharePageLike {
  route?: string
  options?: Record<string, unknown> | null
  $page?: { fullPath?: string } | null
}

export function shareTitle(name?: string | null): string {
  const trimmed = name?.trim() ?? ''
  if (!trimmed || trimmed === UNTITLED_DRAFT) return SHARE_TITLE
  return `${SHARE_TITLE} · ${trimmed}`
}

export function withQuery(path: string, params: Record<string, string | undefined | null>): string {
  const query = Object.entries(params)
    .filter((entry): entry is [string, string] => typeof entry[1] === 'string' && entry[1] !== '')
    .map(([key, value]) => `${encodeURIComponent(key)}=${encodeURIComponent(value)}`)
    .join('&')
  return query ? `${path}?${query}` : path
}

export function queryFromPath(path: string): string {
  const index = path.indexOf('?')
  if (index === -1) return ''
  return path.slice(index + 1).replace(/^\?/, '')
}

export function pathFromPage(page: SharePageLike | null | undefined): string {
  if (!page) return HOME_PATH
  const fullPath = page.$page?.fullPath
  if (typeof fullPath === 'string' && fullPath.startsWith('/pages/')) return fullPath
  const route = typeof page.route === 'string' ? page.route.replace(/^\/+/, '') : ''
  if (!route.startsWith('pages/')) return HOME_PATH
  const query = queryFromOptions(page.options)
  const path = `/${route}`
  return query ? `${path}?${query}` : path
}

export function currentPagePath(): string {
  try {
    const pages = getCurrentPages() as unknown as SharePageLike[]
    return pathFromPage(pages[pages.length - 1])
  } catch {
    return HOME_PATH
  }
}

export function buildShareAppMessage(input: PageShare = {}, currentPath = HOME_PATH): ShareAppMessage {
  const card: ShareAppMessage = {
    title: input.title?.trim() || SHARE_TITLE,
    path: normalizeSharePath(input.path?.trim() || currentPath || HOME_PATH),
  }
  const imageUrl = input.imageUrl?.trim()
  if (imageUrl) card.imageUrl = imageUrl
  return card
}

export function buildShareTimeline(input: PageShare = {}, currentPath = HOME_PATH): ShareTimeline {
  const message = buildShareAppMessage(input, currentPath)
  const query = input.query != null ? input.query.replace(/^\?/, '') : queryFromPath(message.path)
  const card: ShareTimeline = { title: message.title, query }
  if (message.imageUrl) card.imageUrl = message.imageUrl
  return card
}

function normalizeSharePath(path: string): string {
  if (!path) return HOME_PATH
  return path.startsWith('/') ? path : `/${path}`
}

function queryFromOptions(options: Record<string, unknown> | null | undefined): string {
  if (!options) return ''
  return Object.keys(options)
    .sort()
    .flatMap((key) => {
      const value = options[key]
      if (typeof value !== 'string' && typeof value !== 'number' && typeof value !== 'boolean') return []
      const text = String(value)
      if (!text) return []
      return [`${encodeURIComponent(key)}=${encodeURIComponent(text)}`]
    })
    .join('&')
}
