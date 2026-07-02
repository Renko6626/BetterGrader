# 阅卷辅助器 · Ingest(M4) + Label(M2) 实现设计

> 日期：2026-07-03
> 基于已定稿的 v0.2 设计（`2026-07-02-阅卷辅助器-design.md` §6 采集 / §7 打标签），本文档只补**实现层决策**，不改已定的产品设计。
> 目标：把工具从"批演示数据"推进到"**指着一个真实扫描图文件夹 → 导入 → 标注 → 全键盘批任意题 → 导出成绩表**"的闭环。

前置状态：M0/M1（判分循环）+ M3（持久化每场 `exam.db` 目录 + 导出）已合入 `main`。本片一份计划一口气交付 Ingest → Label → 真图显示 + 判分选题。

---

## 1. Ingest（M4）——导入图片文件夹

- 前端用 `@tauri-apps/plugin-dialog` 的 `open({directory:true})` 选**一个图片文件夹**，把路径传给命令 `ingest_folder(dir)`。
- `ingest_folder` 在 Rust 侧：枚举该目录下 `jpg/jpeg/png/webp`（大小写不敏感），**按文件名自然排序**（`1,2,…,10,11` 而非 `1,10,11,2`）确定扫描序，逐张**拷贝**进当前考试的 `images/`（原件不动），每张插一条 `page`：
  - `student_id = NULL`、`problem_number = NULL`（未标注）、`seq = 递增`、`status = 'ingested'`、`image_path = 文件名`（仅文件名，相对 `images/`）。
- **`image_path` 只存文件名**：显示时解析为 `<考试目录>/images/<文件名>`，保证整场目录可整体拎走归档。
- **再次导入 = 追加**：新图 seq 接着当前最大 seq 往后排，支持分批扫、分批导。文件名冲突时加序号后缀避免覆盖。
- 数据不出本地：纯本地文件拷贝，无网络。

## 2. 真实扫描图显示（Ingest/Label/判分共用基建）

- 命令 `read_image(filename: String) -> Vec<u8>`：在当前考试 `images/` 下读该文件返回字节（路径限定在 `images/` 内，拒绝越界路径）。
- 前端：`URL.createObjectURL(new Blob([bytes]))` 得到 blob URL 显示，组件卸载/翻页后 `revokeObjectURL` 释放。
- **判分连续翻页时预取下一张**，减少等待。
- 选型理由：一次显示一张、每张几百 KB~2MB，IPC 读字节完全够用；**彻底绕开 Tauri asset 协议的动态作用域授权**（考试目录动态、asset scope 难动态授权，且 capability 配置是易漏的雷——见 M3 `dialog:allow-save` 教训）。纯 app 命令默认放行，无需 capability。asset 协议留作"量大卡顿再优化"的后续。

## 3. Label（M2）——命门级，核心投入

### 3.1 界面
单图浏览屏：`←/→` 按 seq 翻页；右侧常驻状态栏——**当前学生**、**题号进度 k/N**、**未标注张数**。

### 3.2 纯键盘 reducer `useLabelKeys`（Vitest 单测，复用命门那套）
把打标签键盘逻辑抽成无副作用 reducer（输入 state+key → 输出 state+effect），与 Vue 渲染解耦、可单测：

| 键 | 动作 |
|---|---|
| `←` / `→` | 翻上/下一张（按 seq） |
| `S` / `Enter` | 在**姓名页**定新学生边界 → 触发花名册选人；确认后该页记 `problem_number = 0`（封面，不判分），其后答题页起始题号归 1 |
| `C` | 接上一题（溢出续页，题号计数器**不前进**） |
| `N` | 跳题/缺题（声明本页是题 k、题 k−1 空缺，计数器**跳进**） |
| 数字/回车（选人态） | 在花名册选人框内搜姓名 / 键考号，回车确认 |

- 边界之后的答题页**自动编号 1..N**；`C`/`N` 是对称的"退/进"修正键。
- **花名册选人**：搜索框按姓名或考号过滤，回车确认绑定。**查无此人**（转学/临时加/名册漏录）→ 允许**临时新增学生**或标**待匹配**，待匹配在确认总表单列。

### 3.3 §7.0 承重不变量（软件层强制）
每个学生答题页数须 == 题数 N（缺答靠物理塞空白页占位，见 v0.2 §7.0）。软件层：`页数 == N` 走自动编号快路径；`≠ N` **绝不盲目自增**——在确认总表**高亮报警**并**强制逐页指认题号 / 标出缺哪题**。

### 3.4 增量写库（可续标）
每确认一步（定边界、绑人、编号一页、C/N）立即写对应 `page` 行（`student_id`/`problem_number`/`status='labeled'`）。随时关掉重开可接着标，不丢。前端仍保留响应式工作态负责翻页/回改的即时反馈，但真源是库。

### 3.5 确认总表 + 缺考
- "叠 ↔ 学生 ↔ 各页题号" 全表可视化，翻完扫一眼、纠错位；**页数对不上的叠红标**。
- **缺考检测**：花名册有、但 Label 结束仍无任何 page 绑定的学生，单列列出（与 M3 导出的缺考口径一致）。

## 4. 判分补口（有真数据后必须补）

- **GradeView 选题**：顶部加题目选择器（下拉/列表），选题 → 载该题队列。替换现在写死的 `problemNumber = 1`。
- 判分左侧图像换成 `read_image` 真图（替 `fake://` 占位；文件缺失仍降级为带标签占位块）。

## 5. 数据模型 delta

**零 schema 改动。** `page.student_id` / `page.problem_number` 本就可空（承载"未标注"），`page.status` 承载 `ingested`/`labeled`。本片只加代码逻辑与命令。

## 6. 新增/改动接口概览

- Rust `grading-core`：`ingest::ingest_folder(...)`（枚举/排序/拷贝/建行/追加）、`page` 标注写入（绑定/边界/编号/C/N/页数校验）、未标注与确认总表查询、缺考查询。
- `src-tauri`：命令 `ingest_folder(dir)`、`read_image(filename)`、Label 相关标注命令；capabilities 沿用 M3（dialog open/save 已授；`read_image` 是 app 命令无需授权）。
- 前端：`src/composables/useLabelKeys.ts`（纯 reducer + 单测）、`src/views/LabelView.vue`（浏览器+确认总表）、`src/views/IngestView.vue`（或并入 Setup 的导入入口）、GradeView 选题 + 真图、`api.ts`/`types.ts` 同步。

## 7. 测试策略

- **Rust `cargo test`**：ingest（自然排序、拷贝落地、建未标注行、再导追加、文件名冲突）、read_image（读字节、越界路径拒绝）、Label 落库（绑定/边界/C接续/N跳题/页数≠N 报警/缺考）。
- **前端 Vitest**：`useLabelKeys` 纯 reducer 全键位单测（翻页/定边界/选人态/C/N/计数）。
- **端到端（CDP）**：指真实图文件夹 → 导入 → 标注（造一个溢出、一个跳题、一个缺考）→ 确认总表纠错 → 判分任意题 → 导出。**教训**：凡涉原生对话框/capability 的路径 CDP 难驱动，计划中显式标注"需人工点一次 + 靠单测/ACL 断言兜底"，不谎报已验证。

## 8. 明确的后续缺口（非本片）

- 二维码/OCR 自动识别学生与题号（永远只当辅助校验，见 v0.2 §7.4）。
- 成像/矫正段收进本工具（jscanify/OpenCV，见 v0.2 §6 中期演进）。
- asset 协议按需加载大图（性能优化）。
- 题内步级诊断 `score_item`（占位表已在，v1 不录）。
