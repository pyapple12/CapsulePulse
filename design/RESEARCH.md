# design/RESEARCH.md — 外部参考研究记录

> 用途：记录项目外围调研结论——UI kit、动效、材质工艺、竞品参考等细枝末节，一个文件管所有外部研究。
> 规则：条目递增编号（R001 起），追加式只增不删；失效条目标注「已失效」并写明原因，不删除。
> 分工：`README.md` 记原型 ↔ Vue 组件映射；本文件记外部生态调研，两者不重叠。

---

## R001 uiverse-io/galaxy 调研——UI kit 定位与动效候选（2026-09-16）

**来源**：https://github.com/uiverse-io/galaxy（浅克隆留档 `.temp/galaxy-upstream/`，gitignore 内，全量分析非抽样）
**关联**：PL012 玻璃材质落地的外围参考；后续动效任务的候选池

### 一、定位结论

galaxy 是 Uiverse.io 网站的官方开源档案库：**3802 个社区投稿元素，MIT 许可**（Copyright 2023 Uiverse.io），全部为纯 HTML/CSS（约 6% 为 Tailwind 变体），官方用法即「打开文件、复制粘贴」。三个关键事实：

1. 不是框架级 UI kit——无 npm 包、无设计令牌体系、无 Vue/React 封装；
2. 元素命名随机（`作者_动物名-数字`），仓库内无法按风格检索，只能全量 grep 特征属性；带标签浏览在 uiverse.io 网站上；
3. 元素页 URL 规律：`https://uiverse.io/<作者>/<元素名>`，页面自带 MIT 声明。

**总判定：对 CapsulePulse 不是「引进的 kit」，而是「挖掘的矿」——玻璃工艺以本项目 `glass.css` 定稿配方为唯一标准，galaxy 组件仅作单点技法与动效参考。**

### 二、玻璃组件盘点（全量 grep `backdrop-filter`）

| 指标                        | 数量             | 分布                                                                  |
| --------------------------- | ---------------- | --------------------------------------------------------------------- |
| 真玻璃（backdrop-filter）   | 117 / 3802（3%） | Cards 65、Buttons 约 40、其余散布 Forms / Inputs / loaders / Tooltips |
| 假玻璃（filter: blur 自身） | 296              | 模糊元素自身而非透出背景，无参考价值                                  |
| 复合配方（blur+saturate）   | 5                | 全部为 ArturCodeCraft 按钮系，`saturate(180%) blur(20px)`             |

工艺深度：绝大多数为单层浅配方（rgba 灰底 + blur 2~20px + 1px 描边 + 单层阴影），无高光内描边 / 颗粒 / bloom 等层次工艺。

### 三、动效组件盘点（galaxy 的真正长板）

| 分类            | 带动效（@keyframes） | 占比 |
| --------------- | -------------------- | ---- |
| loaders         | 694 / 718            | 97%  |
| Notifications   | 23 / 23              | 100% |
| Checkboxes      | 46 / 171             | 27%  |
| Cards           | 184 / 726            | 25%  |
| Buttons         | 284 / 1231           | 23%  |
| Toggle-switches | 37 / 260             | 14%  |

「玻璃 × 动效」双修元素全库仅 1 个（`loaders/Subaashbala_ugly-lizard-47`）——两批元素基本不相交；找动效不必限玻璃系，引入后自行换皮即可。

### 四、动效候选清单（已逐一读码核验，按项目缺口映射）

目验入口：uiverse.io 搜索组件名，或点直达链接。

| #   | 组件名                         | 直达链接                                        | 动效表现                                                          | 项目落点                         |
| --- | ------------------------------ | ----------------------------------------------- | ----------------------------------------------------------------- | -------------------------------- |
| 1   | Yaya12085 / smooth-seahorse-63 | https://uiverse.io/Yaya12085/smooth-seahorse-63 | scale(0.98)→1 淡入浮现（0.6s ease）                               | 提醒浮层 / ConfirmModal 入场     |
| 2   | alexruix / gentle-octopus-87   | https://uiverse.io/alexruix/gentle-octopus-87   | slideIn 宽度展开 + 文案渐显                                       | 提醒横条展开                     |
| 3   | ilkhoeri / blue-zebra-2        | https://uiverse.io/ilkhoeri/blue-zebra-2        | 旋钮 overshoot 回弹位移 `cubic-bezier(0.175, 0.885, 0.32, 1.275)` | 设置页开关（声音/通知/自动下班） |
| 4   | Subaashbala / quiet-ape-66     | https://uiverse.io/Subaashbala/quiet-ape-66     | wobble 果冻形变（正反双 keyframes + 统一时长 CSS 变量，工艺讲究） | 开关趣味向备选                   |
| 5   | Galahhad / kind-cheetah-52     | https://uiverse.io/Galahhad/kind-cheetah-52     | 成功对勾 + 回弹曲线 `cubic-bezier(0.68, -0.55, 0.265, 1.55)`      | 打卡成功反馈                     |
| 6   | LilaRest / afraid-fox-80       | https://uiverse.io/LilaRest/afraid-fox-80       | movingBorders 四边流动渐变描边（同作者另有 4 个同款）             | 主按钮轮廓光动效化               |
| 7   | neerajbaniwal / sweet-sloth-99 | https://uiverse.io/neerajbaniwal/sweet-sloth-99 | pang 弹性弹出 scale(0)→1                                          | dock 图标 tooltip                |
| 8   | Nawsome / fluffy-seahorse-28   | https://uiverse.io/Nawsome/fluffy-seahorse-28   | SVG 双环 stroke-dashoffset 反向旋转（3.7K 赞高人气）              | 8h 进度环表盘动效语言            |
| 9   | Nawsome / ancient-yak-42       | https://uiverse.io/Nawsome/ancient-yak-42       | 环 + 箭头组合旋转                                                 | 进度环备选                       |

### 五、引入改造规范（本项目硬约束）

1. **颜色令牌化**：硬编码 hex/rgba 全部换 `glass.css` 令牌，禁写死颜色，光效取主题令牌；
2. **reduced-motion 守卫**：uiverse 元素基本不带 `prefers-reduced-motion` 保护，引入时必须补齐（PL009 纪律）；
3. **类名隔离**：元素类名朴素（`.button` / `.card`），并入 Vue 组件时加前缀防全局污染；
4. **纯 CSS 优先**：约 6% 元素为 Tailwind 变体（241 个），不取；选中元素需确认无 CDN 依赖；
5. **曲线族统一**：优先复用清单里已验证的两条曲线（overshoot `0.175, 0.885, 0.32, 1.275` / back `0.68, -0.55, 0.265, 1.55`），不每处自造。

### 六、后续调研占位

（R002+ 预留：动效曲线体系 / 图标库对比 / 其他 UI kit 等，按需追加。）
