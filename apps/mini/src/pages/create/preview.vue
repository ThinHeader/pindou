<script setup lang="ts">
import { onLoad } from '@dcloudio/uni-app'
import { ref } from 'vue'
import PaletteLegend from '../../components/PaletteLegend.vue'
import { draftsApi } from '../../api/http'
import type { DraftDetail } from '../../api/types'
import { useCreateStore } from '../../stores/create'
import { boardLabel, showError, showTip } from '../../utils/ui'

const create = useCreateStore()
const draft = ref<DraftDetail | null>(create.lastDraft)

onLoad(async (query) => {
  const id = query?.id || create.lastDraft?.id
  if (!id) return
  try {
    draft.value = await draftsApi.detail(String(id))
  } catch (e) {
    showError(e, '预览加载失败')
  }
})

function saveImage() {
  const url = draft.value?.preview_url
  if (!url) return
  uni.downloadFile({
    url,
    success: (res) => {
      uni.saveImageToPhotosAlbum({
        filePath: res.tempFilePath,
        success: () => showTip('已保存到相册'),
        fail: () => uni.previewImage({ urls: [url] }),
      })
    },
    fail: () => uni.previewImage({ urls: [url] }),
  })
}

function toLibrary() {
  const id = draft.value?.id
  if (!id) {
    uni.switchTab({ url: '/pages/library/index' })
    return
  }
  uni.redirectTo({ url: `/pages/draft/detail?id=${id}` })
}
</script>

<template>
  <view class="page">
    <view v-if="draft" class="stage">
      <view class="head">
        <text class="title">{{ draft.title }}</text>
        <text class="meta">{{ boardLabel(draft.board_size, draft.color_count) }} · {{ draft.peg_count }} 钉</text>
      </view>
      <image class="preview" :src="draft.preview_url" mode="aspectFit" />
      <view class="sec-title"><text>色号图例</text></view>
      <PaletteLegend :items="draft.palette" />
    </view>
    <view v-else class="empty">还没有生成结果</view>
    <view class="actions">
      <view class="btn ghost" @click="saveImage">保存图片</view>
      <view class="btn primary" @click="toLibrary">存入稿件库</view>
    </view>
    <text class="note">生成成功时已经写入稿件库，这里可以继续查看用豆清单。</text>
  </view>
</template>

<style scoped lang="scss">
.page { padding: 24rpx 32rpx 48rpx; }
.stage { background: $card; border-radius: 28rpx; border: 1rpx solid $line; padding: 24rpx; }
.head { display: flex; justify-content: space-between; align-items: baseline; gap: 12rpx; margin-bottom: 12rpx; }
.title { font-size: 32rpx; font-weight: 800; }
.meta { color: $ink-3; font-size: 22rpx; }
.preview { width: 100%; height: 640rpx; background: $brand-lighter; border-radius: 20rpx; }
.actions { display: flex; gap: 16rpx; margin-top: 28rpx; }
.btn { flex: 1; text-align: center; padding: 24rpx; border-radius: 999rpx; font-weight: 700; }
.ghost { background: $card; border: 1rpx solid $line; }
.primary { flex: 1.3; background: $brand; color: #fff; }
.note { display: block; margin-top: 16rpx; color: $ink-3; font-size: 22rpx; line-height: 1.5; }
</style>
