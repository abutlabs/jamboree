use ratatui::prelude::*;
use crate::ui::colors::COLORS;

pub fn highlight_text(text: &str, needles: Option<&[(String, usize)]>) -> Text<'static> {
    let Some(needles) = needles else {
        return Text::raw(text.to_string());
    };
    
    if needles.is_empty() {
        return Text::raw(text.to_string());
    }

    let mut lines = Vec::new();

    for line in text.lines() {
        // Find all matches with their positions and which term matched
        let mut matches: Vec<(usize, usize, usize)> = Vec::new(); // (position, length, color_idx)
        
        for (needle, color_idx) in needles.iter().map(|(n, c)| (n, c)) {
            if needle.is_empty() {
                continue;
            }
            let mut search_pos = 0;
            while let Some(pos) = line[search_pos..].find(needle) {
                let absolute_pos = search_pos + pos;
                matches.push((absolute_pos, needle.len(), *color_idx));
                search_pos = absolute_pos + needle.len();
            }
        }
        
        // Sort matches: first by position, then by term index (earlier terms have priority)
        matches.sort_by_key(|&(pos, _, color_idx)| (pos, color_idx));
        
        // Remove overlapping matches (keep earlier terms when they overlap)
        let mut filtered_matches = Vec::new();
        let mut last_end = 0;
        for (pos, len, color_idx) in matches {
            if pos >= last_end {
                filtered_matches.push((pos, len, color_idx));
                last_end = pos + len;
            }
        }
        
        // Build spans
        let mut spans = Vec::new();
        let mut current_pos = 0;
        
        for (pos, len, color_idx) in filtered_matches {
            // Add text before match
            if pos > current_pos {
                spans.push(Span::raw(line[current_pos..pos].to_string()));
            }
            
            // Add highlighted match using the stored color index
            let (fg, bg) = COLORS[color_idx.min(5)];
            spans.push(Span::styled(
                line[pos..pos + len].to_string(),
                Style::default()
                    .fg(fg)
                    .bg(bg)
                    .add_modifier(Modifier::BOLD),
            ));
            
            current_pos = pos + len;
        }
        
        // Add remaining text
        if current_pos < line.len() {
            spans.push(Span::raw(line[current_pos..].to_string()));
        }
        
        lines.push(Line::from(spans));
    }

    Text::from(lines)
}
