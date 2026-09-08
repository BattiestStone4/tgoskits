---
sidebar_position: 2
sidebar_label: "各队决赛评审"
title: "Proj4 各队决赛技术评审"
---

# Proj4 各队决赛技术评审

> 评审日期：2026-09-03（2026-09-08 按 git 取证修正个别 PR 编号引用）
> 评审对象：《Proj4 每周同步共享文档》附件中的决赛报告（同赛题"面向边缘智能的 AIOS 设计与优化"，任务均为 RK3588 上网球抓取机器人 + StarryOS 系统优化）。报告原件位于 `~/Downloads/附件下载_Proj4 每周同步共享文档/`（含 zip 备份，不入库）。
> 对照基线：`rcore-os/tgoskits` upstream `dev`（本地镜像 @ `3049b0c7be`，2026-08-25）及集成树 `next`（@ `02d7b870f0`，2026-09-03）
> 方法：5 份 PDF 逐页精读（按页拆文本、共约 480K 字符）+ 对关键重合点在 `next`/`dev` 现树做代码取证；文档内证据分级：【代码核验】= 已在本仓库现树核对 / 【报告自述】= 仅据报告文本，待 diff / 【存疑】= 报告未说明或无法判定
> 接手入口：候选的合入队列与建议推进顺序见 [Proj4 工作整理与接手指南](./integration-guide.md) 第 2 章。

## 0. 结论先行（摘要）

1. **7 队同做一题，重复造轮子严重**。除对照的清华 Posad（~90 PR 已合入 dev）外，其余各队都独立重造了 rknpu/UVC-EHCI/TTY/sched 等轮子的不同变体——根源是各队 fork 起点都是较旧的 dev 快照（甚至 `tty_serial-ajax` 旧线、`atomgit aios-porting` 异化线），互相看不见也看不见上游已有能力。**合入前必须以"三点 diff"核净变更**，大量"自研"在现树里已存在（§6）。
2. **真正值得优先跟进的净新增只有约 8~10 项**（§3 P0 表），集中在四类：① 通用内核正确性修复（dma-buf 缓存维护、EHCI 帧回绕/错误传播、同页双缺页竞态、rsext4 写回策略）；② 调度器机制增量（优先级分层 RR + 优先级感知唤醒抢占）；③ rknpu 执行路径增量（三核并发状态机、ABI 错误严格化）；④ 机器人侧通用组件与测试方法论（JPEG 流式组装、四阶段验证阶梯、observe/drive 基准体系）。
3. **可核验性两极分化**：大象队（commit hash 锚点齐全、提交二分、归属诚实）、重返未来队（分层验证 + 提交二分）的产物适合直接提取；YatSenOS/火锅队全报告**无任何自身 commit/fork 链接**，合入成本主要花在"先找到净变更"。
4. **几乎所有候选都缺确定性回归测例**——不符合本仓库"先回归后修复"铁律，接手时须补测。
5. 覆盖缺口：仪星队、球球别跑队未在附件提供决赛文档（飞书链接本地不可达），本次无法评审。

---

## 1. 参赛队伍与材料一览

| 队伍 | 学校 | 硬件 | 基线 | 任务形态 | 材料（页数） | 闭环达成 |
|---|---|---|---|---|---|---|
| **Posad**（对照） | 清华 | OrangePi 5 Plus/RK3588 | oscomp-posad fork，上游 dev @73409e079 | 优化 + 上游化（~90 PR 已合 dev） | Posad技术报告（决赛）.pdf，83 页 | ✅ 六状态拾球闭环 + 里程计回桶 |
| **YatSenOS** | 中山大学 | OrangePi 5 Plus/RK3588 | StarryOS fork（**未给链接/版本**） | 从"能跑 demo"到生产链优化 | YatSenOS决赛文档.pdf，57 页 | ✅ 追球→抓球→放桶（8/15-16） |
| **火锅宇泡面** | 天津大学 | OrangePi 5 Plus/RK3588 | StarryOS/tgoskits（**未给 fork 版本**） | 用户态四层重构 + 系统层优化 | 决赛文档.pdf，51 页 | ✅ 搜索→…→投放全流程，drive 成功率 94–98% |
| **大象飞出冰箱外** | 南开大学 | OrangePi 5/RK3588S（非 5 Plus） | `/home/wdzw/aios`（源自 atomgit aios-porting 逆向线，rdrive 0.18 基线） | 系统级适配 + 追球机器人 | 大象飞出冰箱外-决赛报告.pdf，121 页 | ✅ 完整抓放闭环（control=uart） |
| **重返未来** | 吉林大学 | OrangePi 5 Plus/RK3588 | pengzechen/tgoskits `tty_serial-ajax`（**旧布局快照**）+ pengzechen/aka-rk3588 | 开源网球机器人内核迁移 | 重返未来队技术文档 (决赛).pdf，30 页 | ✅ 全链路跑通、30 min 无 panic（识别正确性仅文字声明） |
| 仪星队 | — | — | — | — | 仅飞书链接，不可达 | 未评审 |
| 球球别跑队 | 南开大学 | — | — | — | 仅飞书链接，不可达 | 未评审 |

