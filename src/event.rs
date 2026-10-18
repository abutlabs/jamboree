use crate::AppState;
use crate::ViewMode;
use crate::net::Net;
use crate::node::{self, Node};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Position;
use std::time::{Duration, Instant};

pub enum Action {
    Continue,
    Quit,
    ChangeView(ViewMode),
}

pub fn handle_key_event(
    key: KeyCode,
    modifiers: KeyModifiers,
    app: &mut AppState,
    focused: &mut usize,
    nodes: &mut [Node],
    net: Option<&Net>,
) -> Action {
    // If help is showing, any key closes it
    if app.show_help {
        app.show_help = false;
        app.last_key = None;
        return Action::Continue;
    }

    if app.search_mode {
        app.handle_search_key(key);
        app.last_key = None;
        return Action::Continue;
    }

    // Check if scrolling is allowed (paused or log viewer mode)
    let can_scroll = app.paused || app.log_viewer_mode;
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);

    // Vi scroll keys auto-pause first
    let vi_scroll = match key {
        KeyCode::Char('g') => !ctrl && app.last_key == Some('g'),
        KeyCode::Char('G' | 'j' | 'k') => true,
        KeyCode::Char('d' | 'u' | 'f' | 'b') => ctrl,
        KeyCode::Char('n' | 'N') => app.highlight.is_some(),
        KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => true,
        KeyCode::Home | KeyCode::End => true,
        _ => false,
    };
    if vi_scroll && !can_scroll {
        app.toggle_pause(nodes);
    }

    // Main key handling
    let action = match key {
        // Help - always available
        KeyCode::Char('?') => {
            app.toggle_help();
            Action::Continue
        }

        // Search - always available
        KeyCode::Char('/') => {
            app.search_mode = true;
            if let Some(ref terms) = app.highlight {
                app.search_input = terms
                    .iter()
                    .map(|(term, _)| term.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
            }
            Action::Continue
        }

        // Grep toggle with Ctrl+G - always available
        KeyCode::Char('g') if ctrl => {
            app.toggle_grep_mode();
            Action::Continue
        }

        // Vi: gg - go to top (second g after first g)
        KeyCode::Char('g') if app.last_key == Some('g') => {
            app.scroll_to_top(*focused);
            Action::Continue
        }

        // First 'g' press - wait for potential 'gg'
        KeyCode::Char('g') => {
            app.last_key = Some('g');
            return Action::Continue; // Return early to preserve last_key
        }

        // Vi: G - go to bottom
        KeyCode::Char('G') => {
            app.scroll_to_bottom(*focused);
            Action::Continue
        }

        // Vi: j - scroll down one line
        KeyCode::Char('j') => {
            app.scroll_delta(*focused, 1);
            Action::Continue
        }

        // Vi: k - scroll up one line
        KeyCode::Char('k') => {
            app.scroll_delta(*focused, -1);
            Action::Continue
        }

        // Vi: Ctrl+d - scroll down half page
        KeyCode::Char('d') if ctrl => {
            app.scroll_half_page(*focused, true);
            Action::Continue
        }

        // Vi: Ctrl+u - scroll up half page
        KeyCode::Char('u') if ctrl => {
            app.scroll_half_page(*focused, false);
            Action::Continue
        }

        // Vi: Ctrl+f - scroll down full page
        KeyCode::Char('f') if ctrl => {
            app.scroll_full_page(*focused, true);
            Action::Continue
        }

        // Vi: Ctrl+b - scroll up full page
        KeyCode::Char('b') if ctrl => {
            app.scroll_full_page(*focused, false);
            Action::Continue
        }

        // Vi: n - jump to next search match
        KeyCode::Char('n') => {
            if app.highlight.is_some() {
                let text = nodes[*focused].get_text_tail(5_000_000);
                let filtered_text = app.filter_text(&text);
                app.jump_to_next_match(*focused, &filtered_text);
            }
            Action::Continue
        }

        // Vi: N - jump to previous search match
        KeyCode::Char('N') => {
            if app.highlight.is_some() {
                let text = nodes[*focused].get_text_tail(5_000_000);
                let filtered_text = app.filter_text(&text);
                app.jump_to_prev_match(*focused, &filtered_text);
            }
            Action::Continue
        }

        // Arrow, page, Home and End keys scroll, like the vi keys
        KeyCode::Up => {
            app.scroll_delta(*focused, -1);
            Action::Continue
        }
        KeyCode::Down => {
            app.scroll_delta(*focused, 1);
            Action::Continue
        }
        KeyCode::PageUp => {
            app.scroll_full_page(*focused, false);
            Action::Continue
        }
        KeyCode::PageDown => {
            app.scroll_full_page(*focused, true);
            Action::Continue
        }
        KeyCode::Home => {
            app.scroll_to_top(*focused);
            Action::Continue
        }
        KeyCode::End => {
            app.scroll_to_bottom(*focused);
            Action::Continue
        }

        // Freeze/unfreeze all panes; F follows the logs again, as in less
        KeyCode::Char(' ') if !app.log_viewer_mode => {
            app.toggle_pause(nodes);
            Action::Continue
        }
        KeyCode::Char('F') if app.paused && !app.log_viewer_mode => {
            app.toggle_pause(nodes);
            Action::Continue
        }

        // Stop/continue the focused node (SIGSTOP/SIGCONT), marked in every log
        KeyCode::Char('s') if !app.log_viewer_mode => {
            node::set_stopped(nodes, *focused, !nodes[*focused].is_stopped());
            Action::Continue
        }

        // Cut the focused node off from all others, or heal it (every node behind jamboree's relay)
        KeyCode::Char('i') => {
            if let Some(net) = net {
                net.set_isolated(nodes, *focused, !net.is_isolated(*focused));
            }
            Action::Continue
        }

        // Split the network at the focused node (nodes before it | it and after), or heal it
        KeyCode::Char('p') => {
            let at = *focused;
            let split = net.is_some_and(|net| net.split_side(at).is_none());
            match net {
                Some(_) if split && at == 0 => {
                    nodes[0].mark("to split, focus the first node of side B")
                }
                Some(net) => {
                    let sides = (0..nodes.len()).map(|k| usize::from(split && k >= at));
                    net.set_split(nodes, sides.collect())
                }
                None => {}
            }
            Action::Continue
        }

        // Restart the focused node: kill it and run its command again
        KeyCode::Char('r') if !app.log_viewer_mode => {
            node::restart(nodes, *focused);
            Action::Continue
        }

        // Shutdown
        KeyCode::Char('x') if !app.log_viewer_mode => {
            app.shutdown_all_nodes(nodes);
            Action::Continue
        }

        // Quit
        KeyCode::Char('q') => Action::Quit,

        // Focus the next or previous node
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
            *focused = (*focused + 1) % nodes.len();
            Action::Continue
        }
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
            *focused = (*focused + nodes.len() - 1) % nodes.len();
            Action::Continue
        }

        // Zoom
        KeyCode::Enter | KeyCode::Char('z') => Action::ChangeView(ViewMode::Zoomed),

        // Node visibility toggle
        KeyCode::Char(c @ '0'..='9') => {
            let idx = (c as usize) - ('0' as usize);
            if idx < nodes.len() {
                app.hidden[idx] = !app.hidden[idx];
            }
            Action::Continue
        }

        // Back to grid
        KeyCode::Esc => Action::ChangeView(ViewMode::Grid),

        _ => Action::Continue,
    };

    // Clear last_key after processing (except when we're waiting for 'gg')
    if key != KeyCode::Char('g') || app.last_key == Some('g') {
        app.last_key = None;
    }

    action
}

