CREATE TABLE IF NOT EXISTS exam (
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, date TEXT
);
CREATE TABLE IF NOT EXISTS problem (
  id INTEGER PRIMARY KEY, exam_id INTEGER NOT NULL,
  number INTEGER NOT NULL, title TEXT, max_score INTEGER NOT NULL,
  rubric TEXT,                       -- 本题评分标准/参考答案（Markdown，可空）
  UNIQUE(exam_id, number)
);
-- 每题快捷档位，绑判分键槽 0..9（slot 0 = 反引号 ` 键）；满分@9/零分@0 自动预置
CREATE TABLE IF NOT EXISTS score_preset (
  id INTEGER PRIMARY KEY, problem_id INTEGER NOT NULL,
  slot INTEGER NOT NULL, label TEXT NOT NULL, points INTEGER NOT NULL,
  UNIQUE(problem_id, slot)
);
-- 学生来自花名册；无 page 绑定者 = 缺考
CREATE TABLE IF NOT EXISTS student (
  id INTEGER PRIMARY KEY, exam_id INTEGER NOT NULL,
  name TEXT NOT NULL, exam_number TEXT, roster_order INTEGER
);
-- 一张扫描图一条；problem_number = 0 表示姓名页/封面（不判分）
CREATE TABLE IF NOT EXISTS page (
  id INTEGER PRIMARY KEY, exam_id INTEGER NOT NULL,
  student_id INTEGER, problem_number INTEGER,
  image_path TEXT NOT NULL, seq INTEGER, status TEXT
);
-- 判分结果，单元 = (学生,题号)，只存总分 + 状态；preset_id 手动时为 NULL
CREATE TABLE IF NOT EXISTS score (
  id INTEGER PRIMARY KEY,
  student_id INTEGER NOT NULL, problem_id INTEGER NOT NULL,
  total INTEGER, state TEXT NOT NULL DEFAULT 'Ungraded',
  preset_id INTEGER, comment TEXT, grader TEXT, submitted_at TEXT,
  UNIQUE(student_id, problem_id)
);
-- 【占位·v1 不录入】未来题内步级诊断
CREATE TABLE IF NOT EXISTS rubric_step (
  id INTEGER PRIMARY KEY, problem_id INTEGER NOT NULL,
  "order" INTEGER, label TEXT, points INTEGER
);
CREATE TABLE IF NOT EXISTS score_item (
  id INTEGER PRIMARY KEY, score_id INTEGER NOT NULL,
  rubric_step_id INTEGER NOT NULL, earned INTEGER
);
