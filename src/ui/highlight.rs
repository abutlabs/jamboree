use crate::ui::colors::COLORS;
use ansi_to_tui::IntoText;
use ratatui::prelude::*;

/// Pane text in the client's colours (or plain, without `colors`), with each search term
/// highlighted over them
pub fn highlight_text(text: &str, needles: Option<&[(String, usize)]>, colors: bool) -> Text<'static> {
    let needles = needles.unwrap_or_default();
    let text = match colors {
        true => text.to_string(),
        false => strip_ansi_escapes::strip_str(text),
    };
    Text::from(text.lines().map(|line| highlight_line(line, needles)).collect::<Vec<_>>())
}

fn highlight_line(raw: &str, needles: &[(String, usize)]) -> Line<'static> {
    let line = (raw.into_text().ok())
        .and_then(|text| text.lines.into_iter().next())
        .unwrap_or_default();
    if needles.is_empty() {
        return line;
    }
    let plain: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

    // Find all matches with their positions and which term matched
    let mut matches: Vec<(usize, usize, usize)> = Vec::new(); // (start, end, color_idx)
    for (needle, color_idx) in needles {
        if needle.is_empty() {
            continue;
        }
        for (pos, _) in plain.match_indices(needle.as_str()) {
            matches.push((pos, pos + needle.len(), *color_idx));
        }
    }

    // Sort matches: first by position, then by term index (earlier terms have priority)
    matches.sort_by_key(|&(pos, _, color_idx)| (pos, color_idx));

    // Remove overlapping matches (keep earlier terms when they overlap)
    let mut last_end = 0;
    matches.retain(|&(start, end, _)| {
        let keep = start >= last_end;
        if keep {
            last_end = end;
        }
        keep
    });
    if matches.is_empty() {
        return line;
    }

    // Cut each coloured span where a match starts or ends, and restyle the matched pieces
    let mut spans = Vec::new();
    let mut at = 0;
    for span in line.spans {
        let end = at + span.content.len();
        let mut cuts: Vec<usize> = (matches.iter())
            .flat_map(|&(s, e, _)| [s, e])
            .filter(|&p| p > at && p < end)
            .chain([end])
            .collect();
        cuts.sort();
        let mut from = at;
        for to in cuts {
            let style = match matches.iter().find(|&&(s, e, _)| from >= s && from < e) {
                Some(&(_, _, color_idx)) => {
                    let (fg, bg) = COLORS[color_idx.min(5)];
                    span.style.fg(fg).bg(bg).add_modifier(Modifier::BOLD)
                }
                None => span.style,
            };
            spans.push(Span::styled(span.content[from - at..to - at].to_string(), style));
            from = to;
        }
        at = end;
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_keeps_colours_around_a_match() {
        let line = highlight_line("\x1b[32mState Upd\x1b[0mated ok", &[("Updated".into(), 0)]);
        let spans: Vec<(&str, Option<Color>)> =
            line.spans.iter().map(|s| (s.content.as_ref(), s.style.bg)).collect();
        let (_, bg) = COLORS[0];
        assert_eq!(
            spans,
            [("State ", None), ("Upd", Some(bg)), ("ated", Some(bg)), (" ok", Some(Color::Reset))]
        );
        assert_eq!(line.spans[0].style.fg, Some(Color::Green));
    }

    #[test]
    fn without_client_colors_only_the_search_terms_are_coloured() {
        let text = highlight_text("\x1b[36mdebug line\x1b[0m", Some(&[("line".into(), 0)]), false);
        let spans = &text.lines[0].spans;
        assert_eq!((spans[0].content.as_ref(), spans[0].style.fg), ("debug ", None));
        assert_eq!(spans[1].style.bg, Some(COLORS[0].1));
    }
}
