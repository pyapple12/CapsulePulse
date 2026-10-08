//! 标题粒子化（design/index.html initTitleParticles 1:1 移植，PL021 前补口）：
//! 画布接管标题视觉——文字 alpha 网格采样成粒子，光标进入窗口即排斥、离开后弹簧归位，
//! 静止自动停帧省 GPU；reduced-motion 不初始化，保留静态文字（与 design 同款守卫）。
//! 消费的 --panel-shadow / --input-focus-glow 两令牌 design 侧未定义，恒走 fallback
//! （CT 软晕配方 1.7/1.7/8/0.35 与 --accent 辉光）；保留读取口子与设计同源。
//! 样式依托 topbar.css 的 .title--particles（原文字透明让位）与 .title-canvas（外扩余量）。

/** 粒子状态：home = 字形坐标，ed = 到字形边缘距离（CSS px，辉光负 spread 只采深点） */
interface Particle {
  hx: number;
  hy: number;
  x: number;
  y: number;
  vx: number;
  vy: number;
  ed: number;
}

/** 初始化标题粒子动画；reduced-motion 或标题件缺失时返回 undefined（静态文字兜底）。
 * 返回清理函数（解绑 window 监听 + 停帧），App 卸载时调用 */
export function initTitleParticles(win: HTMLElement): (() => void) | undefined {
  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
  const h1 = win.querySelector<HTMLHeadingElement>(".title");
  const canvas = h1?.querySelector<HTMLCanvasElement>(".title-canvas");
  if (!h1 || !canvas || reducedMotion.matches) {
    return undefined;
  }
  const ctx = canvas.getContext("2d");
  if (ctx == null) {
    return undefined;
  }
  h1.classList.add("title--particles"); // 仅粒子形态：原文字透明让位

  const TEXT = "CapsulePulse";
  const GAP = 2; // 采样步长（离屏像素）
  const REPEL_RADIUS = 55;
  const REPEL_FORCE = 0.45;
  const SPRING = 0.06;
  const FRICTION = 0.85;
  const MAX_OFFSET = 15;
  const SETTLE_DIST = 0.5;
  const FRAME_CAP = 240;
  const MARGIN_X = 30;
  const MARGIN_TOP = 20; // 与 .title-canvas top:-20px 联动
  const MARGIN_BOTTOM = 30;
  const GLOWS = [
    { x: 0, y: 10, blur: 15, spread: 3 },
    { x: 0, y: 4, blur: 6, spread: 4 },
  ];
  const SHADOW_THICK = 1.5;
  const sCan = document.createElement("canvas"); // 影层离屏
  const sctx = sCan.getContext("2d");
  if (sctx == null) {
    return undefined;
  }
  let particles: Particle[] = [];
  let dpr = 1;
  let raf = 0;
  let rect: DOMRect | null = null;
  const mouse = { x: -9999, y: -9999 };
  let frames = 0;
  let zone: { left: number; right: number; top: number; bottom: number } | null = null;
  let shadow = { x: 1.7, y: 1.7, blur: 8, alpha: 0.35 };
  let glowColor = "rgba(74, 168, 232, 0.35)";
  const accent = () =>
    (getComputedStyle(document.documentElement).getPropertyValue("--accent") || "#4aa8e8").trim();

  /** 离屏 2x 采样：文字 alpha 网格扫描 → 粒子（home = 字形坐标） */
  function build(): void {
    frames = 0;
    rect = h1!.getBoundingClientRect();
    const cr = win.getBoundingClientRect();
    zone = { left: cr.left, right: cr.right, top: cr.top, bottom: cr.bottom };
    dpr = Math.min(window.devicePixelRatio || 1, 2);
    const cw = rect.width + MARGIN_X * 2;
    const ch = rect.height + MARGIN_TOP + MARGIN_BOTTOM;
    canvas!.width = Math.round(cw * dpr);
    canvas!.height = Math.round(ch * dpr);
    canvas!.style.width = `${cw}px`;
    canvas!.style.height = `${ch}px`;
    sCan.width = canvas!.width;
    sCan.height = canvas!.height;
    const cs = getComputedStyle(document.documentElement);
    const raw = cs.getPropertyValue("--panel-shadow");
    const dims = raw.match(/([\d.]+)px\s+([\d.]+)px\s+([\d.]+)px/);
    const a = raw.match(/,\s*([\d.]+)\)/);
    shadow = dims
      ? {
          x: Number(dims[1]),
          y: Number(dims[2]),
          blur: Number(dims[3]),
          alpha: a ? Number(a[1]) : 0.35,
        }
      : { x: 1.7, y: 1.7, blur: 8, alpha: 0.35 };
    glowColor = cs.getPropertyValue("--input-focus-glow").trim() || glowColor;
    const style = getComputedStyle(h1!);
    const cssFont = parseFloat(style.fontSize) || 15;
    const cssWeight = style.fontWeight || "700";
    const sampleFont = cssFont * 2; // 2 倍采样：渲染减半呈现，密度翻倍
    const font = `${cssWeight} ${sampleFont}px "SF Pro Display", "Segoe UI Variable Display", "Segoe UI", sans-serif`;
    const off = document.createElement("canvas");
    const octx = off.getContext("2d", { willReadFrequently: true });
    if (octx == null) {
      return;
    }
    octx.font = font;
    const tw = octx.measureText(TEXT).width;
    off.width = Math.ceil(tw + 8);
    off.height = Math.ceil(sampleFont * 1.5);
    octx.font = font; // 重设尺寸会重置上下文，需再赋一次
    octx.textBaseline = "middle";
    octx.fillStyle = "#fff";
    octx.fillText(TEXT, 4, off.height / 2);
    const data = octx.getImageData(0, 0, off.width, off.height).data;
    const scale = rect.width / tw; // 粒子字形精确铺满标题宽
    const midY = MARGIN_TOP + rect.height / 2;
    const dots = new Set<string>();
    for (let y = 0; y < off.height; y += GAP)
      for (let x = 0; x < off.width; x += GAP)
        if (data[(y * off.width + x) * 4 + 3] > 128) dots.add(`${x},${y}`);
    particles = [];
    for (let y = 0; y < off.height; y += GAP) {
      for (let x = 0; x < off.width; x += GAP) {
        if (!dots.has(`${x},${y}`)) continue;
        // 到字形边缘距离（CSS px）：辉光负 spread 只采深点
        let ed = 5 * GAP * scale;
        outer: for (let ring = 1; ring <= 5; ring++) {
          for (let ny = y - ring * GAP; ny <= y + ring * GAP; ny += GAP) {
            for (let nx = x - ring * GAP; nx <= x + ring * GAP; nx += GAP) {
              const onRing = Math.abs(nx - x) === ring * GAP || Math.abs(ny - y) === ring * GAP;
              if (onRing && !dots.has(`${nx},${ny}`)) {
                ed = ring * GAP * scale;
                break outer;
              }
            }
          }
        }
        const hx = MARGIN_X + (x - 4) * scale;
        const hy = midY + (y - off.height / 2) * scale;
        // 开场：随机撒在标题周边，靠弹簧汇聚成字
        particles.push({
          hx,
          hy,
          x: hx + (Math.random() - 0.5) * 80,
          y: hy + (Math.random() - 0.5) * 40,
          vx: 0,
          vy: 0,
          ed,
        });
      }
    }
  }

  function frame(): void {
    frames++;
    ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx!.clearRect(0, 0, canvas!.width, canvas!.height); // 真清屏：玻璃上不留拖尾
    let maxHome = 0;
    for (const p of particles) {
      const dx = p.x - mouse.x;
      const dy = p.y - mouse.y;
      const dist = Math.hypot(dx, dy);
      if (dist < REPEL_RADIUS && dist > 0.01) {
        const f = ((REPEL_RADIUS - dist) / REPEL_RADIUS) * REPEL_FORCE;
        p.vx += (dx / dist) * f;
        p.vy += (dy / dist) * f;
      }
      p.vx = (p.vx + (p.hx - p.x) * SPRING) * FRICTION;
      p.vy = (p.vy + (p.hy - p.y) * SPRING) * FRICTION;
      p.x += p.vx;
      p.y += p.vy;
      const ddx = p.x - p.hx;
      const ddy = p.y - p.hy;
      const homeDist = Math.hypot(ddx, ddy);
      if (frames > 120 && homeDist > MAX_OFFSET) {
        p.x = p.hx + (ddx / homeDist) * MAX_OFFSET;
        p.y = p.hy + (ddy / homeDist) * MAX_OFFSET;
      }
      if (homeDist > maxHome) maxHome = homeDist;
    }
    paint();
    const far = mouse.x < -999;
    if (far && (maxHome < SETTLE_DIST || frames > FRAME_CAP)) {
      for (const p of particles) {
        p.x = p.hx;
        p.y = p.hy;
        p.vx = 0;
        p.vy = 0;
      }
      ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx!.clearRect(0, 0, canvas!.width, canvas!.height);
      paint();
      raf = 0;
      return;
    }
    raf = requestAnimationFrame(frame);
  }

  /** 影层统一管线：同坐标暗点落进离屏 → 整层一次模糊 → 叠到主画布 */
  function paintLayer(
    color: string,
    ox: number,
    oy: number,
    thick: number,
    blur: number,
    alpha: number,
    spread: number,
  ): void {
    sctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
    sctx!.clearRect(0, 0, canvas!.width, canvas!.height);
    sctx!.fillStyle = color;
    for (const p of particles) {
      if (p.ed < spread) continue;
      const r = 0.5 + Math.min(1, Math.hypot(p.vx, p.vy) / 3) * 0.4 + thick;
      sctx!.beginPath();
      sctx!.arc(p.x + ox, p.y + oy, r, 0, 6.2832);
      sctx!.fill();
    }
    ctx!.save();
    ctx!.setTransform(1, 0, 0, 1, 0, 0);
    ctx!.filter = `blur(${blur * dpr}px)`;
    ctx!.globalAlpha = alpha;
    ctx!.drawImage(sCan, 0, 0);
    ctx!.restore();
  }

  /** 影层 + 粒子本体一次画齐：辉光两层 → 玻璃落影 → 粒子 */
  function paint(): void {
    for (const g of GLOWS) paintLayer(glowColor, g.x, g.y, 0, g.blur, 1, g.spread);
    paintLayer("#000", shadow.x, shadow.y, SHADOW_THICK, shadow.blur, shadow.alpha, 0);
    ctx!.fillStyle = accent();
    for (const p of particles) {
      const disp = Math.min(1, Math.hypot(p.vx, p.vy) / 3);
      ctx!.globalAlpha = 1;
      ctx!.beginPath();
      ctx!.arc(p.x, p.y, 0.5 + disp * 0.4, 0, 6.2832); // 粒子基础半径 0.5
      ctx!.fill();
    }
  }

  function wake(): void {
    if (!raf) raf = requestAnimationFrame(frame);
  }

  /** 交互监听在 window：光标落在窗口矩形内即唤醒并施加排斥，越界视作远离 */
  const track = (clientX: number, clientY: number): void => {
    if (!rect || !zone) return;
    const cx = clientX - rect.left + MARGIN_X;
    const cy = clientY - rect.top + MARGIN_TOP;
    const near =
      clientX >= zone.left &&
      clientX <= zone.right &&
      clientY >= zone.top &&
      clientY <= zone.bottom;
    if (!near) {
      mouse.x = -9999;
      mouse.y = -9999;
      frames = 0; // 出区弹回按新周期计帧
      wake();
      return;
    }
    mouse.x = cx;
    mouse.y = cy;
    wake();
  };
  const onMove = (e: MouseEvent): void => track(e.clientX, e.clientY);
  const onResize = (): void => {
    build();
    wake();
  };
  window.addEventListener("mousemove", onMove, { passive: true });
  window.addEventListener("resize", onResize);

  build();
  wake(); // 开场汇聚动画

  return () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("resize", onResize);
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    h1.classList.remove("title--particles");
  };
}
