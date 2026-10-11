struct LineFormatMasks<'a> {
    in_var_block: &'a [bool],
    in_block_comment: &'a [bool],
    has_line_comment: &'a [bool],
    has_pragma: &'a [bool],
    has_string_literal: &'a [bool],
}

impl<'a> LineFormatMasks<'a> {
    fn in_var_block(&self, idx: usize) -> bool {
        self.in_var_block.get(idx).copied().unwrap_or(false)
    }

    fn skip_alignment(&self, idx: usize) -> bool {
        self.in_block_comment.get(idx).copied().unwrap_or(false)
            || self.has_line_comment.get(idx).copied().unwrap_or(false)
            || self.has_pragma.get(idx).copied().unwrap_or(false)
            || self.has_string_literal.get(idx).copied().unwrap_or(false)
    }

    fn skip_wrapping(&self, idx: usize) -> bool {
        self.skip_alignment(idx)
    }
}

fn align_assignment_ops(lines: &mut [String], masks: &LineFormatMasks<'_>) {
    let mut i = 0usize;
    while i < lines.len() {
        if masks.skip_alignment(i) {
            i += 1;
            continue;
        }
        let indent = leading_whitespace(&lines[i]).to_string();
        let Some(mut max_op) = find_assignment_op(&lines[i]) else {
            i += 1;
            continue;
        };

        let start = i;
        i += 1;
        while i < lines.len() {
            let line = &lines[i];
            if masks.skip_alignment(i) {
                break;
            }
            if leading_whitespace(line) != indent {
                break;
            }
            let Some(op_idx) = find_assignment_op(line) else {
                break;
            };
            max_op = max_op.max(op_idx);
            i += 1;
        }

        for line in lines.iter_mut().take(i).skip(start) {
            if let Some(op_idx) = find_assignment_op(line)
                && op_idx < max_op
            {
                let padding = " ".repeat(max_op - op_idx);
                line.insert_str(op_idx, &padding);
            }
        }
    }
}

fn find_assignment_op(line: &str) -> Option<usize> {
    let assign = line.find(":=");
    let arrow = line.find("=>");
    match (assign, arrow) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn leading_whitespace(line: &str) -> &str {
    let end = line
        .chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| c.len_utf8())
        .sum();
    &line[..end]
}

fn wrap_long_lines(
    lines: &[String],
    masks: &LineFormatMasks<'_>,
    indent_unit: &str,
    max_len: usize,
) -> Vec<String> {
    let mut output = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        if masks.in_var_block(idx) {
            output.push(line.clone());
            continue;
        }
        if masks.skip_wrapping(idx) {
            output.push(line.clone());
            continue;
        }
        if line.len() <= max_len || !line.contains(',') {
            output.push(line.clone());
            continue;
        }
        let indent = leading_whitespace(line);
        let continuation = format!("{indent}{indent_unit}");
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() <= 1 {
            output.push(line.clone());
            continue;
        }
        let mut current = parts[0].trim_end().to_string();
        for part in parts.iter().skip(1) {
            output.push(format!("{current},"));
            current = format!("{continuation}{}", part.trim_start());
        }
        output.push(current);
    }
    output
}
