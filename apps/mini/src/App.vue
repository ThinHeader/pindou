<script setup lang="ts">
import { onLaunch } from '@dcloudio/uni-app'
import { useCreditsStore } from './stores/credits'
import { useUserStore } from './stores/user'

onLaunch(() => {
  const credits = useCreditsStore()
  const user = useUserStore()
  user.hydrate()
  credits.hydrate()
})
</script>

<style lang="scss">
@import './uni.scss';

page {
  background: $bg;
  color: $ink;
  font-family: $font;
  font-size: 28rpx;
  line-height: 1.45;
}
</style>