各队关键指标对比（口径不一，仅作量级参考）：

| 指标 | YatSenOS | 火锅宇泡面 | 大象 | 重返未来 | Posad（对照） |
|---|---|---|---|---|---|
| rknn_run | 34.5 ms 均值（200 帧，6.4×） | p50 ≈ 177 ms（Linux ≈140–150） | 定频 200 MHz 下未给单值 | 872 ms/帧 → 记录最高 ~10.7 FPS（口径乱） | 5.68 ms（模型协同优化后） |
| UVC/JPEG 链路 | 完整 JPEG 24.9 FPS（EHCI 修复后） | — | 数千帧 errors=0，29.7–30.0 FPS | UVC 640×480@30 建立 | — |
| 整环/端到端 | 端到端 1147→（优化后未单列） | total p50 ≈ 398 ms（Linux ≈312，仅图存疑） | 控制循环 2.4–3.1 FPS（295 ms 相位版） | 首轮 5–6.5 s/帧 → 终值未给 | 端到端 p50 11.75 ms（帧→指令） |
| 启动 | 26.9→6.8 s | — | ~26 s（后未优化） | ~49→30–33 s | TTFI 9.83 s（Linux 11.28） |
| 其他 | 输入链 16.9→8.9 ms | drive 成功率 94–98% | CMD18 2048→16 次/帧；三核墙钟≈单核 | SD 4KB 185–213→13–24 ms；close 数秒→ms | cpu8 sysbench 0.96× Linux |

---

## 2. 候选总表（先看这张）

优先级定义：**P0** = 净新增且通用价值高、改动内聚，建议优先评估入 dev；**P1** = 有价值但与现树重合度未知或证据不足，须先三点 diff / 联系作者拿净变更；**P2** = 不直接合代码，吸收设计/方法或作为框架内重做。**已在上游** = 现树已含同等或更全实现，不重复提交（§6 给证据）。

