<script setup lang="ts">
import { onLoad } from '@dcloudio/uni-app'
import { ref } from 'vue'
import { plazaApi } from '../../api/http'
import type { CommentItem, WorkCard } from '../../api/types'
import { useCreateStore } from '../../stores/create'
import { useUserStore } from '../../stores/user'
import { boardLabel, ensureLogin, initialOf, showError, showTip, timeAgo } from '../../utils/ui'

const user = useUserStore()
const create = useCreateStore()
const work = ref<WorkCard | null>(null)
const comments = ref<CommentItem[]>([])
const text = ref('')

async function load(id: string) {
  work.value = await plazaApi.detail(id)
  const data = await plazaApi.comments(id)
  comments.value = data.items
}

onLoad(async (query) => {
  if (!query?.id) return
  try {
    await load(String(query.id))
  } catch (e) {
    showError(e, '作品不存在')
  }
})

async function like() {
  if (!work.value || !ensureLogin()) return
  try {
    const res = await plazaApi.like(work.value.id)
    work.value.liked = res.liked
    work.value.like_count = res.like_count
  } catch (e) {
    showError(e)
  }
}

async function favorite() {
  if (!work.value || !ensureLogin()) return
  try {
    const res = await plazaApi.favorite(work.value.id)
    work.value.favorited = res.favorited
    work.value.favorite_count = res.favorite_count
    showTip(res.favorited ? '已收藏' : '已取消收藏')
  } catch (e) {
    showError(e)
  }
}

async function send() {
  if (!work.value || !ensureLogin()) return
  const content = text.value.trim()
  if (!content) return
  try {
    const item = await plazaApi.addComment(work.value.id, content)
    comments.value = [item, ...comments.value]
    work.value.comment_count += 1
    text.value = ''
  } catch (e) {
    showError(e, '评论失败')
  }
}

function follow() {
  showTip(user.isLoggedIn ? '关注即将开放' : '请先登录')
}

function copyMake() {
  if (!work.value) return
  create.referenceTitle = work.value.title
  showTip('已带上作品参考')
  uni.switchTab({ url: '/pages/create/index' })
}
</script>

<template>
  <view v-if="work" class="page">
    <image class="hero" :src="work.preview_url" mode="aspectFit" />
    <view class="author">
      <text class="avatar">{{ initialOf(work.author.nickname) }}</text>
      <view class="who">
        <text class="name">{{ work.author.nickname }}</text>
        <text class="sub">{{ timeAgo(work.created_at) }}</text>
      </view>
      <text class="follow" @click="follow">关注</text>
    </view>
    <text class="title">{{ work.title }}</text>
    <text v-if="work.caption" class="caption">{{ work.caption }}</text>
    <text class="meta">{{ boardLabel(work.board_size, work.color_count) }} · {{ work.peg_count }} 钉</text>
    <view class="engage">
      <text @click="like">{{ work.liked ? '♥' : '♡' }} {{ work.like_count }}</text>
      <text @click="favorite">{{ work.favorited ? '★' : '☆' }} {{ work.favorite_count }}</text>
      <text>💬 {{ work.comment_count }}</text>
      <text @click="showTip('可使用右上角转发')">↗ 分享</text>
    </view>

    <view class="sec-title"><text>评论 {{ work.comment_count }}</text></view>
    <view v-if="comments.length" class="comments">
      <view v-for="item in comments" :key="item.id" class="comment">
        <text class="avatar sm">{{ initialOf(item.author.nickname) }}</text>
        <view>
          <text class="name">{{ item.author.nickname }}</text>
          <text class="txt">{{ item.content }}</text>
          <text class="sub">{{ timeAgo(item.created_at) }}</text>
        </view>
      </view>
    </view>
    <view v-else class="empty">还没有评论，来说第一句</view>
    <view class="composer">
      <input v-model="text" class="input" placeholder="说点什么…" confirm-type="send" @confirm="send" />
      <text class="send" @click="send">发送</text>
    </view>
    <view class="do" @click="copyMake">照着做</view>
  </view>
  <view v-else class="page"><view class="empty">加载作品…</view></view>
</template>

<style scoped lang="scss">
.page { padding: 24rpx 32rpx 160rpx; }
.hero { width: 100%; height: 640rpx; background: $brand-lighter; border-radius: 28rpx; }
.author { display: flex; align-items: center; gap: 16rpx; margin-top: 20rpx; }
.avatar {
  width: 72rpx; height: 72rpx; border-radius: 50%;
  background: linear-gradient(135deg, $brand, $warm); color: #fff;
  text-align: center; line-height: 72rpx; font-weight: 800;
}
.avatar.sm { width: 56rpx; height: 56rpx; line-height: 56rpx; font-size: 24rpx; flex-shrink: 0; }
.who { flex: 1; }
.name { display: block; font-weight: 750; }
.sub { display: block; color: $ink-3; font-size: 22rpx; }
.follow { color: $brand-dark; background: $brand-soft; border-radius: 999rpx; padding: 8rpx 20rpx; font-size: 24rpx; }
.title { display: block; margin-top: 18rpx; font-size: 36rpx; font-weight: 800; }
.caption { display: block; margin-top: 8rpx; color: $ink-2; line-height: 1.5; }
.meta { display: block; margin-top: 8rpx; color: $ink-3; font-size: 24rpx; }
.engage { display: flex; gap: 28rpx; margin-top: 18rpx; color: $ink-2; font-size: 26rpx; }
.comments { display: flex; flex-direction: column; gap: 20rpx; }
.comment { display: flex; gap: 12rpx; }
.txt { display: block; margin-top: 4rpx; color: $ink; }
.composer {
  margin-top: 20rpx; display: flex; gap: 12rpx; align-items: center;
  background: $card; border: 1rpx solid $line; border-radius: 999rpx; padding: 8rpx 8rpx 8rpx 24rpx;
}
.input { flex: 1; font-size: 26rpx; }
.send { color: #fff; background: $brand; border-radius: 999rpx; padding: 12rpx 24rpx; font-size: 24rpx; }
.do {
  position: fixed; left: 32rpx; right: 32rpx; bottom: calc(24rpx + env(safe-area-inset-bottom));
  text-align: center; background: $brand; color: #fff; border-radius: 999rpx;
  padding: 24rpx; font-weight: 800; box-shadow: 0 12rpx 28rpx rgba(255, 107, 91, 0.3);
}
</style>
