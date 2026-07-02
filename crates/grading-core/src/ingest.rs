use anyhow::Result;
use crate::Db;
use std::cmp::Ordering;

/// 自然排序比较：并行扫描两个字符串；同为数字时按整段数值比较，
/// 否则按字符（大小写不敏感）逐个比较。这样 "1.jpg" 与 "img2.png" 这类
/// 数字开头 vs 字母开头的混合场景也能得到符合直觉的顺序（数字排在字母前，
/// 因为 ASCII 中数字字符本就小于字母字符）。
fn compare_nat(a: &str, b: &str) -> Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) => {
                if ca.is_ascii_digit() && cb.is_ascii_digit() {
                    let mut na = String::new();
                    while let Some(&d) = ai.peek() {
                        if d.is_ascii_digit() { na.push(d); ai.next(); } else { break; }
                    }
                    let mut nb = String::new();
                    while let Some(&d) = bi.peek() {
                        if d.is_ascii_digit() { nb.push(d); bi.next(); } else { break; }
                    }
                    let va: u64 = na.parse().unwrap_or(u64::MAX);
                    let vb: u64 = nb.parse().unwrap_or(u64::MAX);
                    match va.cmp(&vb) {
                        Ordering::Equal => continue,
                        other => return other,
                    }
                } else {
                    let (la, lb) = (ca.to_ascii_lowercase(), cb.to_ascii_lowercase());
                    match la.cmp(&lb) {
                        Ordering::Equal => { ai.next(); bi.next(); continue; }
                        other => return other,
                    }
                }
            }
        }
    }
}

pub fn sort_scan_order(mut files: Vec<String>) -> Vec<String> {
    files.sort_by(|a, b| compare_nat(a, b));
    files
}

pub fn next_seq(db: &Db, exam_id: i64) -> Result<i64> {
    let n: Option<i64> = db.conn.query_row(
        "SELECT MAX(seq) FROM page WHERE exam_id=?1", [exam_id], |r| r.get(0))?;
    Ok(n.map(|x| x + 1).unwrap_or(0))
}

pub fn add_ingested_page(db: &Db, exam_id: i64, filename: &str, seq: i64) -> Result<i64> {
    db.conn.execute(
        "INSERT INTO page(exam_id, student_id, problem_number, image_path, seq, status)
         VALUES(?1, NULL, NULL, ?2, ?3, 'ingested')",
        (exam_id, filename, seq))?;
    Ok(db.conn.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Db, setup::create_exam};

    #[test]
    fn natural_sort_orders_numbers_by_value() {
        let got = sort_scan_order(vec![
            "10.jpg".into(), "2.jpg".into(), "1.jpg".into(), "img11.png".into(), "img2.png".into(),
        ]);
        assert_eq!(got, vec!["1.jpg", "2.jpg", "10.jpg", "img2.png", "img11.png"]);
    }

    #[test]
    fn next_seq_and_add_ingested_page() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "E", "2026-07-03").unwrap();
        assert_eq!(next_seq(&db, exam).unwrap(), 0);
        let id = add_ingested_page(&db, exam, "a.jpg", 0).unwrap();
        assert!(id > 0);
        add_ingested_page(&db, exam, "b.jpg", 1).unwrap();
        assert_eq!(next_seq(&db, exam).unwrap(), 2); // max(seq)=1 → next 2
        // 未标注：student_id/problem_number 为 NULL，status='ingested'
        let (sid, pn, st): (Option<i64>, Option<i64>, String) = db.conn.query_row(
            "SELECT student_id, problem_number, status FROM page WHERE image_path='a.jpg'",
            [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!((sid, pn, st.as_str()), (None, None, "ingested"));
    }
}
