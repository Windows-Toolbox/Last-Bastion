//! TAB's post-processing (`ZXSystem_GameLevel.PostProcessingWithBloom`, the shader `ApplyBloomFullScreen` in
//! `Shaders.dxshader`): the map isn't drawn straight onto the screen but into a picture first. That picture then goes
//! onto the screen brighter (× 1.5), with a soft glow of its bright parts on top (bloom, × 1.8), and through the
//! landscape's colour filter (`ZXMapTheme.SetMapObjectStyle`: Caustic Lands turquoise, the moorland warm …).
//! Without it the landscapes look pale and grey next to TAB's. The panel at the bottom stays as it is, like in TAB.
//!
//! The glow: the picture shrunk to an eighth (in halves, so nothing flickers), smeared sideways and then up and down
//! (TAB: `Effect_BlurOnePass`, 9 steps to each side over 4 % of the picture), and only what is brighter than
//! `MIN_LUMINANCE` counts.

use macroquad::miniquad::{BlendFactor, BlendState, Equation, PipelineParams};
use macroquad::prelude::*;

/// TAB's numbers for the map (`PostProcessingWithBloom`).
const BLOOM_INTENSITY: f32 = 1.8;
const ORIGINAL_INTENSITY: f32 = 1.5;
/// The editor's views without the player's light (everything as if seen right now): TAB's brightening on top of that
/// glared – there the picture goes on only this much brighter (`finish`, `unlit`).
const UNLIT_INTENSITY: f32 = 1.1;
const MIN_LUMINANCE: f32 = 0.1;
/// How far the glow smears: 4 % of the picture's width (sideways) and height (up and down).
const BLOOM_WIDTH: f32 = 0.04;
/// Steps of the smear to each side (`NStepsBlurOnePass`).
const BLUR_STEPS: f32 = 9.0;

/// A landscape's colour filter – TAB's `ColorHue`, `Saturation`, `Contrast`, `Brightness` (`DXObjectState`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grade {
    /// Every channel times this (0..255, like TAB's `Color`).
    hue: [u8; 3],
    saturation: f32,
    contrast: f32,
    brightness: f32,
}

/// The filter of a landscape (`ZXMapTheme*.SetMapObjectStyle`). Before calling it TAB sets hue (255, 192, 128),
/// saturation 0.85, contrast 1 and brightness 0.97 – what a landscape doesn't set stays that.
pub fn grade(look: &str) -> Grade {
    let base = Grade { hue: [255, 192, 128], saturation: 0.85, contrast: 1.0, brightness: 0.97 };
    match look {
        "BR" => Grade { brightness: 0.96, ..base },
        "FA" => Grade { hue: [255, 200, 110], saturation: 1.0, contrast: 1.05, brightness: 0.98 },
        // `Color.Wheat`.
        "TM" => Grade { hue: [245, 222, 179], contrast: 1.1, ..base },
        // (160, 220, 255) with its lightness set to 0.85 (`SetBrightness`, in HSL).
        "AL" => Grade { hue: [179, 227, 255], saturation: 0.7, contrast: 1.3, ..base },
        "DS" => Grade { hue: [255, 220, 170], saturation: 1.0, contrast: 1.1, brightness: 0.98 },
        "VO" => Grade { hue: [92, 255, 255], saturation: 1.0, contrast: 1.02, brightness: 0.97 },
        _ => base,
    }
}

impl Grade {
    /// TAB's colour matrix (`DXObjectState.UpdateColorMatrix`), number for number: row `i` says how much channel `i`
    /// (red, green, blue, alpha) goes into each channel of the result. The alpha row is the brightness: it's added.
    fn matrix(self) -> [[f32; 4]; 4] {
        let [r, g, b] = self.hue.map(|c| c as f32 / 255.0);
        let (c, s, n) = (self.contrast, self.saturation, self.brightness - 1.0);
        // Less saturation: every channel takes some of the others (by how bright the eye sees them).
        let (gr, gg, gb) = if s != 1.0 { (0.3086 * (1.0 - s), 0.6094 * (1.0 - s), 0.082 * (1.0 - s)) } else { (0.0, 0.0, 0.0) };
        [
            [c * r * (gr + s), c * r * gr, c * r * gr, 0.0],
            [c * g * gg, c * g * (gg + s), c * g * gg, 0.0],
            [b * c * gb, b * c * gb, b * c * (gb + s), 0.0],
            [
                c * r * n * (gr + s) + b * c * n * gb + c * g * n * gg,
                c * g * n * (gg + s) + b * c * n * gb + c * r * n * gr,
                b * c * n * (gb + s) + c * g * n * gg + c * r * n * gr,
                1.0,
            ],
        ]
    }
}

