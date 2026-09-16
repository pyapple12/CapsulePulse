# design/ — 玻璃材质 UI 原型（PL012 实现依据）

> 配方来源：`.temp/glass-lab-e.html` 试验场定稿读数（2026-09-16）。
> 本目录是**当前 APP 完整页面与逻辑**按定稿玻璃配方的可交互复刻，验证通过后按映射表落回 `ui/`。

## 定稿配方

纱浓度 15% · 颗粒 10 · 云纹 15 · 光带 60% · 磨砂 5px · 折射强度 25% ·
阴影浓度 100% · 阴影范围 200% · 阴影角度 140° · Bloom 80% · 透镜线 开 · 胶片漏光 关 · 元素色溢 关

## 文件

| 文件 | 职责 |
| --- | --- |
| `index.html` | 可交互原型：计时/统计双页 + 确认框 + 设置面板 + 状态机（客户端模拟） |
| `glass.css` | 玻璃引擎：配方令牌 + 五层纱 + 位移折射 + 阴影/辉光/透镜系统 |
| `assets/lab-bg.jpg` | 演示背景（本地照片，JPEG q90；验证用，不随 PL012 进 APP） |

## 查看方式

```bash
cd .temp && node serve.mjs     # 端口 8471
# 浏览器打开 http://localhost:8471/design/index.html
```

## 映射表（原型 → APP 回归路径）

| 原型区块 | 对应 Vue 组件 | PL012 落点 |
| --- | --- | --- |
| `.window` 五层纱 + 位移 + 阴影系统 | `App.vue`（.glass-card 根） | :root 令牌换血 + backdrop-filter 链 + fx 层 |
| `--blur:5px` 清玻璃 | 聚焦态 DWM 磨砂观感基准 | 真实窗口失焦=纯纱（alpha 路线），聚焦=PL011 DWM |
| `.ringwrap` 8h 表盘 | `ProgressRing.vue` + `TimerCard.vue` | 描边/数字样式按原型微调 |
| `.pill-cast` 打卡 pill | `TimerCard.vue`（pill） | 配色换玻璃钮配方（同色系描边 + 底缘透光） |
| `.gbtn` 开始/暂停 | `TimerCard.vue`（主按钮） | 同上 |
| `.dock` 页签 | `DockNav.vue` | 选中态加 Bloom 辉光 |
| `.gcard` 五卡 / `.stats` | `StatsView.vue` / `StatsCard.vue` | 卡片配方统一为清玻璃 |
| `.overlay .sheet` 确认框/设置 | `ConfirmModal.vue` / `SettingsPanel.vue` | sheet 材质按原型 |
| `.banner` 文案条 | `App.vue`（提醒/自动下班条） | 样式微调 |

## 真实窗口可行性标注

- ✅ 半透明纱、颗粒/云纹/光带、同色系描边、底缘透光、可调阴影、Bloom、透镜线——内容无关绘制层，失焦/聚焦均有效；
- ⚠️ 磨砂 5px 与位移折射：作用于**页面内内容**（真实窗口中=大板纱层），桌面本体像素只有聚焦态 DWM 磨砂覆盖（PL011 架构）；失焦态原型观感 = 纱 + 折射叠加在透明窗上（近似可达）；
- ❌ 原型中 backdrop-filter 对窗外桌面背景的模糊——真实窗口不可复现（OS 边界），落地时以 PL011 焦点联动承担。
