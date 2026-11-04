//! Draws the step markers: a persimmon disc, edged in white, with its number in
//! Kata's own rounded digits.

use Stroke::{Arc, Line};

use crate::{Image, Rect};

/// The diameter the edge, the shadow and the digits are laid out for: a usual marker
/// at 100 %.
pub(crate) const STEP_SIZE: f32 = 28.0;
/// Kata's persimmon orange, as the image stores it: blue, green, red.
const ORANGE: [f32; 3] = [34.0, 90.0, 236.0];
/// The white edge around the disc, on a marker `STEP_SIZE` wide.
const EDGE: f32 = 2.0;
/// The shadow under the marker: its reach and drop on a marker `STEP_SIZE` wide, and
/// its darkness.
const SHADOW_REACH: f32 = 4.0;
const SHADOW_DROP: f32 = 1.0;
const SHADOW_DARKNESS: f32 = 0.3;
/// The digits' height and the width of their lines, as parts of the diameter.
const DIGIT_HEIGHT: f32 = 0.5;
const DIGIT_LINE: f32 = 0.1;
/// Between two digits, and the most a number spans, as parts of the diameter.
const DIGIT_GAP: f32 = 0.06;
const NUMBER_WIDTH: f32 = 0.66;

/// Everything a marker in `rect` changes: its disc and shadow.
pub fn footprint(rect: Rect) -> Rect {
    rect.inflate(((SHADOW_REACH + SHADOW_DROP) * scale(rect)).ceil() as i32 + 1)
}

/// Draws the marker of step `number` in `rect`, over what `image` shows, only inside
/// `area`.
pub fn draw(image: &mut Image, rect: Rect, number: usize, area: Rect) {
    let Some(area) = footprint(rect).intersect(&area) else {
        return;
    };
    let center = (
        (rect.left + rect.right) as f32 / 2.0,
        (rect.top + rect.bottom) as f32 / 2.0,
    );
    let radius = rect.width().min(rect.height()) as f32 / 2.0;
    let scale = scale(rect);
    let edge = EDGE * scale;
    let (reach, drop) = (SHADOW_REACH * scale, SHADOW_DROP * scale);
    let number = Number::new(number, center, 2.0 * radius);
    for y in area.top..area.bottom {
        for x in area.left..area.right {
            let point = (x as f32 + 0.5, y as f32 + 0.5);
            let outside = distance(point, center) - radius;
            let shadow = (distance(point, (center.0, center.1 + drop)) - radius).max(0.0);
            let fade = (1.0 - shadow / reach).max(0.0);
            let keep = 1.0 - SHADOW_DARKNESS * fade * fade;
            let white = (0.5 - outside).clamp(0.0, 1.0);
            let orange = (0.5 - outside - edge).clamp(0.0, 1.0);
            let ink = if orange > 0.0 { number.ink(point) } else { 0.0 };

            let at = image.offset(x, y);
            let pixel = &mut image.pixels_mut()[at..at + 3];
            for (channel, value) in pixel.iter_mut().enumerate() {
                let mut color = f32::from(*value) * keep;
                color += (255.0 - color) * white;
                color += (ORANGE[channel] - color) * orange;
                color += (255.0 - color) * ink;
                *value = (color + 0.5) as u8;
            }
        }
    }
}

/// A number laid out in a marker.
struct Number {
    /// Each digit and where its box's top-left corner falls.
    digits: Vec<(&'static Digit, (f32, f32))>,
    /// Pixels per digit unit, and half the line's width in pixels.
    unit: f32,
    half_line: f32,
}

impl Number {
    /// `number` centered on `center`, in a marker `size` pixels wide.
    fn new(number: usize, center: (f32, f32), size: f32) -> Self {
        let digits: Vec<&Digit> = number
            .to_string()
            .bytes()
            .map(|digit| &DIGITS[usize::from(digit - b'0')])
            .collect();
        // The digits' lines run inside their height, and their ink a half-line beyond.
        let line = DIGIT_LINE * size;
        let gap = DIGIT_GAP * size;
        let ink = |unit: f32| {
            let boxes: f32 = digits.iter().map(|digit| digit.width * unit + line).sum();
            boxes + gap * (digits.len() - 1) as f32
        };
        let mut unit = DIGIT_HEIGHT * size - line;
        unit *= (NUMBER_WIDTH * size / ink(unit)).min(1.0);
        let height = unit + line;
        let mut left = center.0 - ink(unit) / 2.0;
        let top = center.1 - height / 2.0 + line / 2.0;
        let digits = digits
            .into_iter()
            .map(|digit| {
                let at = (left + line / 2.0, top);
                left += digit.width * unit + line + gap;
                (digit, at)
            })
            .collect();
        Self {
            digits,
            unit,
            half_line: line / 2.0,
        }
    }