| # | 来源队 | 候选 | 类别 | 优先级 |
|---|---|---|---|---|
| C01 | YatSenOS | `/dev/dma_heap/system-cached` + dma-buf 方向化 cache 维护 | 内核新特性 | **P0** |
| C02 | 火锅宇泡面 | 优先级分层 RR（多级队列）+ 优先级感知唤醒抢占 | 调度器机制增量 | **P0** |
| C03 | 大象 | rknpu 三核并发提交（核状态机 + 5 槽双索引 + 核租约） | rknpu 驱动增量 | **P0**（先 diff） |
| C04 | 大象 | rknpu ABI 错误严格化 + MemSync 按 obj_addr 反查 + destroy 真释放 | rknpu 驱动 bug 修复 | **P0**（先 diff） |
| C05 | 大象 | 同页双缺页竞态：PTE 查证 + 单页 flush_tlb 重试 | 内核 bug 修复 | **P0**（先 diff） |
| C06 | 重返未来 | rsext4 隐式 sync_to_disk 显式化（close 数秒→毫秒的主因之一） | 文件系统策略 | P0/P1 边界（缺掉电一致性验证，先评审策略） |
| C07 | YatSenOS | EHCI periodic absolute-frame + ISO 错误 packet 粒度传播 | 内核 bug 修复 | P1（与 crab-usb diff） |
| C08 | 大象 | USB2 PHY（inno-usb2）复位序列：退 IDDQ 先于复位、补模拟 PHY 复位 | 内核 bug 修复 | P1（剥离 + 对照上游） |
| C09 | YatSenOS | JPEG 跨 callback 流式组装器（SOI/EOI 状态机） | 用户态通用组件 | P1（并入网球应用生态） |
| C10 | 大象 | aka_simulate / aka_driver_test / 四级验证阶梯 | 工具/方法论 | P1（最有可移植性的部分） |
| C11 | 火锅宇泡面 | observe/drive + CSV 阶段计时 + 版本消融的整环基准体系 | 测试方法论 | P1（作测试样板） |
| C12 | 重返未来 | SD 识别时钟规范（~100 kHz）+ SCR FIFO drain + 升频 CMD13 安全回退 | 驱动 bug 修复 | P1（diff #1987/#2002 后） |
| C13 | 重返未来 | rsext4 CRC32C 链头尾初值 + tmpfs MemoryNode BLOCKING 标志 | 内核 bug 修复（小） | P1（diff 后独立小 PR） |
| C14 | 重返未来 | COW/ELF loader 可执行页 I-Cache 刷新（全零取指 SIGILL） | 内核 bug 修复 | P1（证据弱，需补测例） |
| C15 | 大象 | RawMutex 先注册后检查（check-then-register 丢唤醒） | 同步原语 bug 修复 | P1（先核 rcore-os/arceos 上游是否仍存在） |
| C16 | 大象 | USB2 PHY 曝光预热、UVC 五重帧校验、iTD 滚动环/rearm 自检 | 驱动设计输入 | P2（吸收设计，不整体合入 fixed_uvc） |
| C17 | 火锅宇泡面 | NPU 电源域/时钟上电顺序经验（clock_profile 消融） | 经验吸收 | P2（按上游 clk/power/DVFS 框架重做） |
| C18 | 重返未来 | 无界页缓存 LRU | 方向参考 | P2（需可回收设计） |
| C19 | 重返未来 | USB bounce 清零 workaround 等 | — | **不建议照搬**（根因存疑） |
| — | 火锅 | GemPool/GemBuffer、sched_setscheduler 系 | — | **已在上游**（#1364 / #986 系） |
| — | 大象 | TTY 空格/Tab、ONLCR、u128 计时、TCSBRK/TCFLSH、SMP SGI 跨核唤醒、rdrive | — | **已在上游/系基线**（#1484/#1922/#1495 等） |

---

## 3. P0 候选详表（建议优先评估）

### C01【YatSenOS】dma-heap cacheable + dma-buf 方向化缓存维护
- **出处**：YatSenOS 报告 §3.8（PDF P42-44）。实测 CPU 读 RGA 输出在 uncached heap 上 149.88 ms，切 cacheable heap 后 6.15 ms；输入链 18.05→8.92 ms（−47%）；32 帧双图交替 bitwise 一致、stale_cache_detected=0。
- **净内容**：内核新增 cacheable 系统堆 / heap；`DMA_BUF_SYNC` 按方向做缓存维护：ReadStart→invalidate_range、WriteEnd→clean_and_invalidate、ReadEnd 空；用户态失败即报错不静默回退。
- **与上游关系**：【代码核验】上游 dma-heap/GEM 路径无此能力——**疑似净新增**。解决"CPU 读加速器（JPU/RGA/VPU/GPU）写出的 DMA 内存"这一通用问题，不绑 RK3588。
- **合入动作**：与现树 `drivers/` dma-heap、rknpu gem sync 路径 diff（报告无 commit，需联系作者拿 fork）；按上游 dma-heap/DMA_BUF_SYNC 设计对照评审；内核侧自含，可独立 PR。属 RK3588 真机场景，需板级验证。
- **注意**：报告同时称其 fork 用"旧版五槽 ABI rknpu"，sync 语义与上游 #1364 是否同构未说明——净变更要以**其 diff 文件 + 现树**双份对照。

### C02【火锅宇泡面】优先级分层 RR + 优先级感知唤醒抢占
- **出处**：报告 §5.1（P22-29）。消融：单队列→多级队列使 rknn p50 1741→626 ms；叠加 I/O 唤醒优化整环 total 1959→363 ms（v4 是最大跃迁点）；QEMU 唤醒→运行延迟 p50 149986→59 µs。
- **净内容**：
  1. axsched 单 RR 队列 → 按优先级分层的多级队列（BTreeMap<isize, List>），RRTask 双优先级（入队原子语义："先设优先级再暴露"，跨层惰性修复）；
  2. wait_queue `notify_one/wake_by_ref` 默认 resched=false 不强制让出 → 唤醒后判 `should_preempt(current)` 置 preempt_pending（sched-rr feature 门控），补上"高优先级唤醒即抢占"一环。
