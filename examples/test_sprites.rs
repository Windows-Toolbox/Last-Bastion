//! Paints the test sheet for the sprite system (DESIGN.md 12.3): a simple grey figure in 16 directions, with an idle,
//! a walk and an attack animation – so turning, animating, the feet and the depth order can be checked in the game
//! before there is any real art.
//!
//! `cargo run --example test_sprites` writes `assets\test\figure.png`. What's where in it says `assets\test\figure.json`
//! – the numbers here and there have to match.
//!
//! It is a tiny "renderer" of its own: every body part is a 3D point projected exactly like the game's camera does it
//! (`iso.rs`), and then painted as a round-ended line. Real art comes from Blender later (`assets\README.md`).

use std::f32::consts::{SQRT_2, TAU};

use image::{Rgba, RgbaImage};
use macroquad::math::{Vec2, vec2};

/// One frame in pixels …
const W: u32 = 96;
const H: u32 = 128;
/// … and where the feet stand in it.
const FEET: Vec2 = vec2(48.0, 108.0);
/// This many pixels are one tile width while drawing the picture. The JSON's `pixels_per_tile` may be bigger – then the
/// figure is drawn that much smaller in the game (`figure.json`: 256, half the size).
const PPT: f32 = 128.0;
const DIRECTIONS: u32 = 16;
/// The animations side by side, each with its number of frames: idle, walk, attack.
const FRAMES: [u32; 3] = [4, 6, 4];

/// The outline of every part, and how thick it is (pixels).
const OUTLINE: [f32; 3] = [0.12, 0.12, 0.13];
const OUTLINE_WIDTH: f32 = 1.6;

fn main() {
    let columns: u32 = FRAMES.iter().sum();
    // Transparent pixels in the outline's colour: when the game draws the picture smaller, it mixes neighbouring
    // pixels – with white or black ones there, every figure would get a light or dark halo.
    let [r, g, b] = OUTLINE.map(|c| (c * 255.0) as u8);
    let mut image = RgbaImage::from_pixel(columns * W, DIRECTIONS * H, Rgba([r, g, b, 0]));

    for direction in 0..DIRECTIONS {
        let facing = direction as f32 / DIRECTIONS as f32 * TAU;
        let mut column = 0;
        for (animation, &frames) in FRAMES.iter().enumerate() {
            for i in 0..frames {
                let mut frame = Frame { image: &mut image, left: column * W, top: direction * H };
                let pose = Pose::new(animation, i as f32 / frames as f32);
                paint_figure(&mut frame, facing, pose);
                column += 1;
            }
        }
    }

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/test/figure.png");
    image.save(path).expect("couldn't write the picture");
    println!("Written: {path} ({} × {} pixels)", image.width(), image.height());
}

/// How the figure stands in one frame – worked out from the animation and how far into it the frame is.
struct Pose {
    /// Swing of the legs along where it faces (tiles), and how high each foot is lifted.
    stride: f32,
    lift: [f32; 2],
    /// The whole body a bit up or down (tile widths): stepping, breathing.
    bob: f32,
    /// Attack: how far the weapon is pushed forward (0..1), and a flash at its tip.
    thrust: f32,
    flash: bool,
}

impl Pose {
    /// `animation`: 0 idle, 1 walk, 2 attack (the order of `FRAMES`). `share`: how far into it (0..1).
    fn new(animation: usize, share: f32) -> Pose {
        let angle = share * TAU;
        let mut pose = Pose { stride: 0.0, lift: [0.0; 2], bob: 0.0, thrust: 0.0, flash: false };
        match animation {
            // Breathing: the upper body a little up and down.
            0 => pose.bob = angle.sin() * 0.006,
            // Two steps per loop: the legs swing, the foot going forward is lifted, the body is lowest with the legs
            // apart.
            1 => {
                pose.stride = angle.sin() * 0.09;
                pose.lift = [angle.cos().max(0.0) * 0.03, (-angle.cos()).max(0.0) * 0.03];
                pose.bob = (1.0 - angle.sin().abs()) * 0.015;
            }
            // A thrust (rifle recoil, a zombie's lunge): out at once, then slowly back.
            _ => {
                pose.thrust = 1.0 - share;
                pose.flash = share == 0.0;
            }
        }
        pose
    }
}

