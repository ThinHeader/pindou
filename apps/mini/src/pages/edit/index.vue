<script setup lang="ts">
import { onShow } from '@dcloudio/uni-app'
import { ref } from 'vue'
import { draftsApi, errorOf } from '../../api/http'
import { readableError } from '../../api/payload'
import type { DraftCard } from '../../api/types'
import { useUserStore } from '../../stores/user'
import { boardLabel, ensureLogin, showError, showTip } from '../../utils/ui'
import { canRevise } from '../../utils/pattern'

const user = useUserStore()
const pills = [
  { id: 'mine', label: '我的稿' },
  { id: 'favorite', label: '收藏' },
  { id: 'all', label: '全部' },
]
const tab = ref('mine')
const keyword = ref('')
const items = ref<DraftCard[]>([])
const loading = ref(false)
const failed = ref('')

async function load() {
  if (!user.token) user.hydrate()
  if (!user.isLoggedIn) {
    items.value = []
    failed.value = ''
    return
  }
  loading.value = true
  failed.value = ''
  try {
    const data = await draftsApi.list(tab.value, keyword.value.trim())
    items.value = Array.isArray(data.items) ? data.items : []
    if (!Array.isArray(data.items)) failed.value = '稿件列表格式不对'
  } catch (e) {
    items.value = []
    if (errorOf(e).silent) {
      failed.value = ''
      return
    }
    failed.value = readableError(e, '稿件加载失败')
    showError(e, '稿件加载失败')
  } finally {
    loading.value = false
  }
}

onShow(() => {
  if (!user.token) user.hydrate()
  if (!user.isLoggedIn) {
    items.value = []
    failed.value = ''
    return
  }
  load()
})

function search() {
  if (!ensureLogin()) return
  load()
}

function pick(id: string) {
  if (!ensureLogin()) return
  tab.value = id
  load()
}

function open(item: DraftCard) {
  if (!canRevise(item)) {
    showTip(item.is_official || item.is_template ? '官方稿不能改' : '只能改自己的稿')
    return
  }
  uni.navigateTo({ url: `/pages/edit/board?id=${item.id}` })
}

function goCreate() {
  uni.switchTab({ url: '/pages/create/index' })
}

function goLibrary() {
  uni.switchTab({ url: '/pages/library/index' })
}
</script>

<template>
  <view class="page">
    <text class="lead">从稿件库选一张自己的稿，在现有色号上改。</text>
    <view class="search">
      <text>⌕</text>
      <input
        v-model="keyword"
        class="input"
        placeholder="搜索稿件名"
        confirm-type="search"
        @confirm="search"
      />
    </view>
    <view class="pills">
      <view
        v-for="pill in pills"
        :key="pill.id"
        class="pill"
        :class="{ on: tab === pill.id }"
        @click="pick(pill.id)"
      ><text>{{ pill.label }}</text></view>
    </view>
    <view v-if="!user.isLoggedIn" class="empty">
      <view class="line"><text>登录后才能选择自己的稿来改</text></view>
      <view class="line" @click="ensureLogin()"><text class="go">去登录</text></view>
    </view>
    <view v-else-if="loading" class="empty">
      <view class="line"><text>正在加载稿件…</text></view>
    </view>
    <view v-else-if="failed" class="empty">
      <view class="line"><text>{{ failed }}</text></view>
      <view class="line" @click="load"><text class="go">重试</text></view>
      <view class="line" @click="goLibrary"><text class="go">去稿件库</text></view>
    </view>
    <view v-else-if="items.length" class="grid">
      <view v-for="item in items" :key="item.id + item.preview_url" class="card" @click="open(item)">
        <image class="thumb" :src="item.preview_url" mode="aspectFill" />
        <view class="meta">
          <text class="name">{{ item.title }}</text>
          <text class="tags">{{ boardLabel(item.board_size, item.color_count) }} · {{ item.badge }}</text>
          <text v-if="!canRevise(item)" class="lock">只能看，不能改</text>
        </view>
      </view>
    </view>
    <view v-else class="empty">
      <view class="line"><text>这里还没有可改的稿</text></view>
      <view class="line"><text class="hint">从稿件库选一张自己的稿，或先去出一张新稿。</text></view>
      <view class="line" @click="goLibrary"><text class="go">去稿件库</text></view>
      <view class="line" @click="goCreate"><text class="go">去出稿</text></view>
    </view>
  </view>
</template>

<style scoped lang="scss">
.page { padding: 16rpx 24rpx 140rpx; }
.lead { display: block; color: $ink-2; font-size: 26rpx; line-height: 1.5; margin-bottom: 16rpx; }
.search {
  display: flex; align-items: center; gap: 12rpx;
  background: $card; border: 1rpx solid $line; border-radius: 999rpx;
  padding: 14rpx 24rpx; color: $ink-3;
}
.input { flex: 1; font-size: 26rpx; color: $ink; }
.pills { display: flex; gap: 12rpx; margin: 20rpx 0; flex-wrap: wrap; }
.pill {
  padding: 10rpx 22rpx; border-radius: 999rpx; background: $card;
  border: 1rpx solid $line; color: $ink-2; font-size: 24rpx;
}
.pill text { font-size: 24rpx; color: $ink-2; }
.pill.on { background: $ink; border-color: $ink; }
.pill.on text { color: #fff; }
.grid { @include feed-grid; }
.card {
  @include feed-cell;
  background: $card; border-radius: 24rpx;
  border: 1rpx solid $line; overflow: hidden;
}
.thumb { width: 100%; height: 280rpx; background: $brand-lighter; display: block; }
.meta { padding: 14rpx 16rpx 18rpx; }
.name { display: block; font-weight: 700; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.tags { display: block; margin-top: 6rpx; color: $ink-3; font-size: 22rpx; }
.lock { display: block; margin-top: 6rpx; color: $ink-3; font-size: 20rpx; }
.empty { display: flex; flex-direction: column; align-items: center; gap: 12rpx; }
.line { width: 100%; }
.line text { color: $ink; font-size: 28rpx; line-height: 1.5; }
.line .hint { display: block; color: $ink-2; font-size: 24rpx; line-height: 1.5; }
.line .go { color: $brand; font-weight: 700; }
</style>
