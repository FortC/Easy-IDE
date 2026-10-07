//! 轻量行级 diff（LCS 动态规划 → unified 格式），供 Agent 文件修改提案预览使用。
//! 超大文件（> 4000 行）退化为整块替换展示，避免 O(n·m) 爆内存。

#[derive(Clone, Copy, PartialEq, Debug)]
enum Op {
    Equal,
    Del,
    Add,
}

/// 行级 LCS diff。返回 (op, 行文本) 序列。
fn diff_lines(old: &[&str], new: &[&str]) -> Vec<(Op, String)> {
    let n = old.len();
    let m = new.len();
    // LCS 表（u32 计数足够；4000×4000×4B = 64MB 上限保护在调用侧）
    let mut dp = vec![0u32; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[idx(i, j)] = if old[i] == new[j] {
                dp[idx(i + 1, j + 1)] + 1
            } else {
                dp[idx(i + 1, j)].max(dp[idx(i, j + 1)])
            };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if old[i] == new[j] {
            out.push((Op::Equal, old[i].to_string()));
            i += 1;
            j += 1;
        } else if dp[idx(i + 1, j)] >= dp[idx(i, j + 1)] {
            out.push((Op::Del, old[i].to_string()));
            i += 1;
        } else {
            out.push((Op::Add, new[j].to_string()));
            j += 1;
        }
    }
    while i < n {
        out.push((Op::Del, old[i].to_string()));
        i += 1;
    }
    while j < m {
        out.push((Op::Add, new[j].to_string()));
        j += 1;
    }
    out
}

/// 生成 unified 风格 diff 文本（@@ hunk 头 + +/-/= 行；上下文各 3 行）
pub fn unified_diff(old: &str, new: &str, path: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    if old_lines.len() > 4000 || new_lines.len() > 4000 {
        return format!(
            "--- {}\n+++ {}\n@@ 文件过大，展示整文件替换（{} 行 → {} 行）@@\n{}\n",
            path,
            path,
            old_lines.len(),
            new_lines.len(),
            new
        );
    }
    let ops = diff_lines(&old_lines, &new_lines);
    let mut out = format!("--- {}\n+++ {}\n", path, path);
    let ctx = 3usize;
    // 找变更区间，按间隔切 hunk
    let changed: Vec<usize> = ops
        .iter()
        .enumerate()
        .filter(|(_, (op, _))| *op != Op::Equal)
        .map(|(i, _)| i)
        .collect();
    if changed.is_empty() {
        return String::new();
    }
    // 合并相邻区间（间隔 ≤ 2*ctx）
    let mut hunks: Vec<(usize, usize)> = Vec::new(); // [start, end]（含变更的 ops 索引闭区间）
    let mut start = changed[0];
    let mut end = changed[0];
    for &c in &changed[1..] {
        if c <= end + 2 * ctx + 1 {
            end = c;
        } else {
            hunks.push((start, end));
            start = c;
            end = c;
        }
    }
    hunks.push((start, end));

    // 行号跟踪（旧/新文件 1 基）
    let mut old_no = 1usize;
    let mut new_no = 1usize;
    // 前置跳过的等行计数
    let mut pos = 0usize;
    for (hs, he) in hunks {
        let lead = ctx.min(hs - 0);
        let from = hs - lead;
        // 跳过 from 之前的等行
        for _ in pos..from {
            old_no += 1;
            new_no += 1;
        }
        let tail = ctx.min(ops.len() - 1 - he);
        let to = (he + tail).min(ops.len() - 1);
        let hunk_old_start = old_no;
        let hunk_new_start = new_no;
        let mut hunk_old_count = 0;
        let mut hunk_new_count = 0;
        let mut body = String::new();
        for k in from..=to {
            let (op, line) = &ops[k];
            match op {
                Op::Equal => {
                    body.push_str(&format!(" {}\n", line));
                    hunk_old_count += 1;
                    hunk_new_count += 1;
                    old_no += 1;
                    new_no += 1;
                }
                Op::Del => {
                    body.push_str(&format!("-{}\n", line));
                    hunk_old_count += 1;
                    old_no += 1;
                }
                Op::Add => {
                    body.push_str(&format!("+{}\n", line));
                    hunk_new_count += 1;
                    new_no += 1;
                }
            }
        }
        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n{}",
            hunk_old_start, hunk_old_count, hunk_new_start, hunk_new_count, body
        ));
        pos = to + 1;
        // pos..ops.len() 之间的等行在下一轮 lead 跳过前先不计数（上面已按实际输出推进 old_no/new_no）
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_basic() {
        let d = unified_diff("a\nb\nc\n", "a\nx\nc\n", "f.txt");
        assert!(d.contains("--- f.txt"));
        assert!(d.contains("-b"));
        assert!(d.contains("+x"));
        assert!(d.contains(" a"));
        assert!(d.contains(" c"));
        assert!(!d.contains("+a"));
    }

    #[test]
    fn diff_no_change() {
        assert_eq!(unified_diff("same\n", "same\n", "f"), "");
    }

    #[test]
    fn diff_add_delete_blocks() {
        let d = unified_diff("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n", "1\n2\nX\n9\n10\n", "f");
        assert!(d.contains("-3"));
        assert!(d.contains("+X"));
        // 中间大量删除也应在同一 hunk 或正确分块
        assert!(d.contains("-4"));
        assert!(d.contains("-8"));
    }

    #[test]
    fn diff_empty_to_content() {
        let d = unified_diff("", "hello\n", "new.txt");
        assert!(d.contains("+hello"));
    }
}
