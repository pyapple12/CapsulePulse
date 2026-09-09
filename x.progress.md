# 进度追踪（x.progress.md）

> 文件职责：任务清单与进度追踪，与 `z.plan.md`（方案与审计归档）配套：方案在 z.plan 展开，执行拆条在本文件勾选。结构：**`## 已完成 ✅` 区在前、`## 未完成` 区在后**，任务完成后整组移动位置。
> 格式速查：任务组 `### PL{NNN}: {标题} [来源引用]`（来源引用：[plan#Phase N] 计划书 / [problems#N] 问题备忘录 / [audit#A{NNN}] 审计报告），组内用 `#### {小节}` 分层；子任务 `- [ ] PL{NNN}.{序号} {标题} —— {做法}；验证：{检验方式}`；审计修复任务组 FIX{NNN} 由 audit-report 归档环节生成，条目格式 `- [ ] FIX{NNN}.{序号} [P{级别}] {标题} —— {做法}；验证：{检验方式}`，编号规则见 `.agents/skills/audit-report`。
> 勾选注记：条目完成时改 `[x]`，并在句尾追加 `（YYYY-MM-DD 已验证：{一句话结论}）`——结论如实，不达标不降级宣布。

## 已完成 ✅

### PL003: 提醒调度与设置持久化 [plan#Phase 3]

> 结果：阈值触达 → 系统通知 + 提示音 + 卡片文案条三通道齐发（5 分钟重发、暂停/重开即重置），⚙ 面板设置持久化即时生效，降级三态互不依赖人工验证全过。素材定案变更：提示音 = 用户自备 mp3 经 import 打包。全组 12 条勾结。详见 z.plan.md 附录 PL003。

#### 阶段 A：设置持久化 + 提示音素材（TDD）

- [x] PL003.1 依赖与素材探针 —— cargo add tauri-plugin-notification；.temp/gen-chime.mjs 生成约 0.5s 双音正弦 WAV → ui/public/chime.wav；验证：文件生成且 WAV 头校验过 + cargo test 编译绿（2026-09-09 已验证：tauri-plugin-notification 2.4.0/serde_json 入账；WAV 头 RIFF/PCM/44.1kHz/16bit 校验过；**素材后经用户拍板改为自备 mp3**——见 PL003.12 注记，chime.wav 与生成脚本随 src 变更退役）
- [x] PL003.2 settings.rs TDD —— `core/src/settings.rs`：ReminderSettings（threshold_min=50/sound_enabled=true/notify_enabled=true，serde default）+ load/save（JSON）+ SettingsError（Json/Io）；用例：不存在→默认、往返一致、损坏 JSON 严格报错、非法阈值拒绝；验证：N2 先 FAIL 后 PASS（临时目录，禁真实用户数据）（2026-09-09 已验证：红灯 E0422 → 4 用例绿——缺省回默认/往返一致/损坏严格/0 与 241 越界均拒；另加 HomeDirUnavailable 变体与 default_path）
- [x] PL003.3 容错白名单登记（设置缺省）—— AGENTS 登记"设置文件不存在回退默认值"（NotFound 语义，场景/降级/理由三要素）；验证：文档三要素核对（2026-09-09 已验证：白名单三条之一，JSON 损坏等其余错误仍严格报错的边界已写明）

#### 阶段 B：提醒评估器（TDD）

- [x] PL003.4 reminder.rs 纯逻辑 —— ReminderConfig/ReminderFire/evaluate（阈值触发 + 5min 重发，无减法比较规避段回退下溢）+ clear（暂停/开始/重开时调用）；验证：N1 用例先 FAIL 后 PASS（未达不触发/恰达触发/触发后 4min 不发 5min 发/暂停重置后新段再触发）（2026-09-09 已验证：红灯 E0425 → 4 用例绿；跨段残留不下溢另有专用例）

#### 阶段 C：接线

- [x] PL003.5 WorkSession::segment_secs —— 当前段时长访问器（Running = now − start，否则零）+ 用例；验证：cargo test 全绿（2026-09-09 已验证：Idle 零/Running 实时/Paused 零/resume 后新段独立计量（total=15 段=5））
- [x] PL003.6 AppContext 扩展与命令 —— + settings/fire 双 Mutex；get_settings/set_settings 命令（set 持久化 + 更新内存态）；验证：T4a 临时路径往返用例（2026-09-09 已验证：AppContext 增 settings/fire/settings_path 三字段；set 校验+持久化+内存态更新、非法阈值 0 被拒）
- [x] PL003.7 status 集成提醒评估 —— session_status 顺路 evaluate；触发 → emit reminder-due + 系统通知（notify_enabled 门控）；capabilities + notification:default；lib.rs 注册插件 + 启动加载设置（NotFound→默认，其他错退出）；验证：构建绿（评估副作用 live 并入 U1）（2026-09-09 已验证：**评估与副作用分离**——status_snapshot 返回 ReminderDecision（无 AppHandle 可测），deliver_reminder 薄层执行 emit+通知；session_status 增 AppHandle 参数（tauri command 原生支持）；status 命令同时接线动作 clear()（暂停/开始/重开即重置））
- [x] PL003.8 容错白名单登记（降级互不依赖）—— 通知发送失败降级仅声音（错误落日志，事件照发）——计划书 §2.3 定案、AGENTS 预告项正式落地；验证：文档三要素核对（2026-09-09 已验证：白名单 +2——通知失败仅声音、提示音播放失败仅通知（前端 catch+console.warn））
- [x] PL003.9 前端监听与提示音 —— listen("reminder-due")（.catch 可见失败）→ &lt;audio&gt; 播提示音（sound_enabled 门控）+ 卡片提醒文案条；验证：npm build 绿（live 并入 U1）（2026-09-09 已验证：listen 注册失败 .catch 可见；**初版 chime.wav 因 vite publicDir 陷阱 404 无声**——降级路径被意外实测有效；修复 = mp3 import 打包，见 PL003.12）
- [x] PL003.10 ⚙ 设置面板 —— 展开式：阈值输入（1–240 分钟）+ 声音/通知开关 → set_settings 即时生效；提醒文案条（"已连续工作 N 分钟，休息一下吧"）；验证：vue-tsc 过 + live 改阈值短段触发（2026-09-09 已验证：面板本地草稿保存才上抛；vue-tsc/npm build 绿；live 改阈值 1 分钟短段触发成功）
- [x] PL003.11 降级三态验证 —— 默认双开 / 关声音仅通知 / 关通知仅声音（都关仅文案）；验证：R1 人工三态（2026-09-09 已验证：R1 三态一次全过）
- [x] PL003.12 PL003 收口 —— 门禁全绿；N1/N2/T4a/U1/R1 结论回写 z.plan 附录 PL003；README/AGENTS 状态行；勾结；验证：门禁全绿 + 文档一致性核对（2026-09-09 已验证：收口轮门禁全绿（fmt --check/clippy -D warnings/test 35/doc 0 告警/npm build/prettier）；**U1 修正轮一次通过**——轮 1 揪出两真问题（提示音 publicDir 404 无声 → mp3 import 打包修复；toast 署名 PowerShell → 登记 [problems#2] 随打包解决），轮 2 全过；素材定案变更 = 用户自备 mp3（assets/house_alarm-clock_loud.mp3，文件名不改）；反向验收记录 `.temp/pl003-verification.md`）

## 未完成

（暂无——下一个大件：Phase 2 托盘常驻与全局快捷键（PL004 候选）/ Phase 5 打包分发，未立项）