    /// How much of the pixel around `point` the digits cover, from 0 to 1.
    fn ink(&self, point: (f32, f32)) -> f32 {
        let nearest = self
            .digits
            .iter()
            .flat_map(|(digit, at)| {
                let local = ((point.0 - at.0) / self.unit, (point.1 - at.1) / self.unit);
                digit
                    .strokes
                    .iter()
                    .map(move |stroke| stroke.distance(local))
            })
            .fold(f32::INFINITY, f32::min);
        (self.half_line - nearest * self.unit + 0.5).clamp(0.0, 1.0)
    }
}

/// A digit's lines, in a box 1 unit high and `width` wide, the top-left at 0, 0.
struct Digit {
    width: f32,
    strokes: &'static [Stroke],
}

/// Part of a digit: a line, or an arc along a circle from angle `from` to `to`, in
/// degrees clockwise from the right, as the image's rows run down.
enum Stroke {
    Line((f32, f32), (f32, f32)),
    Arc {
        center: (f32, f32),
        radius: f32,
        from: f32,
        to: f32,
    },
}

impl Stroke {
    /// How far `point` lies from the stroke's middle.
    fn distance(&self, point: (f32, f32)) -> f32 {
        match *self {
            Line(a, b) => {
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let along = ((point.0 - a.0) * dx + (point.1 - a.1) * dy) / (dx * dx + dy * dy);
                let t = along.clamp(0.0, 1.0);
                distance(point, (a.0 + t * dx, a.1 + t * dy))
            }
            Arc {
                center,
                radius,
                from,
                to,
            } => {
                let (dx, dy) = (point.0 - center.0, point.1 - center.1);
                let angle = dy.atan2(dx).to_degrees();
                if (angle - from).rem_euclid(360.0) <= to - from {
                    (dx.hypot(dy) - radius).abs()
                } else {
                    let end = |degrees: f32| {
                        let (sin, cos) = degrees.to_radians().sin_cos();
                        (center.0 + radius * cos, center.1 + radius * sin)
                    };
                    distance(point, end(from)).min(distance(point, end(to)))
                }
            }
        }
    }
}

/// The digits 0 to 9: geometric, with round ends, all as tall.
const DIGITS: [Digit; 10] = [
    Digit {
        width: 0.56,
        strokes: &[
            Arc {
                center: (0.28, 0.28),
                radius: 0.28,
                from: 180.0,
                to: 360.0,
            },
            Line((0.0, 0.28), (0.0, 0.72)),
            Line((0.56, 0.28), (0.56, 0.72)),
            Arc {
                center: (0.28, 0.72),
                radius: 0.28,
                from: 0.0,
                to: 180.0,
            },
        ],
    },
    Digit {
        width: 0.3,
        strokes: &[Line((0.3, 0.0), (0.3, 1.0)), Line((0.3, 0.0), (0.0, 0.22))],
    },
    Digit {
        width: 0.56,
        strokes: &[
            Arc {
                center: (0.28, 0.28),
                radius: 0.28,
                from: 195.0,
                to: 400.0,
            },
            Line((0.4945, 0.46), (0.0, 1.0)),
            Line((0.0, 1.0), (0.56, 1.0)),
        ],
    },
    Digit {
        width: 0.54,
        strokes: &[
            Arc {
                center: (0.27, 0.24),
                radius: 0.24,
                from: 200.0,
                to: 450.0,
            },
            Line((0.16, 0.48), (0.27, 0.48)),
            Arc {
                center: (0.27, 0.74),
                radius: 0.26,
                from: -90.0,
                to: 160.0,
            },
        ],
    },
    Digit {
        width: 0.6,
        strokes: &[
            Line((0.44, 0.0), (0.44, 1.0)),
            Line((0.44, 0.0), (0.0, 0.68)),
            Line((0.0, 0.68), (0.6, 0.68)),
        ],
    },
    Digit {
        width: 0.56,
        strokes: &[
            Line((0.52, 0.0), (0.1, 0.0)),
            Line((0.1, 0.0), (0.084, 0.488)),
            Arc {
                center: (0.27, 0.71),
                radius: 0.29,
                from: 230.0,
                to: 510.0,
            },
        ],
    },
    Digit {
        width: 0.56,
        strokes: &[
            Line((0.42, 0.0), (0.04, 0.6)),
            Arc {
                center: (0.28, 0.72),
                radius: 0.28,
                from: 0.0,
                to: 360.0,
            },
        ],
    },
    Digit {
        width: 0.56,
        strokes: &[
            Line((0.0, 0.0), (0.56, 0.0)),
            Line((0.56, 0.0), (0.18, 1.0)),
        ],
    },
    Digit {
        width: 0.57,
        strokes: &[
            Arc {
                center: (0.285, 0.225),
                radius: 0.225,
                from: 0.0,
                to: 360.0,
            },
            Arc {
                center: (0.285, 0.715),
                radius: 0.285,
                from: 0.0,
                to: 360.0,
            },
        ],
    },
    Digit {
        width: 0.56,
        strokes: &[
            Arc {
                center: (0.28, 0.28),
                radius: 0.28,
                from: 0.0,
                to: 360.0,
            },
            Line((0.52, 0.4), (0.14, 1.0)),
        ],
    },
];

/// How much bigger than a usual marker at 100 % the one in `rect` is.
fn scale(rect: Rect) -> f32 {
    rect.width().min(rect.height()) as f32 / STEP_SIZE
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}