- **与上游关系**：【代码核验】dev `components/axsched/src/round_robin.rs` 仍是单队列、`set_priority()` 返回 false 的桩（round_robin.rs:146）；dev 已有替代调度器 `fifo.rs`/`cfs.rs`（a3868568b1，在 next）与 `preempt_pending`/`notify_one(resched)` 机制——**净新增的是"分层 RR + 优先级感知抢占判定"，不是第三套调度器**。其 sched_setscheduler 等系统调用侧基本已在上游（见 §6）。
- **合入动作**：拆 components/axsched + axtask + starry-kernel 三段；与 fifo/cfs 的 feature 面合并，避免并行机制；补 axsched 单测（QEMU 唤醒延迟可做 host 侧测例）；时间片数值与每 CPU 队列细节报告未给，需向作者要代码。报告无 fork 链接【存疑】。
- **补充**：报告自述"未实现互斥优先级继承"，其"继承"仅指 clone 复制调度属性——合入边界以此为准，别高估。

### C03【大象】rknpu 三核并发提交
- **出处**：报告 §8.5（P74-78），commit `4505b19`（+275/−103，5 文件）。
- **净内容**：串行 submit 对三核拆分图死锁（核间数据依赖）→ 每核状态机 + 三段式调度（清中断→各核发射即返、max 4095 切块→统一轮询、完成续发、无进展 yield_now）；`&mut self`→`&self`（按核独立 MMIO 窗 + unsafe Sync 注释论证）；完成判定收紧为 `status==int_mask` 全等比较；**5 槽双索引语义**：1–2 核图槽=N、三核图槽 2–4；core_mask∩0x7 校验；核租约（进程级原子 CAS 占位）；GEM 池独立锁。验证：multicore_bench 三核并行墙钟≈单核耗时。
- **与上游关系**：【代码核验】上游 `drivers/npu/rockchip-npu`（#1364，gem.rs 21ef4b218e 在 dev）**只有 GEM 管理，无核状态机/并发提交**——若其 job/submit 路径无等价物，此即真增量。但该队驱动本体源自 aios-porting 逆向线（非 #1364 线），**两套驱动代码结构的 diff 是前提**。
- **合入动作**：三点 diff（merge-base 起算）提取净变更 → 移植到现树 rockchip-npu 执行路径 → 实机三核 A/B。**验证通道**：Posad 的 QEMU RKNPU 功能级仿真（processmission/qemu #19，106 qtests）可覆盖部分无板验证。
- **风险**：5 槽语义来自对闭源 librknnrt 0.9.8 的逆向实测【存疑】，runtime 升级即脆弱；报告未给与 Linux rknpu 的对照依据。

### C04【大象】rknpu ABI 错误严格化 + MemSync/destroy 真实现
- **出处**：报告 §8.3（P70-73），commit `cad4e79`（4 文件 +72/−18）。
- **净内容**：① submit 失败被 warn 吞 → `?` 透传 InvalidData（关键路径严格 / 探测路径宽容）；②③ MemSync/MemDestroy 双层空壳 → 真实现，**sync 按 obj_addr 反查**（实测 runtime 填 obj_addr 而非 handle）；④ subcore_task[5] 越界 → len.min()+core_mask 过滤；⑤ 删 submit 前后全量 cache 兜底，职责移交 MemSync（全量接口保留作回退）。
- **与上游关系**：需与 #1364 线 diff——上游若已有同语义修复则跳过【存疑】。
- **合入动作**：同上，取净变更 + 补"错误路径严格化"评审；`obj_addr` 反查的正确性建议对照 librknnrt 官方头文件语义再确认。

### C05【大象】同页双缺页竞态（PTE 查证 + 单页 flush_tlb 重试）
- **出处**：报告 §8.5.5（P76），与 `4505b19` 连带、无独立 hash。
- **净内容**：双线程同 GEM 页同时缺页，后到者见 n==0 误判失败 → 改为查 PTE 确认映射存在后**单页 flush_tlb 重试**。单线程永不触发，三核 bench 首次现形（确定性场景）。
- **与上游关系**：与 Posad 页表/TLB 修复同域但**不同点**——后者是维护序/回收序（next 上以未编号提交落地：`763222dac4` break-before-make 大页切分、`25331b14ed` 回收中间页表前 invalidate），此是并发缺页判定【存疑：是否已被现有 COW/缺页修复链覆盖需 diff】。
- **合入动作**：小而清晰，适合作为独立小 PR + host 测例（多线程共享页并发 fault 的确定性构造）。

