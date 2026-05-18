const MV_MZ_MESSAGE_WRAP_WIDTH: usize = 48;

pub(crate) fn wrap_mv_mz_message_translation(
    translation: &str,
    compact_blank_lines: bool,
    max_lines: usize,
) -> String {
    let lines = reflow_mv_mz_message_lines(translation, compact_blank_lines)
        .into_iter()
        .filter(|line| !compact_blank_lines || !line.trim().is_empty())
        .flat_map(|line| wrap_mv_mz_message_line(&line, MV_MZ_MESSAGE_WRAP_WIDTH))
        .collect::<Vec<_>>();
    fit_message_lines_to_existing_slots(lines, max_lines).join("\n")
}

pub(crate) fn join_overflow_message_lines(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn fit_message_lines_to_existing_slots(lines: Vec<String>, max_lines: usize) -> Vec<String> {
    if max_lines == 0 || lines.is_empty() {
        return Vec::new();
    }
    if lines.len() <= max_lines {
        return lines;
    }

    let mut fitted = lines[..max_lines.saturating_sub(1)].to_vec();
    let overflow = lines[max_lines.saturating_sub(1)..]
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    fitted.push(join_overflow_message_lines(&overflow));
    fitted
}

fn reflow_mv_mz_message_lines(translation: &str, compact_blank_lines: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let mut paragraph = String::new();

    for raw_line in translation.split('\n') {
        let line = raw_line.trim();
        if line.is_empty() {
            flush_reflowed_message_paragraph(&mut paragraph, &mut lines);
            if !compact_blank_lines {
                lines.push(String::new());
            }
            continue;
        }

        if is_structural_message_line(line) {
            flush_reflowed_message_paragraph(&mut paragraph, &mut lines);
            lines.push(line.to_string());
            continue;
        }

        let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if paragraph.is_empty() {
            paragraph = normalized;
        } else {
            paragraph.push(' ');
            paragraph.push_str(&normalized);
        }
    }

    flush_reflowed_message_paragraph(&mut paragraph, &mut lines);
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn flush_reflowed_message_paragraph(paragraph: &mut String, lines: &mut Vec<String>) {
    if !paragraph.is_empty() {
        lines.push(std::mem::take(paragraph));
    }
}

fn is_structural_message_line(line: &str) -> bool {
    line.contains('\\') && visible_message_width(line) == 0
}

fn wrap_mv_mz_message_line(line: &str, max_visible_width: usize) -> Vec<String> {
    if visible_message_width(line) <= max_visible_width {
        return vec![line.to_string()];
    }

    let mut wrapped = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
            continue;
        }

        let candidate = format!("{current} {word}");
        if visible_message_width(&candidate) <= max_visible_width {
            current = candidate;
        } else {
            wrapped.push(current);
            current = word.to_string();
        }
    }

    if current.is_empty() {
        return hard_wrap_message_line(line, max_visible_width);
    }

    if visible_message_width(&current) > max_visible_width {
        wrapped.extend(hard_wrap_message_line(&current, max_visible_width));
    } else {
        wrapped.push(current);
    }

    wrapped
}

fn hard_wrap_message_line(line: &str, max_visible_width: usize) -> Vec<String> {
    let mut wrapped = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    let chars = line.chars().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < chars.len() {
        if let Some(end) = message_escape_code_end(&chars, index) {
            for ch in &chars[index..end] {
                current.push(*ch);
            }
            index = end;
            continue;
        }

        let ch = chars[index];
        let width = if ch == '　' { 1 } else { 1 };
        if current_width + width > max_visible_width && !current.is_empty() {
            wrapped.push(current);
            current = String::new();
            current_width = 0;
        }
        current.push(ch);
        current_width += width;
        index += 1;
    }

    if !current.is_empty() {
        wrapped.push(current);
    }
    if wrapped.is_empty() {
        wrapped.push(String::new());
    }
    wrapped
}

fn visible_message_width(line: &str) -> usize {
    let mut width = 0usize;
    let chars = line.chars().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < chars.len() {
        if let Some(end) = message_escape_code_end(&chars, index) {
            index = end;
            continue;
        }
        width += 1;
        index += 1;
    }

    width
}

fn message_escape_code_end(chars: &[char], start: usize) -> Option<usize> {
    if chars.get(start) != Some(&'\\') {
        return None;
    }

    let mut index = start + 1;
    let Some(&next) = chars.get(index) else {
        return Some(index);
    };

    if next.is_ascii_alphabetic() {
        while chars.get(index).is_some_and(|ch| ch.is_ascii_alphabetic()) {
            index += 1;
        }
        if chars.get(index) == Some(&'[') {
            index += 1;
            while chars.get(index).is_some_and(|ch| *ch != ']') {
                index += 1;
            }
            if chars.get(index) == Some(&']') {
                index += 1;
            }
        }
        return Some(index);
    }

    Some(index + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_join_trims_blank_lines() {
        assert_eq!(
            join_overflow_message_lines(&[" first ", "", "second "]),
            "first second"
        );
    }

    #[test]
    fn wraps_long_text_without_counting_control_codes() {
        let wrapped = wrap_mv_mz_message_translation(
            "\\N[1] This is a long sentence that should wrap while preserving control codes.",
            true,
            3,
        );

        assert!(wrapped.starts_with("\\N[1]"));
        assert!(wrapped.lines().count() <= 3);
    }
}