/// Click focuses a pane and a second click on it zooms; the wheel scrolls the pane under the
/// pointer, freezing the panes first
pub fn handle_mouse_event(
    mouse: MouseEvent,
    app: &mut AppState,
    focused: &mut usize,
    nodes: &[Node],
    zoomed: bool,
) -> Action {
    let under = (app.panes.borrow().iter())
        .find(|(_, r)| r.contains(Position::new(mouse.column, mouse.row)))
        .map(|(i, _)| *i);
    let scroll = match mouse.kind {
        MouseEventKind::ScrollUp => -3,
        MouseEventKind::ScrollDown => 3,
        MouseEventKind::Down(MouseButton::Left) if !zoomed => {
            let Some(i) = under else {
                return Action::Continue;
            };
            let double = app.last_click.is_some_and(|(j, at)| {
                j == i && at.elapsed() < Duration::from_millis(400)
            });
            *focused = i;
            app.last_click = Some((i, Instant::now()));
            return match double {
                true => Action::ChangeView(ViewMode::Zoomed),
                false => Action::Continue,
            };
        }
        _ => return Action::Continue,
    };
    if !zoomed {
        match under {
            Some(i) => *focused = i,
            None => return Action::Continue,
        }
    }
    if !app.paused && !app.log_viewer_mode {
        app.toggle_pause(nodes);
    }
    app.scroll_delta(*focused, scroll);
    Action::Continue
}

