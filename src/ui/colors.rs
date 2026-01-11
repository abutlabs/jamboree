use ratatui::prelude::Color;

// Color palette for up to 6 search terms
pub const COLORS: [(Color, Color); 6] = [
    (Color::Black, Color::Yellow),  // Term 1: Yellow background
    (Color::Black, Color::Cyan),    // Term 2: Cyan background
    (Color::White, Color::Magenta), // Term 3: Magenta background
    (Color::Black, Color::Green),   // Term 4: Green background
    (Color::Black, Color::Blue),    // Term 5: Blue background
    (Color::White, Color::Red),     // Term 6: Red background
];