### C06【重返未来】rsext4 写回显式化
- **出处**：报告 §4.7-4.8（P20-21）。写/元数据路径隐式 `sync_to_disk()` 全量落盘（数据/bitmap/inode/super/journal）→ 删除隐式落盘、flush 空操作、保留显式 fsync、靠 journal 保一致：close() 3.4–6.2 s → 毫秒级，启动数百秒 → ~30 s。
- **净内容**：写回策略显式化（大方向正确，与 Linux ext4 "事务化延迟落盘"哲学一致）。
- **合入动作/风险**：**策略变更必须评审掉电一致性**——报告无掉电测试【存疑】；其 fork 是 `tty_serial-ajax` 旧线，rsext4 现树结构可能已不同；效果与 SD 升频耦合、无独立数据。建议：先在现树复现"隐式全量落盘是否仍在"→ 若在，按最小改动移除并补掉电/崩溃一致性测例（QEMU + 镜像校验可做一部分）。

---

## 4. P1 候选详表（先 diff / 先取证）

| # | 来源队 | 候选 | 净内容 | 需要的前置动作 | 质量信号 |
|---|---|---|---|---|---|
| C07 | YatSenOS | EHCI periodic **absolute-frame** 修帧号回绕 + ISO 错误 **packet 粒度**传播（不整 URB 停链、保留 actual_length，仅 HSE 等 fatal 停链） | 通用 EHCI 语义正确性（P50-53；120 s 实测 19.9→24.9 FPS、错误计数全 0） | 与 dev crab-usb（#1980 线）EHCI 后端 diff；**其 fork 无 commit 引用**【存疑】；"仅 HSE 停链"边界对照 Linux EHCI 复核 | 实机长跑；无单测 |
| C08 | 大象 | USB2 PHY inno-usb2 复位序列：480 MHz 使能→**退 IDDQ 必须先于复位**→**补缺失的模拟 PHY 复位**（CRU+0x30a10 bit10）→HS 调优→自适应连接等待 250×1 ms | 枚举偶发失败消除（P86-88）；若上游 EHCI/PHY 平台侧同样缺模拟复位则通用 | 从 fixed_uvc（915 行）剥离为独立 PHY 序列，对照上游 inno-usb2/dwc3 phy 现状 | 冷/热启动稳定 |
| C09 | YatSenOS | JPEG 跨 callback 流式组装器（SOI/EOI 状态机、嵌套 SOI 重同步、1–16 MiB 上限） | 任何 MJPEG-over-UVC 消费者通用（P26-30） | 并入网球应用共享库或参考应用；**质量最强候选**：2000 次随机分片 + 20 万帧 + ASan/UBSan + 实机真实跨片恢复 | 主机级回归可做 |
| C10 | 大象 | aka_simulate（真实 RKNN 静态图 + 字节级协议断言）+ aka_driver_test（静态链接自检）+ 四级验证阶梯 + multicore_bench | 无硬件回归设计，与仓库测试文化契合（P100-103） | 提取为独立工具（纯用户态，可移植性最好）；闭源 librknnrt/模型依赖走下载脚本化 | --self-test 断言到字节流、退出码即结果 |
| C11 | 火锅 | observe/drive 双模式 + 9 项阶段计时 + CSV + success=1 统计 + v0–v6 单一变量版本消融 | 整环基准方法论范本（P13-16、P43） | 作为 apps/starry 板级应用测试样板沉淀（绑定其 robot app 一并评估） | 消融表 6-1..6-5 |
| C12 | 重返未来 | SD 识别时钟规范：识别期分频到 ~100 kHz（原 6.25 MHz 超 400 kHz 规范）+ SCR FIFO drain（DTO 先于 RXDR）+ 升频 divider=16 + CMD13 验证失败自动回退 | 小而实 + 安全回退设计（P9-10、P21） | 与 dev sdmmc（#1987 Rockchip 复位、#2002 assigned-clocks）diff；其"保持 U-Boot 时钟不动"策略与上游方向可能已冲突 | 10 步排除表清晰；无自动化测例 |
| C13 | 重返未来 | rsext4 CRC32C 链校验补头尾 `^0xFFFFFFFF`；tmpfs MemoryNode 补 BLOCKING 标志 | 独立小修复 | diff 现树 rsext4/tmpfs 是否同 bug；CRC 可补 Linux 对照向量测试 | e2fsck 修复记录 |
| C14 | 重返未来 | COW/ELF loader 可执行页 I-Cache 刷新（修复"COW 后执行全零取指 SIGILL"） | 疑为独立缺口（页表序修复是维护序非 I-Cache） | 证据链弱于其第二个 SIGILL 根因（PLT 空洞，构建脚本问题）【存疑】；先构造复现测例再动 | 无测例 |
| C15 | 大象 | RawMutex check-then-register 竞态修复（`356a770`："先注册 listener→查锁→await"，unlock 通知不落注册空窗） | 同步原语正确性 | **目标代码不在 tgoskits dev**（axsync 旧线，已统一 ax-sync）→ 若 bug 仍在应提交 **rcore-os/arceos**，先核实现状 | 时序分析完整；5 天压测无复发；无确定性复现【存疑】 |
| C16 | 大象 | UVC 帧五重校验 / iTD 滚动环 + rearm 掉队自检 / 800 ms 曝光预热 / 帧槽 ABI（3 拷贝→1） | fixed_uvc 内的可移植设计点 | 作为上游 EHCI 等时增强与 UVC 类驱动的**设计输入**（对照 crab-usb 路线取舍），不整体合入 | 帧损坏 43%→0；数千帧 errors=0 |

