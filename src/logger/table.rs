//! 表格格式化输出模块
//!
//! 用于在 dry-run 或 preview 模式下以表格形式展示生成的样本数据。

use std::cmp::max;

/// 打印表格形式的数据样本
pub fn print_sample_table(rows: &[Vec<String>], headers: &[String]) {
    if rows.is_empty() {
        return;
    }

    // 计算每列最大宽度（按字符数，支持中文）
    let mut col_widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < col_widths.len() {
                let display_len = cell.chars().count().min(30);
                col_widths[i] = max(col_widths[i], display_len);
            }
        }
    }

    let total_width: usize =
        col_widths.iter().sum::<usize>() + 3 * (col_widths.len().saturating_sub(1)) + 4;
    let sep = format!("{:-<1$}", "", total_width);

    // 打印表头
    print_row(&headers, &col_widths);
    println!("{}", sep);

    // 打印数据行
    for row in rows {
        let display_row: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let char_count = cell.chars().count();
                if char_count > 30 {
                    let truncated: String = cell.chars().take(27).collect();
                    format!("{}…", truncated)
                } else {
                    cell.clone()
                }
            })
            .collect();
        print_row(&display_row, &col_widths);
    }
}

/// 打印单行（内部辅助函数）
fn print_row(fields: &[String], widths: &[usize]) {
    print!("│");
    for (i, f) in fields.iter().enumerate() {
        if i >= widths.len() {
            break;
        }
        // 按显示的字符数计算空格填充（注意：这里 width 是字符数，`format!` 期望的是字节数，但在等宽字体下通常没问题）
        // 为了精确对齐，可以使用 `unicode-width` 库，但当前足够。
        print!(" {:width$} │", f, width = widths[i]);
    }
    println!();
}
