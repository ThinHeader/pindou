<script setup lang="ts">
import { onShow } from '@dcloudio/uni-app'
import { ref } from 'vue'
import { plazaApi } from '../../api/http'
import type { WorkCard } from '../../api/types'
import { boardLabel, initialOf, showError } from '../../utils/ui'

const chips = [
  { id: 'recommend', label: '推荐' },
  { id: 'hot', label: '热门' },
  { id: 'pet', label: '萌宠' },
  { id: 'food', label: '食物' },
  { id: 'pixel', label: '像素人' },
  { id: 'festival', label: '节日' },
  { id: 'simple', label: '简约' },
]
const tab = ref('recommend')
const keyword = ref('')
const items = ref<WorkCard[]>([])
const loading = ref(false)

async function load() {
  loading.value = true
  try {
    const data = await plazaApi.feed(tab.value, keyword.value.trim())
    items.value = data.items
  } catch (e) {
    items.value = []
    showError(e, '广场加载失败')
  } finally {
    loading.value = false
  }
}

onShow(() => {
  load()
})

function pick(id: string) {
  tab.value = id
  load()
}
function openWork(id: string) {
  uni.navigateTo({ url: `/pages/work/detail?id=${id}` })
}
</script>

<template>
  <view class="page">
    <view class="search">
      <text class="ico">⌕</text>
      <input
        v-model="keyword"
        class="input"
        confirm-type="search"
        placeholder="搜索作品"
        @confirm="load"
      />
    </view>
    <scroll-view scroll-x class="chips" :show-scrollbar="false">
      <view
        v-for="chip in chips"
        :key="chip.id"
        class="chip"
        :class="{ on: tab === chip.id }"
        @click="pick(chip.id)"
      >{{ chip.label }}</view>
    </scroll-view>
    <view v-if="loading" class="muted">加载中…</view>
    <view v-else-if="items.length" class="grid">
      <view v-for="item in items" :key="item.id" class="card" @click="openWork(item.id)">
        <image class="cover" :src="item.preview_url" mode="aspectFill" />
        <view class="meta">
          <text class="name">{{ item.title }}</text>
          <view class="author">
            <text class="avatar">{{ initialOf(item.author.nickname) }}</text>
            <text>{{ item.author.nickname }}</text>
          </view>
          <view class="sub">
            <text>{{ boardLabel(item.board_size, item.color_count) }}</text>
            <text class="likes">♥ {{ item.like_count }}</text>
          </view>
        </view>
      </view>
    </view>
    <view v-else class="empty">这个分类还没有作品</view>
</view>
</template>

<style scoped lang="scss">
.page { padding: 16rpx 24rpx 140rpx; }
.search {
  display: flex;
  align-items: center;
  gap: 12rpx;
  background: $card;
  border: 1rpx solid $line;
  border-radius: 999rpx;
  padding: 14rpx 24rpx;
  margin-bottom: 16rpx;
}
.ico { color: $ink-3; }
.input { flex: 1; font-size: 26rpx; }
.chips { white-space: nowrap; margin-bottom: 20rpx; }
.chip {
  display: inline-block;
  padding: 12rpx 26rpx;
  margin-right: 12rpx;
  border-radius: 999rpx;
  background: $card;
  color: $ink-2;
  font-size: 26rpx;
  border: 1rpx solid $line;
}
.chip.on { background: $brand-soft; color: $brand-dark; border-color: transparent; font-weight: 700; }
.grid {
  display: flex;
  flex-direction: row;
  flex-wrap: wrap;
  justify-content: space-between;
}
.card {
  flex: 0 0 340rpx;
  width: 340rpx;
  max-width: 340rpx;
  margin-bottom: 16rpx;
  box-sizing: border-box;
}
.cover { width: 100%; height: 300rpx; background: $brand-lighter; display: block; }
.meta { padding: 12rpx 16rpx 18rpx; }
.name { display: block; font-weight: 700; font-size: 28rpx; }
.author { display: flex; align-items: center; gap: 8rpx; margin-top: 8rpx; color: $ink-3; font-size: 22rpx; }
.avatar {
  width: 32rpx; height: 32rpx; border-radius: 50%;
  background: linear-gradient(135deg, $brand, $warm);
  color: #fff; font-size: 18rpx; text-align: center; line-height: 32rpx;
}
.sub { display: flex; justify-content: space-between; margin-top: 8rpx; color: $ink-3; font-size: 22rpx; }
.likes { color: $brand; }
</style>
