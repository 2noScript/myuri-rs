//! Numeric primitives matching CPython `math` and NumPy evaluation order for Cursory.

/// `np.finfo(float).eps`.
pub const EPSILON: f64 = 2.220446049250313e-16;
const DBL_MIN: f64 = 2.2250738585072014e-308;
const SPLITTER: f64 = 134_217_729.0; // 2**27 + 1

/// `math.hypot(x, y)` matching CPython's `vector_norm`.
pub fn hypot(x: f64, y: f64) -> f64 {
    let first = x.abs();
    let second = y.abs();
    let found_nan = first.is_nan() || second.is_nan();
    let mut max = 0.0_f64;
    if first > max {
        max = first;
    }
    if second > max {
        max = second;
    }
    if max.is_infinite() {
        return f64::INFINITY;
    }
    if found_nan {
        return f64::NAN;
    }
    if max == 0.0 {
        return max;
    }
    let max_exponent = frexp_exponent(max);
    if max_exponent < -1023 {
        return DBL_MIN * hypot(first / DBL_MIN, second / DBL_MIN);
    }
    let scale = 2.0_f64.powi(-max_exponent);
    let mut compensated_sum = 1.0_f64;
    let mut product_residue = 0.0_f64;
    let mut sum_residue = 0.0_f64;
    for coordinate in [first, second] {
        let scaled = coordinate * scale;
        let square = scaled * scaled;
        let square_low = two_product_residue(scaled, scaled, square);
        let next_sum = compensated_sum + square;
        sum_residue += compensated_sum - next_sum + square;
        compensated_sum = next_sum;
        product_residue += square_low;
    }
    let mut root = (compensated_sum - 1.0 + (product_residue + sum_residue)).sqrt();
    let square = -root * root;
    let square_low = two_product_residue(-root, root, square);
    let next_sum = compensated_sum + square;
    sum_residue += compensated_sum - next_sum + square;
    compensated_sum = next_sum;
    product_residue += square_low;
    root += (compensated_sum - 1.0 + (product_residue + sum_residue)) / (2.0 * root);
    root / scale
}

#[inline]
fn two_product_residue(x: f64, y: f64, product: f64) -> f64 {
    let x_split = SPLITTER * x;
    let x_high = x_split - (x_split - x);
    let x_low = x - x_high;
    let y_split = SPLITTER * y;
    let y_high = y_split - (y_split - y);
    let y_low = y - y_high;
    x_high * y_high - product + x_high * y_low + x_low * y_high + x_low * y_low
}

#[inline]
fn frexp_exponent(value: f64) -> i32 {
    let bits = value.to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i32;
    if biased == 0 {
        let norm_bits = (value * 2.0_f64.powi(64)).to_bits();
        (((norm_bits >> 52) & 0x7ff) as i32 - 1022) - 64
    } else {
        biased - 1022
    }
}

/// `np.sum` pairwise summation over `values`.
pub fn sum(values: &[f64]) -> f64 {
    sum_slice(values, 0, values.len())
}

fn sum_slice(values: &[f64], start: usize, count: usize) -> f64 {
    if count == 0 {
        return 0.0;
    }
    if count < 8 {
        let mut total = -0.0_f64;
        for &v in &values[start..start + count] {
            total += v;
        }
        return total;
    }
    if count <= 128 {
        let mut acc = [
            values[start],
            values[start + 1],
            values[start + 2],
            values[start + 3],
            values[start + 4],
            values[start + 5],
            values[start + 6],
            values[start + 7],
        ];
        let mut i = 8;
        let limit = count - (count % 8);
        while i < limit {
            for (lane, acc_slot) in acc.iter_mut().enumerate() {
                *acc_slot += values[start + i + lane];
            }
            i += 8;
        }
        let mut total = acc[0] + acc[1] + (acc[2] + acc[3]) + (acc[4] + acc[5] + (acc[6] + acc[7]));
        while i < count {
            total += values[start + i];
            i += 1;
        }
        return total;
    }
    let mut half = count / 2;
    half -= half % 8;
    sum_slice(values, start, half) + sum_slice(values, start + half, count - half)
}

/// `np.searchsorted(sorted, value, side='right')`.
pub fn search_sorted_right(sorted: &[f64], value: f64) -> usize {
    let mut low = 0usize;
    let mut high = sorted.len();
    while low < high {
        let middle = (low + high) >> 1;
        if value < sorted[middle] {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

/// First `count` indices of `np.argsort(values)`, ascending, tie-broken by earliest index.
pub fn arg_sort_top_n(values: &[f64], count: usize) -> Vec<usize> {
    let mut best: Vec<usize> = Vec::with_capacity(count);
    for (index, &val) in values.iter().enumerate() {
        if best.len() == count
            && val.partial_cmp(&values[best[count - 1]]) != Some(std::cmp::Ordering::Less)
        {
            continue;
        }
        let mut position = best.len();
        while position > 0 && val < values[best[position - 1]] {
            position -= 1;
        }
        best.insert(position, index);
        if best.len() > count {
            best.pop();
        }
    }
    best
}

/// Python `round()` / NumPy `rint` (round-half-to-even, no negative zero).
pub fn rint(value: f64) -> f64 {
    let rounded = value.round_ties_even();
    if rounded == 0.0 {
        0.0
    } else {
        rounded
    }
}
