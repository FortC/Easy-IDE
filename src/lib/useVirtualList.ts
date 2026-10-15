import { computed, onBeforeUnmount, onMounted, ref, type Ref } from "vue";

/**
 * 轻量虚拟列表（自研，不引第三方 —— 见 AGENTS.md 禁重依赖）。
 * 仅适用于「等高行」场景（控制台输出、行号列表等）。
 *
 * 行高通过 probe 元素实测获取，因此兼容运行时字体缩放（--font-scale），
 * 不必把行高写死成常量。
 */
export function useVirtualList(
  count: Ref<number>,
  probe: Ref<HTMLElement | undefined>,
  opts: { overscan?: number; gap?: number } = {},
) {
  const overscan = opts.overscan ?? 8;
  const gap = opts.gap ?? 0;

  const viewport = ref<HTMLElement | null>(null);
  const scrollTop = ref(0);
  const viewH = ref(0);
  const rowH = ref(18);

  let rowObserver: ResizeObserver | null = null;
  let viewObserver: ResizeObserver | null = null;

  onMounted(() => {
    // 行高实测：字号随 --font-scale 变化，写死常量会在缩放后错位
    if (probe.value) {
      rowObserver = new ResizeObserver(() => {
        const h = probe.value?.getBoundingClientRect().height ?? 0;
        if (h > 0) rowH.value = h;
      });
      rowObserver.observe(probe.value);
    }
    if (viewport.value) {
      viewObserver = new ResizeObserver(() => {
        viewH.value = viewport.value?.clientHeight ?? 0;
      });
      viewObserver.observe(viewport.value);
      viewH.value = viewport.value.clientHeight;
    }
  });

  onBeforeUnmount(() => {
    rowObserver?.disconnect();
    viewObserver?.disconnect();
  });

  function onScroll() {
    scrollTop.value = viewport.value?.scrollTop ?? 0;
  }

  const stride = computed(() => rowH.value + gap);
  const totalH = computed(() => count.value * stride.value);
  const start = computed(() =>
    Math.max(0, Math.floor(scrollTop.value / stride.value) - overscan),
  );
  const end = computed(() =>
    Math.min(count.value, Math.ceil((scrollTop.value + viewH.value) / stride.value) + overscan),
  );
  /** 渲染区上方的占位高度 */
  const offsetY = computed(() => start.value * stride.value);
  const visibleCount = computed(() => Math.max(0, end.value - start.value));

  return { viewport, onScroll, start, end, offsetY, totalH, visibleCount, rowH };
}
