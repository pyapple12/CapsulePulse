# design/ — 玻璃材质 UI 原型（映射期参照物，已冻结）

> **冻结声明（2026-10-09，映射期 PL016–PL021 完结）**：本目录作为映射期参照物就此冻结——映射已全部完成（映射表 12 行销项见下），后续功能与页面调整在 `ui/` 侧进行，**不再回改本目录**；文中"定稿配方"等描述为映射期历史记录（APP 实际观感以 `ui/` 的 CT 化方案为准）。
> 配方来源：`.temp/glass-lab-e.html` 试验场定稿读数（2026-09-16）。
> 本目录是**当前 APP 完整页面与逻辑**按定稿玻璃配方的可交互复刻，验证通过后按映射表落回 `ui/`。

## 定稿配方

纱浓度 15% · 颗粒 10 · 云纹 15 · 光带 60% · 磨砂 5px · 折射强度 25% ·
阴影浓度 100% · 阴影范围 200% · 阴影角度 140° · Bloom 80% · 透镜线 开 · 胶片漏光 关 · 元素色溢 关

## 文件

| 文件                | 职责                                                                 |
| ------------------- | -------------------------------------------------------------------- |
| `index.html`        | 可交互原型：计时/统计双页 + 确认框 + 设置面板 + 状态机（客户端模拟） |
| `glass.css`         | 玻璃引擎：配方令牌 + 五层纱 + 位移折射 + 阴影/辉光/透镜系统          |
| `assets/lab-bg.jpg` | 演示背景（本地照片，JPEG q90；验证用，不随原型进 APP）               |

## 查看方式

```bash
cd .temp && node serve.mjs     # 端口 8471
# 浏览器打开 http://localhost:8471/design/index.html
```

## 映射表（原型 → APP 回归路径）

