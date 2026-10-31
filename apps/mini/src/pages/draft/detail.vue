<script setup lang="ts">
import { onLoad } from '@dcloudio/uni-app'
import { computed, ref } from 'vue'
import PaletteLegend from '../../components/PaletteLegend.vue'
import { draftsApi } from '../../api/http'
import type { DraftDetail } from '../../api/types'
import { boardLabel, showError, showTip, timeAgo } from '../../utils/ui'

const draft = ref<DraftDetail | null>(null)
const crafting = ref(false)
const step = ref(0)

const ordered = computed(() => {
  const list = draft.value?.palette ? [...draft.value.palette] : []
  return list.sort((a, b) => b.count - a.count)
})
const current = computed(() => ordered.value[step.value])

onLoad(async (query) => {
  if (!query?.id) return
  try {
    draft.value = await draftsApi.detail(String(query.id))
  } catch (e) {
    showError(e, '稿件不存在')
  }
})

async function toggleFav() {
  if (!draft.value) return
  try {
    const res = await draftsApi.favorite(draft.value.id)
    draft.value.favorited = res.favorited
    showTip(res.favorited ? '已收藏' : '已取消收藏')
  } catch (e) {
    showError(e)
  }
}

function publish() {
  if (!draft.value) return
  if (draft.value.is_official) {
    showTip('官方模板不能直接发布')
    return
  }
  if (draft.value.work_id) {
    uni.navigateTo({ url: `/pages/work/detail?id=${draft.value.work_id}` })
    return
  }
  const cats = [
    { id: 'pet', label: '萌宠' },
    { id: 'food', label: '食物' },
    { id: 'pixel', label: '像素人' },
    { id: 'festival', label: '节日' },
    { id: 'simple', label: '简约' },
  ]
  uni.showActionSheet({
    itemList: cats.map((c) => c.label),
    success: async (res) => {
      const category = cats[res.tapIndex]?.id
      if (!category || !draft.value) return
      try {
        const work = await draftsApi.publish(draft.value.id, {
          category,
          caption: '从稿件库发布',
        })
        draft.value.work_id = work.id
        showTip('已发布到广场')
        uni.navigateTo({ url: `/pages/work/detail?id=${work.id}` })
      } catch (e) {
        showError(e, '发布失败')
      }
    },
  })
}

function exportImage() {
  const url = draft.value?.preview_url
  if (!url) return
  uni.previewImage({ urls: [url] })
  showTip('长按图片可保存')
}

function nextBead() {
  if (!ordered.value.length) return
  if (step.value >= ordered.value.length - 1) {
    showTip('这一版拼完了')
    crafting.value = false
    step.value = 0
    return
  }
  step.value += 1
}
</script>

<template>
  <view v-if="draft" class="page">
    <image class="hero" :src="draft.preview_url" mode="aspectFit" />
    <view class="title-row">
      <view>
        <text class="title">{{ draft.title }}</text>
        <text class="sub">{{ timeAgo(draft.created_at) }} · {{ draft.author_nickname }} · {{ draft.work_id ? '已发布' : '未发布到广场' }}</text>
      </view>
      <text class="fav" @click="toggleFav">{{ draft.favorited ? '♥' : '♡' }}</text>
    </view>
    <view class="stats">
      <view class="stat"><text class="n">{{ draft.board_size }}×{{ draft.board_size }}</text><text>底板</text></view>
      <view class="stat"><text class="n">{{ draft.color_count }}</text><text>色数</text></view>
      <view class="stat"><text class="n">{{ draft.peg_count }}</text><text>钉位</text></view>
    </view>

    <view class="block">
      <text class="h">用豆清单</text>
      <PaletteLegend :items="draft.palette" />
    </view>
    <view class="block">
      <text class="h">拼装提示</text>
      <text class="p">{{ draft.hint }}</text>
      <text class="p dim">{{ boardLabel(draft.board_size, draft.color_count) }}</text>
    </view>

    <view v-if="crafting && current" class="craft">
      <view class="swatch" :style="{ background: current.hex }" />
      <view>
        <text class="h">现在铺 {{ current.code }} {{ current.name }}</text>
        <text class="p">用量 {{ current.count }} · 第 {{ step + 1 }}/{{ ordered.length }} 色</text>
      </view>
      <text class="next" @click="nextBead">下一种</text>
    </view>

    <view class="actions">
      <view class="btn ghost" @click="exportImage">导出</view>
      <view class="btn soft" @click="publish">{{ draft.work_id ? '看作品' : '发布' }}</view>
      <view class="btn primary" @click="crafting = true">开拼</view>
    </view>
  </view>
  <view v-else class="page"><view class="empty">加载稿件…</view></view>
</template>

<style scoped lang="scss">
.page { padding: 24rpx 32rpx 48rpx; }
.hero { width: 100%; height: 640rpx; background: $brand-lighter; border-radius: 28rpx; }
.title-row { display: flex; justify-content: space-between; align-items: flex-start; margin-top: 20rpx; }
.title { display: block; font-size: 40rpx; font-weight: 800; }
.sub { display: block; margin-top: 6rpx; color: $ink-3; font-size: 24rpx; }
.fav { font-size: 40rpx; color: $brand; padding: 8rpx; }
.stats { display: flex; background: $card; border-radius: 24rpx; border: 1rpx solid $line; margin-top: 20rpx; }
.stat { flex: 1; text-align: center; padding: 22rpx 0; color: $ink-3; font-size: 22rpx; }
.n { display: block; color: $ink; font-size: 32rpx; font-weight: 800; }
.block { margin-top: 24rpx; background: $card; border-radius: 24rpx; border: 1rpx solid $line; padding: 24rpx; }
.h { display: block; font-weight: 750; margin-bottom: 12rpx; }
.p { display: block; color: $ink-2; font-size: 26rpx; line-height: 1.5; }
.dim { margin-top: 8rpx; color: $ink-3; }
.craft {
  margin-top: 20rpx; display: flex; align-items: center; gap: 16rpx;
  background: $brand-lighter; border-radius: 24rpx; padding: 20rpx;
}
.swatch { width: 64rpx; height: 64rpx; border-radius: 16rpx; }
.next { margin-left: auto; color: $brand-dark; font-weight: 750; }
.actions { display: flex; gap: 12rpx; margin-top: 28rpx; }
.btn { flex: 1; text-align: center; padding: 22rpx 8rpx; border-radius: 999rpx; font-weight: 700; font-size: 26rpx; }
.ghost { background: $card; border: 1rpx solid $line; }
.soft { background: $brand-soft; color: $brand-dark; }
.primary { flex: 1.2; background: $brand; color: #fff; }
</style>