---

## 5. P2 / 参考价值（不直接合代码）

| # | 来源队 | 内容 | 价值与取舍建议 |
|---|---|---|---|
| 参考-1 | YatSenOS | 启动优化全套（login profile 移除、init.sh 最小化、Shell builtin、~320 处日志降级仅 −2.86%） | 代码属其私有 rootfs 定制，不宜照搬；**方法学**（14 段 CNTPCT_EL0 阶段计时定位、日志降级收益小的反直觉结论）值得沉淀 |
| 参考-2 | YatSenOS | 模型重建（27600→8400 候选）与 NPU 优化中"**拒绝把多核收益归因于模型轮**"、多核增量仅 ~3% 的结论 | 对上游 rknpu 后续优化方向有直接指导性：**公共提交路径瘦身 > 多核并行**（与 Posad 回退第三 NPU 核互相印证） |
| 参考-3 | 火锅 | NPU 电源域/时钟 bootargs 私有实现（clock_profile/freq_mhz） | 绕过上游 clk/power/DVFS 框架（assigned-clocks #2002 + rockchip-pm），**不直接合**；"上电域顺序 + 时钟档影响稳定性而非延迟"的经验应吸收进框架内实现（表 6-5 四组消融） |
| 参考-4 | 大象 | fixed_uvc "固定功能直驱 EHCI"整体路线 | 与上游通用栈路线（crab-usb HCD + 通用 UVC）正面冲突，报告自认是比赛选择；整体合入价值低，设计点见 C16 |
| 参考-5 | 重返未来 | 页缓存 64 页→无界 LRU | 方向对但无界实现不可持续（无内存压力回收），上游采纳需可回收 LRU 设计 |
| 参考-6 | 重返未来 | 递进验证体系（mock→QEMU/TCP 在环→真机）+ 提交二分（70ae03b0）+ 诊断无侵入原则 + 系统调用需求清单法 | 方法论成熟可复用，建议作为竞赛/移植项目指南素材（可并入文档站对应分类） |
| 参考-7 | 大象 | 排障纪律（先插桩测量→兜底保命→挖根因→拆脚手架；错误方向修复 17 分钟即 revert；errata 代码克制保留） | 全报告工程纪律标杆，13-commit 复盘时间线（P054）可直接作案例 |
| 参考-8 | 大象 | ballcatch/tennis.cpp（五态机、分段 IBVS、抓取包络标定） | 上游已有同类 app（aka-rk3588 #1976、aka00-tennis-yolo #1594、orangepi-5-plus-tennis 在本地）；闭源 librknnrt.so/模型随包分发仓库不能直接收；**只收方法**（其"标定即回归"/etc/aka-production 放行闸是亮点） |
| 参考-9 | 重返未来 | 旧代码 ACK 失败静默 → 限频 NO-ACK/BAD-RSP/WRITE-FAIL 三分类日志 | 好的下位机调试模式，随应用生态吸收 |