> PL020.2 清场销项（2026-10-08）：12 行全部映射完成——对照批次见「销项」列；映射终点 = ui/src/styles/*.css 1:1 副本 + 组件重装，探针与截图档案见 `.temp/pl016-mapping-verification.md`、`.temp/pl019-pl020-verification.md`。

| 原型区块                                               | 对应 Vue 组件                                 | 映射落点                                        | 销项                                                                         |
| ------------------------------------------------------ | --------------------------------------------- | ----------------------------------------------- | ---------------------------------------------------------------------------- |
| `.window` 五层纱 + 位移折射 + 阴影系统                 | `App.vue`（.glass-card 根）                   | :root 令牌换血 + backdrop-filter 链 + fx 层     | ✓ PL016（glass.css 1:1；真窗 backdrop 不渲染→自雾化+DWM，glass.css 差异③）   |
| `--blur:5px` 清玻璃                                    | 聚焦态 DWM 磨砂观感基准                       | 真实窗口失焦=纯纱（alpha 路线），聚焦=PL011 DWM | ✓ PL016 令牌 1:1 落位；聚焦磨砂归 PL011 DWM（README 可行性 ❌ 分工不变）     |
| `.timer-top`（`.date-card` 日期卡 + `.dial` 区间时钟） | `TimerCard.vue` + `ProgressRing.vue`          | 日期卡样式微调；区间弧染色搬进表盘              | ✓ PL017（timer.css 1:1；区间弧并入 dial，ProgressRing 删净）                 |
| `.flip-clock` 翻转计时                                 | `TimerCard.vue`                               | 翻页动画与配色按原型                            | ✓ PL017（7 格 flap 插拔 + --flip-dur 0.09s 1:1）                             |
| `.status-row`（沙漏 + START + 电量）                   | `TimerCard.vue`（沙漏为新增视觉件）           | 沙漏周期跟随提醒阈值；START 即主按钮态          | ✓ PL017（hourglass.css 1:1 + 沙漏状态机移植，周期=阈值）                     |
| `.pill-cast` 打卡 pill                                 | `TimerCard.vue`（pill）                       | 配色换玻璃钮配方（同色系描边 + 底缘透光）       | ✓ PL017（controls.css 1:1，打卡确认接线原样）                                |
| `.seg-control` 三段式（工作/休息/重置）                | `TimerCard.vue`                               | 选中=按下玻璃语汇；重置=瞬时按钮                | ✓ PL017（三段 radio 态驱动 + session_restart 接线）                          |
| `.sideButton` 侧角钮（统计/设置）+ tooltip             | `App.vue` / `DockNav.vue`（导航形态落地时定） | 悬停 tooltip + 按压回弹                         | ✓ PL019（DockNav 退役提前至 PL018；侧钮 = App 骨架 + 设计齿轮 SVG）          |
| `#stats-board` 统计板（正面五区 + 翻面今日明细）       | `StatsView.vue` / `StatsCard.vue`             | 卡片配方统一为清玻璃；翻面=今日明细入口         | ✓ PL018（StatsView 飞出板重装；StatsCard 设计无对应位移除；翻面=今日明细）   |
| `.graph` 时间图谱 + 节点彗星                           | `StatsView.vue`                               | 彗星发射流规格见下节                            | ✓ PL018（COMET 18/14、clip 走廊、纪元回拨+帧哨兵整段照搬）                   |
| `.overlay .sheet` 确认框 / `#settings-board` 设置板    | `ConfirmModal.vue` / `SettingsPanel.vue`      | sheet 材质按原型                                | ✓ PL019（结构/文案/几何 1:1；真窗自雾化+黑纱档，设计第五行开关常开禁用登记） |
| `.banner` 文案条                                       | `App.vue`（提醒/自动下班条）                  | 样式微调                                        | ✓ PL017（controls.css .banner 1:1；error 红字为 ui 功能胶水保留）            |

## 不迁清单（PL020.2 核对）

- `assets/css/devkit.css` + `index.html` 演示控制台（`.devkit`）：验证专用，不迁（ui/ 无对应物）
- `index.html` demo JS 状态机（S 状态/applyStateCall/演示数据）：由 ui/ 真实 invoke 接线 + dev/mock-invoke（DEV-only）取代
- `assets/lab-bg.jpg` 等演示背景：真窗口恒透明，不迁
- 磨砂对比滑杆、背景切换等 devkit 联动：不迁

## 真实窗口可行性标注

- ✅ 半透明纱、颗粒/云纹/光带、同色系描边、底缘透光、可调阴影、Bloom、透镜线——内容无关绘制层，失焦/聚焦均有效；
- ⚠️ 磨砂 5px 与位移折射：作用于**页面内内容**（真实窗口中=大板纱层），桌面本体像素只有聚焦态 DWM 磨砂覆盖（PL011 架构）；失焦态原型观感 = 纱 + 折射叠加在透明窗上（近似可达）；
- ❌ 原型中 backdrop-filter 对窗外桌面背景的模糊——真实窗口不可复现（OS 边界），落地时以 PL011 焦点联动承担。

## 节点彗星发射流（规格补账，2026-10-04）

时间图谱的白纹彗星不是 CSS 无限循环动画，而是「逐条发射 + 动画钟追赶」的 JS 发射器（index.html 的 `COMET` 常量 + `spawnStripe`/`launchComet`），映射回 ui/ 时整段照搬：

- **参数**：`COMET = { speed: 18, spacing: 14 }`——speed = 白纹位移速度（px/s）；spacing = 每走 14px 发射一条，同屏条数随「进入节点 → D」的走廊宽度自适应；
- **走廊**：`.graph-comet` 左缘 = 进入节点中心、右缘 = D 节点中心（JS 随渲染定位）；白纹 24px 宽，从走廊左外 -24px 出生（含潜行段），走到右缘外自移除；
- **时钟**：记账用 `document.timeline`（与 CSS 动画同钟），**禁用 performance.now 墙钟**——遮挡/最小化/托盘时动画钟冻结、发射网格随之冻结，恢复后从冻结处无缝续播，账面与屏幕永不分叉；
- **挂起恢复**：rAF 帧哨兵（帧距 > 250ms 判挂起）置重建标记，恢复首帧 `launchComet(true)` 清流重播；覆盖层首次创建时纪元回拨两个寿命，F5 后立刻开板即满员稳态、看不到「从头发射」；
- **相位接入**：补发的历史白纹以负 `animation-delay` 从对应相位进入，与 CSS 动画同一时间线，起始位置天然自洽；
- **发射唤醒**：自校正 `setTimeout`（按动画钟折算下一网格槽位），不用会累积漂移的固定 `setInterval`；
- **reduced-motion**：`launchComet` 直接返回不发射，CSS 同步隐藏条纹。