/// A time like 90s or 2m
pub fn parse_duration(w: &str) -> Result<Duration, String> {
    let secs = match w.char_indices().last() {
        Some((at, 's')) => w[..at].parse().ok(),
        Some((at, 'm')) => w[..at].parse().ok().map(|m: u64| m * 60),
        _ => None,
    };
    secs.map(Duration::from_secs)
        .ok_or(format!("'{w}' is not a time like 90s or 2m"))
}

/// A --fault action; split lists the nodes of each side
#[derive(Clone, Debug)]
pub enum Fault {
    Stop(usize),
    Isolate(usize),
    Split(Vec<Vec<usize>>),
    Restart(usize),
    Heal,
}

impl Fault {
    /// Parse `[<time>] <action>`, e.g. `60s split 0,1,2/3,4,5`; no time means at the start
    pub fn parse(s: &str) -> Result<(Duration, Fault), String> {
        let mut words: Vec<&str> = s.split_whitespace().collect();
        let at = match words.first().and_then(|w| parse_duration(w).ok()) {
            Some(at) => {
                words.remove(0);
                at
            }
            None => Duration::ZERO,
        };
        let index = |w: &str| w.parse().map_err(|_| format!("'{w}' is not a node index"));
        let fault = match words[..] {
            ["stop", i] => Fault::Stop(index(i)?),
            ["isolate", i] => Fault::Isolate(index(i)?),
            ["restart", i] => Fault::Restart(index(i)?),
            ["split", sides] => Fault::Split(
                (sides.split('/'))
                    .map(|side| side.split(',').map(index).collect())
                    .collect::<Result<_, _>>()?,
            ),
            ["heal"] => Fault::Heal,
            _ => {
                return Err(format!(
                    "'{s}': expected [<time>] stop N | isolate N | split A/B | restart N | heal, \
                 e.g. '60s split 0,1,2/3,4,5'"
                ));
            }
        };
        Ok((at, fault))
    }

    /// Check the node indices against n nodes
    pub fn check(&self, n: usize) -> Result<(), String> {
        let nodes = match self {
            Fault::Stop(i) | Fault::Isolate(i) | Fault::Restart(i) => vec![*i],
            Fault::Split(sides) => sides.concat(),
            _ => vec![],
        };
        if let Some(i) = nodes.iter().find(|&&i| i >= n) {
            return Err(format!("--fault: there is no node {i} in {n} nodes"));
        }
        if let Fault::Split(sides) = self {
            let mut sorted = sides.concat();
            sorted.sort();
            if n < 2 || sides.iter().any(Vec::is_empty) || sorted != (0..n).collect::<Vec<_>>() {
                return Err(format!(
                    "--fault split: each of the {n} nodes must be on exactly one side"
                ));
            }
        }
        Ok(())
    }

    /// Apply the fault as its key would, with the same marks in every log
    pub fn apply(&self, nodes: &mut [Node], net: &Net) {
        match self {
            Fault::Stop(i) => node::set_stopped(nodes, *i, true),
            Fault::Isolate(i) => net.set_isolated(nodes, *i, true),
            Fault::Restart(i) => node::restart(nodes, *i),
            Fault::Split(sides) => {
                let mut side = vec![0; nodes.len()];
                for (s, members) in sides.iter().enumerate() {
                    members.iter().for_each(|&i| side[i] = s);
                }
                net.set_split(nodes, side)
            }
            Fault::Heal => {
                for i in 0..nodes.len() {
                    if nodes[i].is_stopped() {
                        node::set_stopped(nodes, i, false);
                    }
                    if net.is_isolated(i) {
                        net.set_isolated(nodes, i, false);
                    }
                }
                if net.split_side(0).is_some() {
                    net.set_split(nodes, vec![0; nodes.len()]);
                }
            }
        }
    }

    /// Whether the fault cuts nodes off, so must wait until every node is behind the relay
    pub fn cuts(&self) -> bool {
        matches!(self, Fault::Isolate(_) | Fault::Split(_))
    }
}
