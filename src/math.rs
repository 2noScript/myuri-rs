use serde::{Deserialize, Serialize};

use crate::rng::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vector {
    pub x: f64,
    pub y: f64,
}

impl Vector {
    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimedVector {
    pub x: f64,
    pub y: f64,
    pub t: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewportSize {
    pub width: f64,
    pub height: f64,
}

#[inline]
pub fn sub(a: Vector, b: Vector) -> Vector {
    Vector {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

#[inline]
pub fn add(a: Vector, b: Vector) -> Vector {
    Vector {
        x: a.x + b.x,
        y: a.y + b.y,
    }
}

#[inline]
pub fn direction(a: Vector, b: Vector) -> Vector {
    sub(b, a)
}

#[inline]
pub fn magnitude(a: Vector) -> f64 {
    a.x.hypot(a.y)
}

#[inline]
pub fn point_in_box(p: Vector, b: BoundingBox) -> bool {
    p.x >= b.x && p.x <= b.x + b.width && p.y >= b.y && p.y <= b.y + b.height
}

/// A point drawn uniformly from a disc of `radius` around `coordinate`.
pub fn overshoot(coordinate: Vector, radius: f64, rng: &Rng) -> Vector {
    let angle = rng.next() * 2.0 * std::f64::consts::PI;
    let rad = radius * rng.next().sqrt();
    add(
        coordinate,
        Vector {
            x: rad * angle.cos(),
            y: rad * angle.sin(),
        },
    )
}