/// The pictures in between and the two shaders. Made when first needed; `None`/`broken` if the graphics card can't.
#[derive(Default)]
pub struct Post {
    /// The map, as big as the window (in real pixels).
    scene: Option<RenderTarget>,
    /// Half, quarter, eighth of it – the last one ends up holding the glow.
    smaller: Vec<RenderTarget>,
    /// The eighth smeared sideways.
    across: Option<RenderTarget>,
    blur: Option<Material>,
    finish: Option<Material>,
    /// A shader didn't compile: draw straight onto the screen, as before.
    broken: bool,
}

impl Post {
    /// From here on everything is drawn into the picture instead of onto the screen – `false` if that can't be done
    /// (then it's drawn onto the screen as usual, and `finish` does nothing).
    pub fn begin(&mut self) -> bool {
        if self.broken || !self.ready() {
            return false;
        }
        let (w, h) = (screen_width(), screen_height());
        let scene = self.scene.clone().expect("made by ready");
        set_camera(&Camera2D {
            target: vec2(w / 2.0, h / 2.0),
            zoom: vec2(2.0 / w, 2.0 / h),
            render_target: Some(scene),
            ..Default::default()
        });
        clear_background(crate::draw::BACKGROUND);
        true
    }

    /// The picture onto the screen: shrunk, smeared, and then with its glow and the landscape's filter (`look`).
    /// `unlit`: the map was drawn without the player's light and dark (the editor's other views) – less brightening.
    pub fn finish(&mut self, look: &str, unlit: bool) {
        let (Some(scene), Some(across), Some(blur), Some(finish)) = (&self.scene, &self.across, &self.blur, &self.finish) else {
            return;
        };
        // Shrinking: each one into the next, half as big – smooth, because every pixel is the mean of four.
        let mut from = &scene.texture;
        for target in &self.smaller {
            copy(from, target, None);
            from = &target.texture;
        }
        let eighth = self.smaller.last().expect("three of them");
        // Smearing: sideways into `across`, up and down back into the eighth.
        blur.set_uniform("Step", vec2(BLOOM_WIDTH / BLUR_STEPS, 0.0));
        copy(&eighth.texture, across, Some(blur));
        blur.set_uniform("Step", vec2(0.0, BLOOM_WIDTH / BLUR_STEPS));
        copy(&across.texture, eighth, Some(blur));

        set_default_camera();
        let m = grade(look).matrix();
        // GLSL keeps a matrix by columns: column j = what goes into channel j.
        finish.set_uniform("Grade", Mat4::from_cols_array_2d(&std::array::from_fn(|j| std::array::from_fn(|i| m[i][j]))));
        let original = if unlit { UNLIT_INTENSITY } else { ORIGINAL_INTENSITY };
        finish.set_uniform("Glow", vec4(BLOOM_INTENSITY, original, MIN_LUMINANCE, 0.0));
        finish.set_texture("Bloom", eighth.texture.clone());
        gl_use_material(finish);
        draw_texture_ex(&scene.texture, 0.0, 0.0, WHITE, sized(vec2(screen_width(), screen_height())));
        gl_use_default_material();
    }

    /// Makes what's missing – the pictures anew when the window changed its size. `false`: no shaders.
    fn ready(&mut self) -> bool {
        if self.blur.is_none() {
            match (blur_material(), finish_material()) {
                (Ok(blur), Ok(finish)) => (self.blur, self.finish) = (Some(blur), Some(finish)),
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("post-processing off: {e}");
                    self.broken = true;
                    return false;
                }
            }
        }
        let scale = screen_dpi_scale();
        let (w, h) = (((screen_width() * scale) as u32).max(8), ((screen_height() * scale) as u32).max(8));
        if self.scene.as_ref().is_some_and(|s| s.texture.width() as u32 == w && s.texture.height() as u32 == h) {
            return true;
        }
        let target = |w: u32, h: u32| {
            let target = render_target(w.max(1), h.max(1));
            target.texture.set_filter(FilterMode::Linear);
            target
        };
        self.scene = Some(target(w, h));
        self.smaller = (1..=3).map(|k| target(w >> k, h >> k)).collect();
        self.across = Some(target(w >> 3, h >> 3));
        true
    }
}