/// The figure facing `facing` (radians, 0 = +x like `Unit::facing`). Painted back to front, like the game sorts.
fn paint_figure(frame: &mut Frame, facing: f32, pose: Pose) {
    let forward = vec2(facing.cos(), facing.sin());
    let side = vec2(-forward.y, forward.x);
    // Does it face the camera (down on screen)? Then the face shows and the weapon is in front of the body.
    let toward_camera = forward.x + forward.y;
    let lean = forward * pose.thrust * 0.04;

    // Shadow and an arrow on the ground showing where it faces.
    frame.ellipse(project(Vec2::ZERO, 0.0), 0.2 * PPT / SQRT_2, 0.1 * PPT / SQRT_2, 0.35);
    let tip = forward * 0.34;
    for back in [forward * 0.25 + side * 0.07, forward * 0.25 - side * 0.07] {
        frame.capsule(project(back, 0.0), project(tip, 0.0), 1.6, 0.85, false);
    }

    // Legs: the one further back first.
    let hip = 0.2 + pose.bob;
    let mut legs = [(side * 0.05, forward * pose.stride, pose.lift[0]), (-side * 0.05, -forward * pose.stride, pose.lift[1])];
    legs.sort_by(|a, b| project(a.0 + a.1, 0.0).y.total_cmp(&project(b.0 + b.1, 0.0).y));
    for (offset, step, lift) in legs {
        frame.capsule(project(offset, hip), project(offset + step, lift), 4.0, 0.7, true);
    }

    let shoulder = side * -0.07;
    let weapon = |frame: &mut Frame| {
        let from = project(shoulder + lean, 0.36 + pose.bob);
        let reach = forward * (0.24 + pose.thrust * 0.08);
        let to = project(shoulder + lean + reach, 0.36 + pose.bob);
        frame.capsule(from, to, 2.6, 0.35, true);
        if pose.flash {
            frame.capsule(to, to, 5.0, 1.0, false);
        }
    };
    if toward_camera < 0.0 {
        weapon(frame);
    }
    // Body and head.
    frame.capsule(project(Vec2::ZERO, hip + 0.04), project(lean, 0.4 + pose.bob), 10.0, 0.95, true);
    let head = project(lean * 1.3, 0.52 + pose.bob);
    frame.capsule(head, head, 7.5, 0.95, true);
    if toward_camera > -0.3 {
        let visor = project(lean * 1.3 + forward * 0.045, 0.53 + pose.bob);
        frame.capsule(visor, visor, 2.4, 0.15, false);
    }
    if toward_camera >= 0.0 {
        weapon(frame);
    }
}

/// A point of the figure → pixel in its frame, exactly like `iso::iso`: `ground` in tiles from the feet (x and y like
/// on the map), `up` in tile widths.
fn project(ground: Vec2, up: f32) -> Vec2 {
    FEET + vec2((ground.x - ground.y) * PPT / 2.0, (ground.x + ground.y) * PPT / 4.0 - up * PPT)
}

/// One frame of the sheet to paint into.
struct Frame<'a> {
    image: &'a mut RgbaImage,
    left: u32,
    top: u32,
}

impl Frame<'_> {
    /// A line from `a` to `b` with round ends, `r` pixels to each side (`a == b`: a dot). `grey`: how light it is –
    /// lit from the left, darker to the right. `outline`: with a dark edge.
    fn capsule(&mut self, a: Vec2, b: Vec2, r: f32, grey: f32, outline: bool) {
        self.paint(a.min(b) - r - 1.0, a.max(b) + r + 1.0, |p| {
            let along = b - a;
            let t = if along.length_squared() < 1e-6 { 0.0 } else { ((p - a).dot(along) / along.length_squared()).clamp(0.0, 1.0) };
            let closest = a + along * t;
            let distance = p.distance(closest);
            let light = grey * (0.8 - 0.2 * ((p.x - closest.x) / r).clamp(-1.0, 1.0));
            let edge = if outline { (distance - (r - OUTLINE_WIDTH) + 0.5).clamp(0.0, 1.0) } else { 0.0 };
            let color = [0, 1, 2].map(|k| light + (OUTLINE[k] - light) * edge);
            (color, (r - distance + 0.5).clamp(0.0, 1.0))
        });
    }

    /// A flat black ellipse (the shadow), `alpha` see-through.
    fn ellipse(&mut self, center: Vec2, rx: f32, ry: f32, alpha: f32) {
        let half = vec2(rx, ry);
        self.paint(center - half - 1.0, center + half + 1.0, |p| {
            let distance = ((p - center) / half).length() - 1.0;
            ([0.0; 3], (0.5 - distance * ry).clamp(0.0, 1.0) * alpha)
        });
    }

    /// Every pixel from `min` to `max` (within the frame): `shape` says its colour and how much of it is covered (0..1),
    /// and that is laid over what is already there – like paint ("over").
    fn paint(&mut self, min: Vec2, max: Vec2, shape: impl Fn(Vec2) -> ([f32; 3], f32)) {
        let (x0, y0) = (min.x.max(0.0) as u32, min.y.max(0.0) as u32);
        let (x1, y1) = ((max.x.max(0.0) as u32).min(W - 1), (max.y.max(0.0) as u32).min(H - 1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (color, cover) = shape(vec2(x as f32 + 0.5, y as f32 + 0.5));
                if cover <= 0.0 {
                    continue;
                }
                let pixel = self.image.get_pixel_mut(self.left + x, self.top + y);
                let below = pixel.0.map(|c| c as f32 / 255.0);
                let alpha = cover + below[3] * (1.0 - cover);
                let mix = |k: usize| (color[k] * cover + below[k] * below[3] * (1.0 - cover)) / alpha;
                *pixel = Rgba([mix(0), mix(1), mix(2), alpha].map(|c| (c * 255.0).round() as u8));
            }
        }
    }
}
