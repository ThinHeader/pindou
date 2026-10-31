<script setup lang="ts">
import { onShow } from '@dcloudio/uni-app'
import { ref } from 'vue'
import CreditEmptySheet from '../../components/CreditEmptySheet.vue'
import CreditPill from '../../components/CreditPill.vue'
import { draftsApi, plazaApi } from '../../api/http'
import type { DraftCard, WorkCard } from '../../api/types'
import { useCreditsStore } from '../../stores/credits'
import { useUserStore } from '../../stores/user'
import { boardLabel } from '../../utils/ui'

const credits = useCreditsStore()
const user = useUserStore()
const drafts = ref<DraftCard[]>([])
const works = ref<WorkCard[]>([])

onShow(async () => {
  if (user.isLoggedIn) {
    await credits.refresh()
    try {
      const data = await draftsApi.list('mine')
      drafts.value = data.items.slice(0, 2)
    } catch {
      drafts.value = []
    }
  } else {
    drafts.value = []
  }
  try {
    const feed = await plazaApi.feed('recommend')
    works.value = feed.items.slice(0, 4)
  } catch {
    works.value = []
  }
})

function goCreate() {
  uni.switchTab({ url: '/pages/create/index' })
}
function goPlaza() {
  uni.switchTab({ url: '/pages/plaza/index' })
}
function goLibrary() {
  uni.switchTab({ url: '/pages/library/index' })
}
function openDraft(id: string) {
  uni.navigateTo({ url: `/pages/draft/detail?id=${id}` })
}
function openWork(id: string) {
  uni.navigateTo({ url: `/pages/work/detail?id=${id}` })
}
function stockSoon() {
  uni.showToast({ title: '豆色库存即将开放', icon: 'none' })
}
</script>

<template>
  <view class="page">
    <view class="hello-row">
      <view>
        <text class="hi">你好，{{ user.isLoggedIn ? user.nickname : '一起拼豆' }} 👋</text>
        <text class="headline">今天也来拼一颗可爱的豆吧</text>
      </view>
      <CreditPill />
    </view>

    <view class="cta" @click="goCreate">
      <text class="cta-ico">📷</text>
      <view class="cta-txt">
        <text class="cta-title">拍照出稿</text>
        <text class="cta-sub">一键把照片变成可拼色号稿</text>
      </view>
      <text class="cta-arrow">›</text>
    </view>

    <view class="shortcuts">
      <view class="shortcut" @click="goCreate"><text class="sico">🖼</text>相册出稿</view>
      <view class="shortcut" @click="goPlaza"><text class="sico">▦</text>逛广场</view>
      <view class="shortcut" @click="goLibrary"><text class="sico">▤</text>我的稿件</view>
      <view class="shortcut" @click="stockSoon">
        <text class="sico">🎨</text>豆色库存
      </view>
    </view>

    <view class="sec-title">
      <text>继续拼</text>
      <text class="sec-link" @click="goLibrary">全部</text>
    </view>
    <view v-if="drafts.length" class="stack">
      <view v-for="item in drafts" :key="item.id" class="continue" @click="openDraft(item.id)">
        <image class="thumb" :src="item.preview_url" mode="aspectFill" />
        <view class="meta">
          <text class="name">{{ item.title }}</text>
          <text class="sub">{{ boardLabel(item.board_size, item.color_count) }}</text>
        </view>
        <text class="go">继续</text>
      </view>
    </view>
    <view v-else class="empty">登录并出稿后，未完成的稿会出现在这里</view>

    <view class="sec-title">
      <text>今日推荐</text>
      <text class="sec-link" @click="goPlaza">更多</text>
    </view>
    <view v-if="works.length" class="grid">
      <view v-for="item in works" :key="item.id" class="rec" @click="openWork(item.id)">
        <image class="cover" :src="item.preview_url" mode="aspectFill" />
        <text class="name">{{ item.title }}</text>
        <view class="subrow">
          <text>{{ boardLabel(item.board_size, item.color_count) }}</text>
          <text class="likes">♥ {{ item.like_count }}</text>
        </view>
      </view>
    </view>
    <view v-else class="empty">广场还在预热，稍后再来看看</view>

    <CreditEmptySheet />
  </view>
</template>

<style scoped lang="scss">
.page { padding: 24rpx 32rpx 140rpx; }
.hello-row { display: flex; justify-content: space-between; align-items: flex-start; gap: 16rpx; }
.hi { display: block; font-size: 26rpx; color: $ink-2; }
.headline { display: block; margin-top: 8rpx; font-size: 40rpx; font-weight: 800; line-height: 1.3; }
.cta {
  margin-top: 28rpx;
  display: flex;
  align-items: center;
  gap: 20rpx;
  background: linear-gradient(135deg, $brand, $warm);
  border-radius: 32rpx;
  padding: 36rpx 32rpx;
  color: #fff;
  box-shadow: 0 16rpx 40rpx rgba(255, 107, 91, 0.28);
}
.cta-ico { font-size: 48rpx; }
.cta-txt { flex: 1; }
.cta-title { display: block; font-size: 36rpx; font-weight: 800; }
.cta-sub { display: block; margin-top: 6rpx; font-size: 24rpx; opacity: 0.92; }
.cta-arrow { font-size: 40rpx; opacity: 0.85; }
.shortcuts { display: flex; gap: 12rpx; margin-top: 24rpx; }
.shortcut {
  flex: 1;
  background: $card;
  border-radius: 20rpx;
  border: 1rpx solid $line;
  padding: 18rpx 8rpx;
  text-align: center;
  font-size: 22rpx;
  color: $ink-2;
}
.sico { display: block; font-size: 32rpx; margin-bottom: 6rpx; }
.stack { display: flex; flex-direction: column; gap: 16rpx; }
.continue {
  display: flex;
  align-items: center;
  gap: 16rpx;
  background: $card;
  border-radius: 24rpx;
  border: 1rpx solid $line;
  padding: 16rpx;
}
.thumb { width: 96rpx; height: 96rpx; border-radius: 16rpx; background: $brand-lighter; }
.meta { flex: 1; }
.name { display: block; font-weight: 700; font-size: 28rpx; }
.sub, .subrow { color: $ink-3; font-size: 22rpx; }
.go {
  background: $brand;
  color: #fff;
  border-radius: 999rpx;
  padding: 10rpx 22rpx;
  font-size: 24rpx;
  font-weight: 700;
}
.grid { display: flex; flex-wrap: wrap; gap: 16rpx; }
.rec {
  width: calc(50% - 8rpx);
  background: $card;
  border-radius: 24rpx;
  border: 1rpx solid $line;
  overflow: hidden;
}
.cover { width: 100%; height: 280rpx; background: $brand-lighter; display: block; }
.rec .name { display: block; padding: 12rpx 16rpx 0; }
.subrow { display: flex; justify-content: space-between; padding: 6rpx 16rpx 16rpx; }
.likes { color: $brand; }
</style>