/// Draws `from` over the whole of `to` – through `material` if given.
fn copy(from: &Texture2D, to: &RenderTarget, material: Option<&Material>) {
    // The unit square fills the target.
    set_camera(&Camera2D { target: vec2(0.5, 0.5), zoom: vec2(2.0, 2.0), render_target: Some(to.clone()), ..Default::default() });
    if let Some(material) = material {
        gl_use_material(material);
    }
    draw_texture_ex(from, 0.0, 0.0, WHITE, sized(Vec2::ONE));
    gl_use_default_material();
}

/// Drawn `size` big. (Not turned: a camera with a `render_target` already turns y the way the texture needs it.)
fn sized(size: Vec2) -> DrawTextureParams {
    DrawTextureParams { dest_size: Some(size), ..Default::default() }
}

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
varying highp vec2 uv;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    uv = texcoord;
}"#;

/// Overwrites what's there (the pictures are opaque anyway – no mixing with the old frame).
fn replace() -> PipelineParams {
    PipelineParams {
        color_blend: Some(BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::Zero)),
        alpha_blend: Some(BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::Zero)),
        ..Default::default()
    }
}

/// TAB's `ApplyBlurOnePass`: the pixel and 8 more to each side along `Step`, each 1.5 times weaker than the one
/// before, averaged.
fn blur_material() -> Result<Material, String> {
    let fragment = r#"#version 100
precision mediump float;
varying highp vec2 uv;
uniform sampler2D Texture;
uniform vec2 Step;
void main() {
    vec4 sum = texture2D(Texture, uv);
    float total = 1.0;
    float weight = 1.0;
    for (int i = 1; i < 9; i++) {
        weight /= 1.5;
        vec2 d = float(i) * Step;
        sum += weight * (texture2D(Texture, uv + d) + texture2D(Texture, uv - d));
        total += 2.0 * weight;
    }
    gl_FragColor = sum / total;
}"#;
    let params = MaterialParams {
        uniforms: vec![UniformDesc::new("Step", UniformType::Float2)],
        pipeline_params: replace(),
        ..Default::default()
    };
    load_material(ShaderSource::Glsl { vertex: VERTEX, fragment }, params).map_err(|e| format!("blur shader: {e}"))
}

/// TAB's `ApplyBloomFullScreen` and then its colour matrix (`mul(T, In.ColorMatrix)`): the picture × `Glow.y`, plus
/// the glow above `Glow.z` (stretched back to 0..1) × `Glow.x`, then through `Grade`. Alpha is part of it, like in
/// TAB: it carries the brightness row of the matrix – with an opaque picture TAB's alpha comes to `Glow.y + Glow.x`.
/// Ours isn't opaque everywhere (half see-through edges lower it), so it's set to that straight away.
fn finish_material() -> Result<Material, String> {
    let fragment = r#"#version 100
precision mediump float;
varying highp vec2 uv;
uniform sampler2D Texture;
uniform sampler2D Bloom;
uniform vec4 Glow;
uniform mat4 Grade;
void main() {
    vec3 scene = texture2D(Texture, uv).rgb;
    vec3 bloom = clamp((texture2D(Bloom, uv).rgb - Glow.z) / (1.0 - Glow.z), 0.0, 1.0);
    vec4 color = vec4(Glow.y * scene + Glow.x * bloom, Glow.y + Glow.x);
    color = color * Grade;
    gl_FragColor = vec4(clamp(color.rgb, 0.0, 1.0), 1.0);
}"#;
    let params = MaterialParams {
        uniforms: vec![UniformDesc::new("Glow", UniformType::Float4), UniformDesc::new("Grade", UniformType::Mat4)],
        textures: vec!["Bloom".into()],
        pipeline_params: replace(),
    };
    load_material(ShaderSource::Glsl { vertex: VERTEX, fragment }, params).map_err(|e| format!("post shader: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With TAB's neutral numbers the matrix leaves every colour as it is.
    #[test]
    fn a_neutral_grade_changes_nothing() {
        let neutral = Grade { hue: [255, 255, 255], saturation: 1.0, contrast: 1.0, brightness: 1.0 };
        let identity = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        assert_eq!(neutral.matrix(), identity);
    }

    /// Caustic Lands: red down to 92/255, green and blue kept – times the contrast, minus a bit of brightness.
    #[test]
    fn caustic_lands_turn_turquoise() {
        let m = grade("VO").matrix();
        assert!((m[0][0] - 1.02 * 92.0 / 255.0).abs() < 1e-6);
        assert!((m[1][1] - 1.02).abs() < 1e-6 && (m[2][2] - 1.02).abs() < 1e-6);
        assert!(m[3][0] < 0.0 && m[3][1] < 0.0, "brightness 0.97 takes a little off");
        assert_eq!(m[0][1], 0.0, "full saturation: no channel mixes into another");
    }
}
