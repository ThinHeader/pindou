/// <reference types="@dcloudio/types" />
/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_API_BASE?: string
  readonly VITE_WX_LOGIN?: string
}

interface WxMiniGlobal {
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

declare const wx: WxMiniGlobal | undefined

declare module '*.vue' {
  import type { DefineComponent } from 'vue'
  const component: DefineComponent<object, object, unknown>
  export default component
}
