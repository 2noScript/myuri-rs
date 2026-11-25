use std::collections::HashMap;

use crate::rng::Rng;

const ROWS: [&str; 4] = [
    "`1234567890-=",
    "qwertyuiop[]\\",
    "asdfghjkl;'",
    "zxcvbnm,./",
];

/// Keyboard layout model for neighbor-key typo lookups.
#[derive(Debug, Clone)]
pub struct KeyboardLayout {
    grid: Vec<Vec<char>>,
    pos_map: HashMap<char, (usize, usize)>,
}

impl Default for KeyboardLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardLayout {
    pub fn new() -> Self {
        let grid: Vec<Vec<char>> = ROWS.iter().map(|r| r.chars().collect()).collect();
        let mut pos_map = HashMap::new();
        for (r, row) in grid.iter().enumerate() {
            for (c, &ch) in row.iter().enumerate() {
                pos_map.insert(ch, (r, c));
            }
        }
        Self { grid, pos_map }
    }

    pub fn is_composed_accent(&self, ch: char) -> bool {
        let lower = ch.to_lowercase().next().unwrap_or(ch);
        matches!(
            lower,
            'â' | 'ê' | 'î' | 'ô' | 'û' | 'ä' | 'ë' | 'ï' | 'ö' | 'ü' | 'é' | 'è' | 'à' | 'ù' | 'ç'
        )
    }

    pub fn normalize_char(&self, ch: char) -> char {
        let lower = ch.to_lowercase().next().unwrap_or(ch);
        match lower {
            'â' | 'ä' | 'à' => 'a',
            'ê' | 'ë' | 'é' | 'è' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'û' | 'ü' | 'ù' => 'u',
            'ç' => 'c',
            other => other,
        }
    }

    pub fn has_key(&self, ch: char) -> bool {
        self.pos_map.contains_key(&self.normalize_char(ch))
    }

    pub fn get_neighbor_keys(&self, ch: char) -> Vec<char> {
        let normalized = self.normalize_char(ch);
        let Some(&(r, c)) = self.pos_map.get(&normalized) else {
            return Vec::new();
        };
        let deltas: [(isize, isize); 8] = [
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, -1),
            (0, 1),
            (1, -1),
            (1, 0),
            (1, 1),
        ];
        let mut neighbors = Vec::new();
        for (dr, dc) in deltas {
            let nr = (r as isize) + dr;
            let nc = (c as isize) + dc;
            if nr >= 0 && (nr as usize) < self.grid.len() {
                let row = &self.grid[nr as usize];
                if nc >= 0 && (nc as usize) < row.len() {
                    neighbors.push(row[nc as usize]);
                }
            }
        }
        neighbors
    }

    pub fn get_random_neighbor(&self, ch: char, rng: &Rng) -> char {
        let upper_s = ch.to_uppercase().to_string();
        let lower_s = ch.to_lowercase().to_string();
        let ch_s = ch.to_string();
        let was_upper = ch_s == upper_s && ch_s != lower_s;

        let neighbors = self.get_neighbor_keys(ch);
        let result = if neighbors.is_empty() {
            let flat: Vec<char> = self.grid.iter().flatten().copied().collect();
            flat[(rng.next() * (flat.len() as f64)).floor() as usize]
        } else {
            neighbors[(rng.next() * (neighbors.len() as f64)).floor() as usize]
        };

        if was_upper {
            result.to_uppercase().next().unwrap_or(result)
        } else {
            result
        }
    }
}
