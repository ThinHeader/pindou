<script setup lang="ts">
import { onShow } from '@dcloudio/uni-app'
import { ref } from 'vue'
import CreditEmptySheet from '../../components/CreditEmptySheet.vue'
import CreditPill from '../../components/CreditPill.vue'
import { draftsApi } from '../../api/http'
import type { DraftCard } from '../../api/types'
import { useCreateStore } from '../../stores/create'
import { useCreditsStore } from '../../stores/credits'
import { useUserStore } from '../../stores/user'
import { ensureLogin, showError } from '../../utils/ui'

const credits = useCreditsStore()
const user = useUserStore()
const create = useCreateStore()
const recent = ref<DraftCard[]>([])

onShow(async () => {
  if (!user.isLoggedIn) {
    recent.value = []
    return
  }
  await credits.refresh()
  try {
    const data = await draftsApi.list('mine')
    recent.value = data.items.slice(0, 6)
  } catch {
    recent.value = []
  }
})

function prepareTitle() {
  create.title = create.referenceTitle.trim()
}

function pick(source: 'camera' | 'album') {
  if (!ensureLogin()) return
  if (!credits.canGenerate) {
    credits.openEmptySheet()
    return
  }
  prepareTitle()
  uni.chooseImage({
    count: 1,
    sizeType: ['compressed'],
    sourceType: [source],
    success: async (res) => {
      const path = res.tempFilePaths[0]
      const name = path.split('/').pop() || '参考图'
      uni.showLoading({ title: '上传中' })
      try {
        await create.attachLocal(path, name)
        uni.navigateTo({ url: '/pages/create/params' })
      } catch (e) {
        showError(e, '读取图片失败')
      } finally {
        uni.hideLoading()
      }
    },
  })
}

function useSample() {
  if (!ensureLogin()) return
  if (!credits.canGenerate) {
    credits.openEmptySheet()
    return
  }
  prepareTitle()
  create.useBuiltin('cat', '小橘猫')
  uni.navigateTo({ url: '/pages/create/params' })
}

function openDraft(id: string) {
  uni.navigateTo({ url: `/pages/draft/detail?id=${id}` })
}
function goLibrary() {
  uni.switchTab({ url: '/pages/library/index' })
}
</script>

<template>
  <view class="page">
    <view class="top">
      <view>
        <text class="title">出稿</text>
        <text class="sub">积分 {{ credits.balance }} · 出稿消耗 {{ credits.generateCost }}</text>
      </view>
      <CreditPill compact />
    </view>

    <view v-if="create.referenceTitle" class="ref">参考作品：{{ create.referenceTitle }}</view>

    <view class="pixels">
      <view v-for="c in ['#FF6B5B', '#FFB088', '#FFD166', '#7BDFF2', '#C9B1FF']" :key="c" class="px" :style="{ background: c }" />
    </view>
    <text class="lead">上传一张图片，一键变成可拼的色号稿</text>

    <view class="upload">
      <text class="big">📷</text>
      <text class="h">添加参考图</text>
      <text class="p">支持 JPG / PNG · 建议主体清晰、背景简洁</text>
      <view class="actions">
        <view class="btn primary" @click="pick('camera')">拍照</view>
        <view class="btn ghost" @click="pick('album')">从相册选择</view>
      </view>
      <view class="sample" @click="useSample">没有照片？用示例图试试</view>
      <text class="tip">每次成功出稿消耗 1 积分</text>
    </view>

    <view class="sec-title">
      <text>最近草稿</text>
      <text class="sec-link" @click="goLibrary">全部</text>
    </view>
    <scroll-view v-if="recent.length" scroll-x class="hscroll" :show-scrollbar="false">
      <view v-for="item in recent" :key="item.id" class="mini" @click="openDraft(item.id)">
        <image class="thumb" :src="item.preview_url" mode="aspectFill" />
        <text class="name">{{ item.title }}</text>
      </view>
    </scroll-view>
    <view v-else class="empty">还没有草稿</view>

    <view class="sec-title"><text>出稿小贴士</text></view>
    <view class="tips">
      <text>· 主体居中、光线均匀，生成更干净</text>
      <text>· 色数越少越适合新手开拼</text>
      <text>· 52×52 约 13.5cm · 78×78 约 20cm · 104×104 约 27cm</text>
    </view>
    <CreditEmptySheet />
  </view>
</template>

<style scoped lang="scss">
.page { padding: 24rpx 32rpx 140rpx; }
.top { display: flex; justify-content: space-between; align-items: center; }
.title { display: block; font-size: 40rpx; font-weight: 800; }
.sub { display: block; margin-top: 6rpx; color: $ink-3; font-size: 24rpx; }
.ref {
  margin-top: 16rpx;
  background: $brand-lighter;
  color: $brand-dark;
  border-radius: 16rpx;
  padding: 16rpx 20rpx;
  font-size: 24rpx;
}
.pixels { display: flex; gap: 10rpx; justify-content: center; margin: 28rpx 0 12rpx; }
.px { width: 22rpx; height: 22rpx; border-radius: 6rpx; }
.lead { display: block; text-align: center; color: $ink-2; font-size: 26rpx; margin-bottom: 20rpx; }
.upload {
  background: $card;
  border: 2rpx dashed #F0C4BC;
  border-radius: 32rpx;
  padding: 48rpx 32rpx 32rpx;
  text-align: center;
}
.big { font-size: 64rpx; }
.h { display: block; margin-top: 8rpx; font-size: 34rpx; font-weight: 800; }
.p { display: block; margin-top: 8rpx; color: $ink-3; font-size: 24rpx; }
.actions { display: flex; gap: 16rpx; margin-top: 24rpx; }
.btn { flex: 1; text-align: center; padding: 20rpx 8rpx; border-radius: 999rpx; font-weight: 700; font-size: 28rpx; }
.primary { background: $brand; color: #fff; }
.ghost { background: $bg; color: $ink; border: 1rpx solid $line; }
.sample { margin-top: 20rpx; color: $brand-dark; font-size: 24rpx; font-weight: 650; }
.tip { display: block; margin-top: 12rpx; color: $ink-3; font-size: 22rpx; }
.hscroll { white-space: nowrap; }
.mini {
  display: inline-block;
  width: 180rpx;
  margin-right: 16rpx;
  background: $card;
  border-radius: 20rpx;
  border: 1rpx solid $line;
  overflow: hidden;
  vertical-align: top;
}
.thumb { width: 180rpx; height: 180rpx; background: $brand-lighter; display: block; }
.name { display: block; padding: 12rpx; font-size: 22rpx; font-weight: 650; white-space: nowrap; overflow: hidden; }
.tips {
  background: $brand-lighter;
  border-radius: 20rpx;
  padding: 24rpx 28rpx;
  display: flex;
  flex-direction: column;
  gap: 8rpx;
  color: $ink-2;
  font-size: 24rpx;
}
</style>
