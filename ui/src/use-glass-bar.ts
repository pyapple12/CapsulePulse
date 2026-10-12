/**
 * 玻璃滚动浮钮（PL026.3）：移植 CT `useGlassBar` 1:1（值取 CT 原值——白纱 0.22/0.32、
 * 浮钮 4×16、`right = 4.5`），保留 CT 的按住拖拽。相对 CT 的两处 CP 适配：
 * ① 滚动面按数组传入——统计板是翻转面（前/后各一），两个面都要隐藏原生滑杆并同步滚动，
 *    几何取"活动面"（`active()`），翻面后调用方调 `sync()` 重算；
 * ② 锚层默认 `#win`（底板层）——浮钮落在统计板右缘与窗口右缘之间的缝里，故不能挂板内
 *    （板内坐标不覆盖那道缝）；板关闭（`visibility: hidden`）时浮钮一并隐藏。
 */

/** 浮钮选项（缺省即 CT 原值） */
export interface GlassBarOptions {
  /** 浮钮锚层（默认 `#win` 底板层）。 */
  anchor?: HTMLElement | null;
  /** 浮钮距锚层右缘像素（CT 原值 4.5）。 */
  right?: number;
  /** 轨道按滚动面 padding 上下内缩（面板/列表用 true；textarea 用 false）。 */
  inset?: boolean;
  /** 几何来源（缺省取滚动面数组第一个）。 */
  active?: () => HTMLElement | null;
}

/** 浮钮高（与 `.glass-thumb` 高度同源；轨道行程 = 轨道高 − 此值） */
const THUMB_H = 16;

/**
 * 挂一枚玻璃浮钮。
 * # 参数
 * - scrollers：滚动面（全部挂 `.glass-scroll` + scroll 监听）
 * - opts：见 [`GlassBarOptions`]
 * # 返回
 * - `sync()`：手动重算（翻面 / 开合 / 内容变化后调用）
 * - `setVeiled(v)`：浮层盖住时加/去 `.veiled`（0.22s 淡出）
 * - `destroy()`：解绑全部监听并摘掉浮钮
 */
export function useGlassBar(
  scrollers: HTMLElement[],
  opts: GlassBarOptions = {},
): { sync: () => void; setVeiled: (veiled: boolean) => void; destroy: () => void } {
  const anchor = opts.anchor ?? document.getElementById("win");
  const right = opts.right ?? 4.5;
  const bar = document.createElement("div");
  bar.className = "glass-bar";
  bar.hidden = true;
  const thumb = document.createElement("i");
  thumb.className = "glass-thumb";
  bar.appendChild(thumb);
  anchor?.appendChild(bar);

  /** 当前活动滚动面（翻面切换后由 sync 重新取） */
  const current = (): HTMLElement | null => opts.active?.() ?? scrollers[0] ?? null;

  /** 重算轨道与浮钮位置（板关闭 / 无几何 / 不溢出 → 隐藏） */
  const sync = (): void => {
    const scroller = current();
    if (!anchor || !scroller) {
      bar.hidden = true;
      return;
    }
    const sr = scroller.getBoundingClientRect();
    // 板关闭（visibility: hidden）时布局仍在但不可见——直接隐藏，避免空板上浮一枚钮
    if (!sr.height || getComputedStyle(scroller).visibility === "hidden") {
      bar.hidden = true;
      return;
    }
    const hr = anchor.getBoundingClientRect();
    const cs = getComputedStyle(scroller);
    const padT = opts.inset ? parseFloat(cs.paddingTop) || 0 : 0;
    const padB = opts.inset ? parseFloat(cs.paddingBottom) || 0 : 0;
    bar.style.top = `${sr.top - hr.top + padT}px`;
    bar.style.height = `${sr.height - padT - padB}px`;
    bar.style.right = `${right}px`;
    const scrollable = scroller.scrollHeight > scroller.clientHeight + 1;
    bar.hidden = !scrollable;
    if (!scrollable) {
      return;
    }
    const trackH = Math.max(0, sr.height - padT - padB - THUMB_H);
    const ratio = scroller.scrollTop / (scroller.scrollHeight - scroller.clientHeight || 1);
    thumb.style.top = `${Math.round(ratio * trackH)}px`;
  };

  /** 浮钮拖拽：按住上下平移即滚动（比例换算 1:1 CT） */
  const onThumbDown = (e: PointerEvent): void => {
    const scroller = current();
    if (!scroller) {
      return;
    }
    const trackH = bar.getBoundingClientRect().height - THUMB_H;
    if (trackH <= 0) {
      return;
    }
    const startY = e.clientY;
    const startScroll = scroller.scrollTop;
    const range = scroller.scrollHeight - scroller.clientHeight;
    const move = (ev: PointerEvent): void => {
      scroller.scrollTop = startScroll + ((ev.clientY - startY) / trackH) * range;
    };
    const up = (): void => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    e.preventDefault();
  };
  thumb.addEventListener("pointerdown", onThumbDown);

  for (const s of scrollers) {
    s.classList.add("glass-scroll"); // 隐藏原生滑杆
    s.addEventListener("scroll", sync, { passive: true });
  }
  const resize = new ResizeObserver(sync);
  if (anchor) {
    resize.observe(anchor);
  }
  // 内容重建（明细行变化等）不派发 scroll——观察子节点变化补一次同步（CT 同款）
  const content = new MutationObserver(sync);
  for (const s of scrollers) {
    content.observe(s, { childList: true, subtree: true });
  }
  sync();

  return {
    sync,
    setVeiled: (veiled: boolean): void => {
      bar.classList.toggle("veiled", veiled);
    },
    destroy: (): void => {
      for (const s of scrollers) {
        s.removeEventListener("scroll", sync);
      }
      content.disconnect();
      resize.disconnect();
      thumb.removeEventListener("pointerdown", onThumbDown);
      bar.remove();
    },
  };
}