**不建议照搬**：

- 重返未来 "CPU 清零 bounce 页破坏 DMA 可见性 → 删除清零"（P20）：因果反常（报告自认排除 posted write/cache 后仍无机理），疑似掩盖底层 cache/IO 序真 bug——workaround 化修复会固化错误模型，应先定位真因。
- 重返未来 "每 GEM 加 4KB guard page"（治标，DMA burst 越界根因未修）。
- 重返未来 "per-CPU timer wheel → 全局 TimerRuntime"：与 USB 刷新耦合、无独立验证，且现树 timer 结构未知【存疑】。

---

## 6. 已在上游 / 疑似源自上游（不重复提交）

【代码核验】均已在本地现树核实存在；对应队报告多将其写作自研，实为旧基线重复实现或 fork 前已存在于 dev。

| 来源队 | 报告声称 | 现树实况 | 判定 |
|---|---|---|---|
| 火锅 | GemPool/GemBuffer 生命周期管理（destroy 删句柄、Arc retainer 延迟释放、live/resident 双统计） | `drivers/npu/rockchip-npu/src/gem.rs` 已有同名同构实现（21ef4b218e 在 dev，对应 #1364；API 逐名一致、含单测） | 已在上游；其单测可对照上游补缺 |
| 火锅 | sched_setscheduler/setparam/getscheduler 等 4+2 接口、1..99、CAP_SYS_NICE、per-thread 存储、clone 复制 | `os/StarryOS/kernel/src/syscall/task/schedule.rs`（#986 起）已全量实现 NORMAL/FIFO/RR/BATCH/IDLE + 1..99 + RESET_ON_FORK，比其子集更全 | 已在上游；**真增量只剩调度器机制**（→ C02） |
| 大象 | TTY 空格/Tab 被吞、ONLCR 缺失 | `pseudofs/dev/tty/terminal/ldisc.rs:306` 已放行空格/非 ASCII，echo 已有 OPOST/ONLCR 分支（#1484/#1922 线已消化） | 已在上游 |
| 大象 | TCSBRK/TCFLSH 补齐 | `pseudofs/dev/tty/mod.rs:291-316` 已有 TCSBRK/TCSBRKP/TCFLSH 且更全（TCOFLUSH 真实现） | 已在上游 |
| 大象 | aarch64 计时 u64 溢出 → u128 | `platforms/axplat-dyn/src/generic_timer.rs` 已用 u128 换算 | 已在上游 |
| 大象 | 跨核 IPI 唤醒（SGI TargetList、dsb/isb） | dev 已有 #1495 跨核唤醒（`1fa8b95f4c`）；SGI All 广播细节可核对是否在内 | 大概率已在上游【存疑仅余细节】 |
| 大象 | rdrive 动态驱动框架 | 该队**自己声明**为"接手时已有系统基线、不作原创主张"（P039），且其 rdif 0.18 线旧于 dev 的加固线 | 系基线非其净贡献 |
| 大象 | UART3/TTY 板级注册、console 读路径 | dev 无 Manual 模式（已重构）；console 不上报 IRQ 属 axplat-dyn 配置问题【存疑】 | 不适用/待核 |
| 重返未来 | librknnrt ioctl nr=5 应属 MEM_SYNC 的语义错配 | dev rknpu 经 #1364 重构、与 Linux ioctl 表对齐 | 大概率已含【存疑待 diff】 |
| 重返未来 | SUBMIT 结构类型错、subcore_task 未初始化、R/r magic 双兼容 | 同上，与 #1364 线重叠面大 | 逐条 diff 后取真增量 |
| YatSenOS | EHCI iso/iTD 实现、QH Overlay 对齐断言、root port 状态机、coherent DMA 接入 | 上游 crab-usb（#1980 线）已有 UVC 应用且其自称"复用官方 UVC 示例"——iso 路径疑似上游已有【存疑：取决于其 fork 时点】 | 逐行 diff，疑似大部分已在上游 |

---

## 7. 横切观察（对"如何把各队工作收进 dev"的系统性结论）

