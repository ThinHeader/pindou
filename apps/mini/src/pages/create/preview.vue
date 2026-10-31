<script setup lang="ts">
import { onLoad } from '@dcloudio/uni-app'
import { computed, ref } from 'vue'
import PaletteLegend from '../../components/PaletteLegend.vue'
import { draftsApi } from '../../api/http'
import type { DraftDetail } from '../../api/types'
import { useCreateStore } from '../../stores/create'
import { askText, boardLabel, showError, showTip } from '../../utils/ui'

const create = useCreateStore()
const draft = ref<DraftDetail | null>(create.lastDraft)
const tab = ref<'orig' | 'anime' | 'bead'>('bead')

const tabs = [
  { id: 'orig' as const, label: '原图' },
  { id: 'anime' as const, label: '卡通中间态' },
  { id: 'bead' as const, label: '拼豆结果' },
]

const beadTotal = computed(() => draft.value?.palette.reduce((sum, item) => sum + item.count, 0) || 0)

onLoad(async (query) => {
  const id = query?.id || create.lastDraft?.id
  if (!id) return
  try {
    draft.value = await draftsApi.detail(String(id))
    create.lastDraft = draft.value
  } catch (e) {
    showError(e, '预览加载失败')
  }
})

function retune() {
  if (draft.value?.source_key) create.reuseDraft(draft.value)
  uni.navigateBack({
    fail: () => uni.redirectTo({ url: '/pages/create/params' }),
  })
}

function saveDraft() {
  const id = draft.value?.id
  showTip('已保存草稿')
  if (!id) {
    uni.switchTab({ url: '/pages/library/index' })
    return
  }
  uni.redirectTo({ url: `/pages/draft/detail?id=${id}` })
}

function share() {
  showTip('请点右上角分享')
}

async function publish() {
  if (!draft.value) return
  if (draft.value.work_id) {
    uni.navigateTo({ url: `/pages/work/detail?id=${draft.value.work_id}` })
    return
  }
  const current = draft.value.title === '未命名草稿' ? '' : draft.value.title
  const typed = await askText('发布到广场', current, '给作品起个名字')
  if (typed === null || !draft.value) return
  if ([...typed].length > 40) {
    showError({ message: '稿件名称请控制在 40 字以内' })
    return
  }
  const title = typed
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
          caption: '从出稿预览发布',
          title: title || undefined,
        })
        draft.value.work_id = work.id
        if (title) draft.value.title = work.title
        showTip('已发布到广场')
        uni.navigateTo({ url: `/pages/work/detail?id=${work.id}` })
      } catch (e) {
        showError(e, '发布失败')
      }
    },
  })
}
</script>

<template>
  <view class="page">
    <view v-if="draft" class="stage">
      <view class="tabs">
        <view
          v-for="item in tabs"
          :key="item.id"
          class="chip"
          :class="{ on: tab === item.id }"
          @click="tab = item.id"
        >{{ item.label }}</view>
      </view>
      <view class="head">
        <text class="title">{{ draft.title }}</text>
        <text class="meta">{{ boardLabel(draft.board_size, draft.color_count) }}</text>
      </view>

      <image v-if="tab === 'orig'" class="preview photo" :src="draft.original_url || draft.source_url || draft.preview_url" mode="aspectFit" />
      <view v-if="tab === 'orig' && !(draft.original_url || draft.source_url)" class="miss">这张稿没有留下原图</view>
      <image v-if="tab === 'anime'" class="preview" :src="draft.cartoon_url || draft.preview_url" mode="aspectFit" />
      <view v-if="tab === 'anime' && !draft.cartoon_url" class="miss">没有卡通中间态</view>
      <image v-if="tab === 'bead'" class="preview" :src="draft.preview_url" mode="aspectFit" />

      <view v-if="draft.pipeline_tags?.length" class="tags">
        <text v-for="tag in draft.pipeline_tags" :key="tag" class="tag">{{ tag }}</text>
      </view>

      <view class="sec-title"><text>色号统计</text></view>
      <PaletteLegend :items="draft.palette" />
      <text class="foot">共用 <text class="em">{{ draft.palette.length }}</text> 色 · 合计 <text class="em">{{ beadTotal }}</text> 颗豆</text>
    </view>
    <view v-else class="empty">还没有生成结果</view>
    <view class="actions">
      <view class="row">
        <view class="btn ghost" @click="saveDraft">保存草稿</view>
        <view class="btn soft" @click="retune">换参数重出</view>
      </view>
      <view class="btn primary" @click="publish">发布到广场</view>
      <text class="share" @click="share">分享</text>
    </view>
    <text class="note">生成成功时已经写入稿件库，并扣除 1 积分。失败的任务不会出现在这里。</text>
  </view>
</template>

<style scoped lang="scss">
.page { padding: 24rpx 32rpx 48rpx; }
.stage { background: $card; border-radius: 28rpx; border: 1rpx solid $line; padding: 24rpx; }
.tabs { display: flex; gap: 12rpx; flex-wrap: wrap; margin-bottom: 16rpx; }
.chip {
  padding: 10rpx 22rpx;
  border-radius: 999rpx;
  background: $bg;
  color: $ink-2;
  font-size: 24rpx;
}
.chip.on { background: $brand-soft; color: $brand-dark; font-weight: 700; }
.head { display: flex; justify-content: space-between; align-items: baseline; gap: 12rpx; margin-bottom: 12rpx; }
.title { font-size: 32rpx; font-weight: 800; }
.meta { color: $ink-3; font-size: 22rpx; }
.preview { width: 100%; height: 640rpx; background: $brand-lighter; border-radius: 20rpx; }
.photo { background: $warm-soft; }
.miss { margin-top: 8rpx; color: $ink-3; font-size: 22rpx; }
.tags { display: flex; flex-wrap: wrap; gap: 10rpx; margin-top: 16rpx; }
.tag { background: $brand-lighter; color: $brand-dark; border-radius: 999rpx; padding: 6rpx 16rpx; font-size: 22rpx; }
.foot { display: block; margin-top: 12rpx; color: $ink-2; font-size: 24rpx; }
.em { color: $ink; font-weight: 800; }
.actions { margin-top: 28rpx; display: flex; flex-direction: column; gap: 16rpx; }
.row { display: flex; gap: 16rpx; }
.btn { text-align: center; padding: 24rpx; border-radius: 999rpx; font-weight: 700; }
.ghost { flex: 1; background: $card; border: 1rpx solid $line; }
.soft { flex: 1; background: $brand-soft; color: $brand-dark; }
.primary { background: $brand; color: #fff; }
.share { text-align: center; color: $ink-3; font-size: 24rpx; }
.note { display: block; margin-top: 16rpx; color: $ink-3; font-size: 22rpx; line-height: 1.5; }
</style>
