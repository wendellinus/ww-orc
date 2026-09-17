//! Restore readable horizontal layout before OCR coordinates are discarded.
use paddle_ocr_rs::ocr_result::TextBlock;

#[derive(Debug)]
struct Fragment<'a> {
    text: &'a str,
    left: f64,
    top: f64,
    bottom: f64,
    right: f64,
}

impl Fragment<'_> {
    fn height(&self) -> f64 {
        (self.bottom - self.top).max(1.0)
    }
    fn center(&self) -> f64 {
        (self.top + self.bottom) / 2.0
    }
}

pub fn format_text(blocks: &[TextBlock]) -> String {
    let mut fragments = Vec::new();
    let mut unpositioned = Vec::new();
    for block in blocks {
        let text = block.text.trim();
        if text.is_empty() {
            continue;
        }
        if block.box_points.is_empty() {
            unpositioned.push(text);
            continue;
        }
        let left = block.box_points.iter().map(|p| p.x).min().unwrap() as f64;
        let right = block.box_points.iter().map(|p| p.x).max().unwrap() as f64;
        let top = block.box_points.iter().map(|p| p.y).min().unwrap() as f64;
        let bottom = block.box_points.iter().map(|p| p.y).max().unwrap() as f64;
        fragments.push(Fragment {
            text,
            left,
            right,
            top,
            bottom,
        });
    }
    let mut text = format_fragments(fragments);
    for fragment in unpositioned {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(fragment);
    }
    text
}

// ASCII occupies one monospace column; Chinese and other wide characters occupy two.
fn columns(text: &str) -> usize {
    text.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum()
}

fn format_fragments(mut fragments: Vec<Fragment<'_>>) -> String {
    if fragments.is_empty() {
        return String::new();
    }
    fragments.sort_by(|a, b| {
        a.center()
            .total_cmp(&b.center())
            .then(a.left.total_cmp(&b.left))
    });
    let origin = fragments
        .iter()
        .map(|f| f.left)
        .fold(f64::INFINITY, f64::min);
    let mut widths: Vec<_> = fragments
        .iter()
        .filter(|f| f.right > f.left)
        .map(|f| (f.right - f.left) / columns(f.text).max(1) as f64)
        .collect();
    widths.sort_by(f64::total_cmp);
    let unit = widths
        .get(widths.len() / 2)
        .copied()
        .unwrap_or(8.0)
        .max(1.0);
    let mut rows: Vec<Vec<Fragment<'_>>> = Vec::new();
    for fragment in fragments {
        // Compare with the first fragment, avoiding transitive merging of nearby rows.
        let matching = rows.iter().rposition(|row| {
            let anchor = &row[0];
            (anchor.center() - fragment.center()).abs()
                <= anchor.height().min(fragment.height()) * 0.5
        });
        if let Some(index) = matching {
            rows[index].push(fragment);
        } else {
            rows.push(vec![fragment]);
        }
    }
    let mut output = String::new();
    let mut previous_bottom: Option<f64> = None;
    for mut row in rows {
        row.sort_by(|a, b| a.left.total_cmp(&b.left));
        let top = row.iter().map(|f| f.top).fold(f64::INFINITY, f64::min);
        let bottom = row.iter().map(|f| f.bottom).fold(0.0, f64::max);
        let height = row
            .iter()
            .map(Fragment::height)
            .fold(f64::INFINITY, f64::min);
        if let Some(previous) = previous_bottom {
            output.push('\n');
            if top - previous > height * 1.2 {
                output.push('\n');
            }
        }
        let mut cursor = 0;
        for (index, fragment) in row.iter().enumerate() {
            let target = ((fragment.left - origin) / unit).round() as usize;
            let padding = target
                .saturating_sub(cursor)
                .min(160)
                .max(usize::from(index > 0));
            output.extend(std::iter::repeat_n(' ', padding));
            output.push_str(fragment.text);
            cursor += padding + columns(fragment.text);
        }
        previous_bottom = Some(bottom);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fragment(text: &str, left: f64, top: f64) -> Fragment<'_> {
        Fragment {
            text,
            left,
            top,
            right: left + columns(text) as f64 * 8.0,
            bottom: top + 16.0,
        }
    }
    #[test]
    fn restores_rows_and_columns_from_shuffled_boxes() {
        assert_eq!(
            format_fragments(vec![
                fragment("20", 80.0, 24.0),
                fragment("姓名", 0.0, 0.0),
                fragment("年龄", 80.0, 1.0),
                fragment("小明", 0.0, 24.0)
            ]),
            "姓名      年龄\n小明      20"
        );
    }
    #[test]
    fn separates_distant_paragraphs() {
        assert_eq!(
            format_fragments(vec![
                fragment("第一段", 0.0, 0.0),
                fragment("第二段", 0.0, 64.0)
            ]),
            "第一段\n\n第二段"
        );
    }
    #[test]
    fn does_not_merge_adjacent_lines() {
        assert_eq!(
            format_fragments(vec![fragment("one", 0.0, 0.0), fragment("two", 0.0, 18.0)]),
            "one\ntwo"
        );
    }
    #[test]
    fn empty_result_is_empty() {
        assert_eq!(format_fragments(vec![]), "");
    }
}
