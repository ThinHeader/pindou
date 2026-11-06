<script setup lang="ts">
import { onCopyUrl, onLoad, onShareAppMessage, onShareTimeline } from '@dcloudio/uni-app'
import { computed, ref } from 'vue'
import PaletteLegend from '../../components/PaletteLegend.vue'
import { draftsApi } from '../../api/http'
import type { DraftDetail } from '../../api/types'
import { useCreateStore } from '../../stores/create'
import { SHARE_PAGES, shareTitle, withQuery } from '../../utils/share'
import { askConfirm, askText, boardLabel, checkedDraftTitle, ensureLogin, showError, showTip, timeAgo } from '../../utils/ui'
import { usePageShare } from '../../utils/use-share'

const create = useCreateStore()

const draft = ref<DraftDetail | null>(null)

usePageShare(onShareAppMessage, onShareTimeline, onCopyUrl, () => ({
  title: shareTitle(draft.value?.title),
  path: draft.value?.id ? withQuery(SHARE_PAGES.draftDetail, { id: draft.value.id }) : undefined,
}))

const crafting = ref(false)
const step = ref(0)

const ordered = computed(() => {
  const list = draft.value?.palette ? [...draft.value.palette] : []
  return list.sort((a, b) => b.count - a.count)
})
const current = computed(() => ordered.value[step.value])
const owned = computed(() => draft.value?.badge === '我的')

onLoad(async (query) => {
  if (!query?.id) return
  try {
    draft.value = await draftsApi.detail(String(query.id))
  } catch (e) {
    showError(e, '稿件不存在')
  }
})

async function toggleFav() {
  if (!ensureLogin() || !draft.value) return
  try {
    const res = await draftsApi.favorite(draft.value.id)
    draft.value.favorited = res.favorited
    showTip(res.favorited ? '已收藏' : '已取消收藏')
  } catch (e) {
    showError(e)
  }
}

async function rename() {
  if (!ensureLogin() || !draft.value || !owned.value) return
  const current = draft.value.title === '未命名草稿' ? '' : draft.value.title
  const typed = await askText('修改稿件名称', current, '例如：窗边的猫')
  if (typed === null || !draft.value) return
  const title = checkedDraftTitle(typed)
  if (!title) return
  try {
    draft.value = await draftsApi.rename(draft.value.id, title)
    showTip('已改名')
  } catch (e) {
    showError(e, '改名失败')
  }
}

async function remove() {
  if (!ensureLogin() || !draft.value || !owned.value) return
  const current = draft.value
  const ok = await askConfirm('删除稿件', `删除后无法恢复。确定删除「${current.title}」吗？`)
  if (!ok) return
  try {
    await draftsApi.remove(current.id)
    showTip('已删除')
    setTimeout(() => {
      uni.navigateBack({
        fail: () => uni.switchTab({ url: '/pages/library/index' }),
      })
    }, 400)
  } catch (e) {
    showError(e, '删除失败')
  }
}

function exportImage() {
  const url = draft.value?.preview_url
  if (!url) return
  uni.previewImage({ urls: [url] })
  showTip('长按图片可保存')
}

function revise() {
  if (!ensureLogin() || !draft.value) return
  uni.navigateTo({ url: `/pages/edit/board?id=${draft.value.id}` })
}

function again() {
  if (!ensureLogin()) return
  if (!draft.value?.source_key) {
    showTip('这张稿没有原图，请重新选图')
    uni.switchTab({ url: '/pages/create/index' })
    return
  }
  create.reuseDraft(draft.value)
  uni.navigateTo({ url: '/pages/create/params' })
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
        <text v-if="owned" class="rename" @click="rename">改名</text>
        <text class="sub">{{ timeAgo(draft.created_at) }} · {{ boardLabel(draft.board_size, draft.color_count) }}</text>
      </view>
      <text class="fav" @click="toggleFav">{{ draft.favorited ? '♥' : '♡' }}</text>
    </view>
    <view v-if="draft.pipeline_tags?.length" class="tags">
      <text v-for="tag in draft.pipeline_tags" :key="tag" class="tag">{{ tag }}</text>
    </view>
    <view class="stats">
      <view class="stat"><text class="n">{{ draft.board_size }}×{{ draft.board_size }}</text><text>底板</text></view>
      <view class="stat"><text class="n">{{ draft.color_count }}</text><text>色数</text></view>
      <view class="stat"><text class="n">{{ draft.peg_count }}</text><text>钉位</text></view>
    </view>

    <view v-if="draft.pipeline_lines?.length" class="block">
      <text class="h">处理说明</text>
      <text v-for="line in draft.pipeline_lines" :key="line" class="p">· {{ line }}</text>
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

    <view v-if="owned" class="revise" @click="revise">改这张稿</view>
    <view v-if="owned && draft.source_key" class="again" @click="again">再次出稿</view>
    <view class="actions">
      <view class="btn ghost" @click="exportImage">导出</view>
      <view class="btn primary" @click="crafting = true">开拼</view>
    </view>
    <text v-if="owned" class="danger" @click="remove">删除稿件</text>
  </view>
  <view v-else class="page"><view class="empty">加载稿件…</view></view>
</template>

<style scoped lang="scss">
.page { padding: 24rpx 32rpx 48rpx; }
.hero { width: 100%; height: 640rpx; background: $brand-lighter; border-radius: 28rpx; }
.title-row { display: flex; justify-content: space-between; align-items: flex-start; margin-top: 20rpx; }
.title { display: block; font-size: 40rpx; font-weight: 800; }
.rename { display: inline-block; margin-top: 8rpx; color: $brand-dark; font-size: 24rpx; font-weight: 650; }
.sub { display: block; margin-top: 6rpx; color: $ink-3; font-size: 24rpx; }
.fav { font-size: 40rpx; color: $brand; padding: 8rpx; }
.tags { display: flex; flex-wrap: wrap; gap: 10rpx; margin-top: 16rpx; }
.tag { background: $brand-lighter; color: $brand-dark; border-radius: 999rpx; padding: 6rpx 16rpx; font-size: 22rpx; }
.stats { display: flex; background: $card; border-radius: 24rpx; border: 1rpx solid $line; margin-top: 20rpx; }
.revise {
  margin-top: 24rpx;
  text-align: center;
  background: $card;
  color: $brand-dark;
  border: 1rpx solid $brand;
  border-radius: 999rpx;
  padding: 22rpx;
  font-weight: 700;
}
.again {
  margin-top: 16rpx;
  text-align: center;
  background: $brand;
  color: #fff;
  border-radius: 999rpx;
  padding: 24rpx;
  font-weight: 700;
}
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
.danger { display: block; text-align: center; margin-top: 28rpx; color: $ink-3; font-size: 24rpx; }
</style>
