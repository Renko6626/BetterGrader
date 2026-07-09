<!-- 判分左侧悬浮面板：本题平均分 + 判分分布直方图。
     抽成独立组件，后续可在 body 里继续拼接别的校准组件。Tab 或点标签收起/展开。 -->
<script setup lang="ts">
interface Dist { bins: number[]; peak: number; perPoint: boolean; max: number; total: number; binCount: number }
const props = defineProps<{
  collapsed: boolean;
  average: number | null;
  dist: Dist;
  rubricHtml: string;   // 已渲染的评分标准 HTML；面板下半部截断预览，全文按 R
  currentBin: number | null;  // 当前份的分落在哪个柱，高亮它
}>();
defineEmits<{ (e: "toggle"): void }>();
// 第 i 柱对应的分数（每分一柱时=i，否则按比例换算回分数）
const binScore = (i: number) =>
  props.dist.perPoint ? i : Math.round(i / Math.max(1, props.dist.binCount - 1) * props.dist.max);
</script>

<template>
  <div class="sidebar" :class="{ collapsed }">
    <div v-if="!collapsed" class="body">
      <div class="avg">
        本题平均 <b>{{ average != null ? average.toFixed(1) : "—" }}</b>
        <span class="slash">/ {{ dist.max }}</span>
        <span class="sub">已评 {{ dist.total }} 份</span>
      </div>
      <div v-if="dist.total" class="dist">
        <div class="dh"><span>判分分布</span><span class="pk">峰 {{ dist.peak }} 人</span></div>
        <div class="bars">
          <div v-for="(c, i) in dist.bins" :key="i" class="bar" :class="{ cur: i === currentBin }"
               :style="{ height: (c / dist.peak * 100) + '%' }"
               :title="`${binScore(i)} 分：${c} 人`"></div>
        </div>
        <div class="axis"><span>0</span><span>满分 {{ dist.max }}</span></div>
      </div>
      <div v-else class="empty">本题还没判分</div>
      <!-- 下半部：评分标准截断预览（全文按 R 呼出浮层） -->
      <div v-if="rubricHtml" class="rubric">
        <div class="rh"><span>评分标准</span><span class="more">R 看全文</span></div>
        <div class="rb-clip" v-html="rubricHtml"></div>
      </div>
    </div>
    <div class="tab" title="Tab 收起/展开" @click="$emit('toggle')">{{ collapsed ? "▸" : "◂" }}</div>
  </div>
</template>

<style scoped>
/* 悬浮图层：覆盖在卷面左侧上方；pointer-events 只在面板/标签上生效，其余区域可穿透操作卷面 */
.sidebar { position: absolute; left: 0; top: 0; bottom: 0; z-index: 5;
  display: flex; align-items: stretch; pointer-events: none;
  font-family: ui-monospace, monospace; color: var(--text-body); }
.body { pointer-events: auto; width: 210px; box-sizing: border-box; height: 100%;
  background: rgba(20, 22, 26, 0.94); border-right: 1px solid var(--border); padding: 12px; overflow: hidden;
  display: flex; flex-direction: column; }
.avg, .dist, .empty { flex: none; }
.tab { pointer-events: auto; align-self: center; background: var(--elevated); border: 1px solid var(--border); border-left: none;
  color: var(--text-dim); cursor: pointer; padding: 10px 4px; border-radius: 0 4px 4px 0; user-select: none; }
.avg { font-size: 13px; margin-bottom: 12px; line-height: 1.5; }
.avg b { color: var(--ok); font-size: 22px; }
.avg .slash { color: var(--text-dim); }
.avg .sub { display: block; color: var(--text-faint); font-size: 12px; margin-top: 2px; }
.dist .dh { font-size: 12px; color: var(--text-dim); margin-bottom: 4px; display: flex; justify-content: space-between; }
.dist .dh .pk { color: var(--text-faint); }
.dist .bars { display: flex; align-items: flex-end; gap: 2px; height: 100px; border-bottom: 1px solid var(--border); }
.dist .bar { flex: 1; background: var(--accent); min-width: 3px; }
/* 高亮当前份所在柱：一眼看到"我刚给的分落在分布哪里" */
.dist .bar.cur { background: var(--ok); box-shadow: 0 0 0 1px var(--ok); }
.dist .axis { display: flex; justify-content: space-between; font-size: 11px; color: var(--text-faint); margin-top: 2px; }
.empty { color: var(--text-faint); font-size: 12px; }
/* 下半部评分标准：填满剩余高度，y 方向硬截断 + 底部渐隐 */
.rubric { flex: 1 1 auto; min-height: 0; display: flex; flex-direction: column;
  margin-top: 12px; padding-top: 8px; border-top: 1px solid var(--border-subtle); }
.rubric .rh { flex: none; display: flex; justify-content: space-between; align-items: baseline;
  font-size: 12px; color: var(--text-dim); margin-bottom: 4px; }
.rubric .rh .more { font-size: 11px; color: var(--accent-soft); }
.rb-clip { flex: 1; min-height: 0; overflow: hidden; position: relative;
  font-size: 12px; line-height: 1.55; color: var(--text-body); }
/* 截断处渐隐，暗示"还有更多，按 R 看全" */
.rb-clip::after { content: ""; position: absolute; left: 0; right: 0; bottom: 0; height: 34px;
  background: linear-gradient(transparent, rgba(20, 22, 26, 0.94)); pointer-events: none; }
.rb-clip :deep(h1), .rb-clip :deep(h2), .rb-clip :deep(h3) { color: var(--text); font-size: 13px; margin: 8px 0 3px; }
.rb-clip :deep(ul), .rb-clip :deep(ol) { padding-left: 18px; margin: 3px 0; }
.rb-clip :deep(li) { margin: 2px 0; }
.rb-clip :deep(p) { margin: 3px 0; }
.rb-clip :deep(strong) { color: var(--ok); }
.rb-clip :deep(code) { background: var(--chip); border-radius: 2px; padding: 0 3px; }
.rb-clip :deep(pre) { background: var(--panel); border: 1px solid var(--border); border-radius: 3px; padding: 6px; overflow: hidden; }
.rb-clip :deep(table) { border-collapse: collapse; }
.rb-clip :deep(th), .rb-clip :deep(td) { border: 1px solid var(--border); padding: 1px 5px; }
</style>
