use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};

fn line_indent(line: &str) -> Option<usize> {
    if line.trim().is_empty() {
        return None;
    }
    let mut count = 0usize;
    for ch in line.chars() {
        if ch == ' ' || ch == '\t' {
            count += 1;
        } else {
            break;
        }
    }
    Some(count)
}

fn trim_line_endings(line: &str) -> &str {
    line.trim_end_matches(&['\n', '\r'][..])
}

fn write_blank_line<W: Write>(writer: &mut W, last_output_blank: &mut bool) -> io::Result<()> {
    if !*last_output_blank {
        writer.write_all(b"\n")?;
        *last_output_blank = true;
    }
    Ok(())
}

/// Streamingly process YAML input and write to the provided output.
pub fn process<R: Read, W: Write>(reader: R, writer: W) -> io::Result<()> {
    let mut reader = BufReader::new(reader);
    let mut writer = BufWriter::new(writer);

    let mut prev = String::new();
    if reader.read_line(&mut prev)? == 0 {
        return Ok(());
    }

    let mut last_output_blank = false;

    loop {
        let mut curr = String::new();
        let n = reader.read_line(&mut curr)?;

        let prev_trimmed = trim_line_endings(&prev);
        let prev_indent = line_indent(prev_trimmed);

        if n == 0 {
            writer.write_all(prev.as_bytes())?;
            break;
        }

        let curr_trimmed = trim_line_endings(&curr);
        let curr_indent = line_indent(curr_trimmed);

        if let (Some(p), Some(c)) = (prev_indent, curr_indent) {
            if c > p {
                write_blank_line(&mut writer, &mut last_output_blank)?;
            }
        }

        writer.write_all(prev.as_bytes())?;
        last_output_blank = prev_trimmed.trim().is_empty();

        if let (Some(p), Some(c)) = (prev_indent, curr_indent) {
            if c < p {
                write_blank_line(&mut writer, &mut last_output_blank)?;
            }
        }

        prev = curr;
    }

    writer.flush()?;
    Ok(())
}

/// Format an input &str into a new String.
pub fn format_str(input: &str) -> String {
    let mut output = Vec::new();
    process(input.as_bytes(), &mut output).expect("formatting should not fail");
    String::from_utf8(output).expect("formatter should emit valid UTF-8")
}

/// Format an input &str and append it to the provided String.
pub fn format_into_string(input: &str, output: &mut String) -> io::Result<()> {
    let mut out = StringWriter { target: output };
    process(input.as_bytes(), &mut out)
}

struct StringWriter<'a> {
    target: &'a mut String,
}

impl<'a> Write for StringWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let s = std::str::from_utf8(buf).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "formatter emitted non-utf8")
        })?;
        self.target.push_str(s);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::format_str;

    #[test]
    fn format_str_round_trip() {
        let input = "a:\n  b: 1\nc: 2\n";
        let output = format_str(input);
        assert!(output.contains("a:"));
    }
}
