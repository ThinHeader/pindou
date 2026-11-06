<script setup lang="ts">
import { onCopyUrl, onLoad, onShareAppMessage, onShareTimeline } from '@dcloudio/uni-app'
import { ref } from 'vue'
import { tutorialById, type Tutorial } from '../../data/tutorials'
import { useCreateStore } from '../../stores/create'
import { SHARE_PAGES, shareTitle, withQuery } from '../../utils/share'
import { usePageShare } from '../../utils/use-share'

const create = useCreateStore()
const item = ref<Tutorial | null>(null)

usePageShare(onShareAppMessage, onShareTimeline, onCopyUrl, () => ({
  title: shareTitle(item.value?.title),
  path: item.value ? withQuery(SHARE_PAGES.tutorialDetail, { id: item.value.id }) : SHARE_PAGES.tutorials,
}))

onLoad((query) => {
  item.value = tutorialById(String(query?.id || '')) ?? null
})

function goCreate() {
  if (item.value) create.referenceTitle = item.value.title
  uni.switchTab({ url: '/pages/create/index' })
}
</script>

<template>
  <view v-if="item" class="page">
    <view class="hero" :style="{ background: item.tint }">
      <text class="emoji">{{ item.cover }}</text>
    </view>
    <text class="kicker">{{ item.kicker }} · 官方教程</text>
    <text class="title">{{ item.title }}</text>
    <text class="summary">{{ item.summary }}</text>
    <view v-for="(step, index) in item.steps" :key="step.title" class="step">
      <text class="idx">{{ index + 1 }}</text>
      <view class="body">
        <text class="h">{{ step.title }}</text>
        <text class="p">{{ step.body }}</text>
      </view>
    </view>
    <view class="do" @click="goCreate">去出稿</view>
  </view>
  <view v-else class="page">
    <view class="empty">没有这篇教程</view>
  </view>
</template>

<style scoped lang="scss">
.page { padding: 24rpx 32rpx 180rpx; }
.hero {
  height: 280rpx;
  border-radius: 28rpx;
  display: flex;
  align-items: center;
  justify-content: center;
}
.emoji { font-size: 96rpx; }
.kicker { display: block; margin-top: 20rpx; color: $brand-dark; font-size: 24rpx; }
.title { display: block; margin-top: 8rpx; font-size: 40rpx; font-weight: 800; }
.summary { display: block; margin-top: 12rpx; color: $ink-2; line-height: 1.5; }
.step {
  display: flex;
  gap: 16rpx;
  margin-top: 20rpx;
  background: $card;
  border: 1rpx solid $line;
  border-radius: 24rpx;
  padding: 20rpx;
}
.idx {
  width: 48rpx;
  height: 48rpx;
  border-radius: 50%;
  background: $brand-soft;
  color: $brand-dark;
  text-align: center;
  line-height: 48rpx;
  font-weight: 800;
  flex-shrink: 0;
}
.body { flex: 1; min-width: 0; }
.h { display: block; font-weight: 750; }
.p { display: block; margin-top: 6rpx; color: $ink-2; font-size: 26rpx; line-height: 1.5; }
.do {
  position: fixed;
  left: 32rpx;
  right: 32rpx;
  bottom: calc(24rpx + env(safe-area-inset-bottom));
  text-align: center;
  background: $brand;
  color: #fff;
  border-radius: 999rpx;
  padding: 24rpx;
  font-weight: 800;
  box-shadow: 0 12rpx 28rpx rgba(255, 107, 91, 0.3);
}
</style>