1. **fork 谱系分化是最大障碍**：tgoskits 直 fork（火锅）、`tty_serial-ajax` 旧线（重返未来）、atomgit aios-porting 逆向线（大象）、oscomp-posad（Posad，唯一保持近基线）——净贡献必须 `merge-base...branch` 三点 diff，不能整分支合。报告不给 fork 链接的两队（YatSenOS、火锅）合入成本最高，**第一步是向作者要 fork + 分支名**。
2. **"闭环能跑"是共性但不是合入标准**：五队全部跑通抓球闭环，但多数修复建立在自己旧基线的缺陷上（dev 已修掉一半）。合入前应要求每项候选**在最新 dev 上重做 A/B**——否则收益数字不可迁移。
3. **方法论高度收敛，值得沉淀**：Posad 判定门（数值门 + AI 对抗评审）、大象四级验证阶梯、火锅 observe/drive 双模式、重返未来提交二分——互相印证"无硬件可判定的验证 + 单一变量消融"是高质量系统工作的共性。可整理成 `docs/` 指南 + starry-test-suit 资产，并反向用作**未来邀请队伍贡献时的验收模板**。
4. **应用层三方并存需一次收敛**：上游已收 `aka-rk3588`(#1976，在 next)、`aka00-tennis-yolo`(#1594，dev)、`orangepi-5-plus-uvc-rknn`(dev)；`orangepi-5-plus-tennis` 在本地；火锅/大象/重返未来各有一份独立 tennis.cpp 变体。建议：把通用件（JPEG 组装器、帧校验、阶段计时基准 CLI、协议三分类日志）抽为共享库/参考应用，比赛变体留在各自 fork。
5. **轮询 vs 中断的方向分歧要明确表态**：大象的 NPU/EHCI/UART 大量轮询路径（自认"通用性让位于确定性"）与上游中断驱动方向相反，吸收其正确性修复时不要把轮询架构一起带进来。
6. **rknpu 是重合重灾区**：YatSenOS/大象/重返未来三队各自修了不同版本的 rknpu（五槽 ABI、GEM、同步、并发），上游 #1364 只覆盖了其中一部分。建议以"#1364 线 + C03/C04 净变更"为准绳做一次收敛评审，避免继续分叉。

---

## 8. 后续行动队列

| 步骤 | 动作 | 产出 | 优先级 |
|---|---|---|---|
| 0 | 向 5 队收集 fork 仓库/分支/diff（YatSenOS、火锅无任何链接，重返未来给的是旧快照；模板：仓库 URL、基线 dev commit、净变更 PR 列表） | 净变更可核对 | 立即 |
| 1 | C05 同页双缺页竞态：先在 next 复现场景 → 确认是否被现有页表/TLB/COW 修复链覆盖（`763222dac4`/`25331b14ed` 等）→ 独立小 PR + host 测例 | 首个可落 PR | 高 |
| 2 | C02 调度器机制：axsched 分层 RR 与 fifo/cfs 的 feature 面合并方案 → 拆三段 PR（axsched/axtask/starry）+ QEMU 唤醒延迟测例 | 净新增落地 | 高 |
| 3 | C01 dma-heap cacheable：向 YatSenOS 要 diff → 对照现树 dma-heap/GEM sync 设计 → 内核 PR + 用户态配套 | RK3588 加速器链通用改进 | 高 |
| 4 | C06 rsext4：现树复现隐式全量落盘 → 显式化 + 掉电一致性测例设计 | FS 写路径性能 | 中 |
| 5 | C03/C04 rknpu：与 #1364 线 diff → 核状态机移植 + 错误严格化；优先用 Posad QEMU RKNPU 仿真做无板验证 → 实机三核 A/B（以 dev 为基线重新计时） | rknpu 执行路径增强 | 中 |
| 6 | C07/C08/C12-C15：逐项三点 diff → 已在现树则关闭（记入 §6），真缺口补测例提 PR | 清单收敛 | 中 |
| 7 | 方法论沉淀：四级验证阶梯 / observe-drive / JPEG 组装器入参考应用与文档站 | 下一轮队伍可直接用 | 低 |
| 8 | 补评审缺口：待仪星队、球球别跑队文档可达后按同模板评审 | 完整覆盖 | 待材料 |

> 落地时的工程纪律照旧：next 集成缓冲区 → 每批能编 + 有测例 → T0/T1 → 上板（RK3588 场景多数需真板或 QEMU RKNPU 仿真）→ 提 PR 到 dev，全程遵守 `AGENTS.md`（补测例、`cargo xtask` 流程、clippy/fmt）。
