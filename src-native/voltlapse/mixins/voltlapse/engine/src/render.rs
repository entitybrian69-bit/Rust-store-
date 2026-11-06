// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! Offline rendering: settings, the job queue, and the post-processing maths.
//!
//! Rendering a replay is the same camera path evaluated at frame times instead
//! of tick times, with every frame captured and handed to an encoder. The engine
//! owns everything that can be decided without a GPU — which frames to produce,
//! where the camera is in each, what the post-processing does to a pixel — and
//! leaves the drawing to the game and the encoding to ffmpeg (or to a PNG
//! sequence when no encoder is installed).

use crate::json::Json;

/// Container the render is written into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VideoFormat {
    #[default]
    Mp4,
    Avi,
    WebM,
    /// One PNG per frame, for a compositor.
    PngSequence,
}

impl VideoFormat {
    pub fn id(self) -> &'static str {
        match self {
            VideoFormat::Mp4 => "mp4",
            VideoFormat::Avi => "avi",
            VideoFormat::WebM => "webm",
            VideoFormat::PngSequence => "png",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id.trim().to_ascii_lowercase().as_str() {
            "mp4" | "h264" | "h265" => VideoFormat::Mp4,
            "avi" => VideoFormat::Avi,
            "webm" | "vp9" | "av1" => VideoFormat::WebM,
            "png" | "png_sequence" | "frames" | "images" => VideoFormat::PngSequence,
            _ => return None,
        })
    }

    /// The encoder ffmpeg uses, unless the user overrode it.
    pub fn default_codec(self) -> &'static str {
        match self {
            VideoFormat::Mp4 => "libx264",
            VideoFormat::Avi => "mpeg4",
            VideoFormat::WebM => "libvpx-vp9",
            VideoFormat::PngSequence => "png",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            VideoFormat::Mp4 => "mp4",
            VideoFormat::Avi => "avi",
            VideoFormat::WebM => "webm",
            VideoFormat::PngSequence => "png",
        }
    }
}

/// Anti-aliasing for a render, independent of the game's setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AntiAliasing {
    #[default]
    None,
    /// Multisample with the given sample count (2, 4, 8, 16).
    Msaa(u8),
    /// Fast approximate AA, a post pass.
    Fxaa,
    /// Temporal AA, needs jittered frames and history.
    Taa,
}

impl AntiAliasing {
    pub fn id(self) -> String {
        match self {
            AntiAliasing::None => "none".into(),
            AntiAliasing::Msaa(samples) => format!("msaa{samples}"),
            AntiAliasing::Fxaa => "fxaa".into(),
            AntiAliasing::Taa => "taa".into(),
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        let id = id.trim().to_ascii_lowercase();
        Some(match id.as_str() {
            "none" | "off" => AntiAliasing::None,
            "fxaa" => AntiAliasing::Fxaa,
            "taa" => AntiAliasing::Taa,
            other if other.starts_with("msaa") => {
                let samples: u8 = other[4..].parse().unwrap_or(4);
                // The GPU only knows these counts; snap to the nearest.
                let samples = match samples {
                    0..=2 => 2,
                    3..=4 => 4,
                    5..=8 => 8,
                    _ => 16,
                };
                AntiAliasing::Msaa(samples)
            }
            _ => return None,
        })
    }

    /// Extra sub-frames this needs per output frame (MSAA is free at capture
    /// time, TAA needs the history, motion blur is handled separately).
    pub fn sub_frames(self) -> u32 {
        match self {
            AntiAliasing::Taa => 2,
            _ => 1,
        }
    }
}

/// Motion blur: how many sub-frames and how long the shutter stays open.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionBlur {
    pub enabled: bool,
    /// Sub-frames blended into one output frame.
    pub samples: u8,
    /// 0..=1 share of a frame's time the shutter is open.
    pub intensity: f32,
}

impl Default for MotionBlur {
    fn default() -> Self {
        Self { enabled: false, samples: 8, intensity: 0.5 }
    }
}

/// Depth of field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthOfField {
    pub enabled: bool,
    /// Distance in blocks that stays sharp.
    pub focal_distance: f32,
    /// 0..=1; larger is a shallower depth of field.
    pub aperture: f32,
    /// Blur strength ceiling in pixels (of a 1080p frame).
    pub max_blur: f32,
}

impl Default for DepthOfField {
    fn default() -> Self {
        Self { enabled: false, focal_distance: 12.0, aperture: 0.35, max_blur: 8.0 }
    }
}

/// Tilt-shift: a sharp band with blur above and below it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TiltShift {
    pub enabled: bool,
    /// Centre of the sharp band, 0..=1 down the frame.
    pub center: f32,
    /// Height of the sharp band, 0..=1.
    pub band: f32,
    pub strength: f32,
}

impl Default for TiltShift {
    fn default() -> Self {
        Self { enabled: false, center: 0.5, band: 0.25, strength: 0.6 }
    }
}

/// Colour grading via a `.cube` LUT plus a vignette.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorGrade {
    /// Path to a `.cube` file, or empty for none.
    pub lut: String,
    /// 0..=1 blend between the ungraded and graded image.
    pub lut_strength: f32,
    /// 0 disables the vignette.
    pub vignette: f32,
    /// Vignette roundness: 0..=1 (1 is circular).
    pub vignette_softness: f32,
    pub saturation: f32,
    pub contrast: f32,
    pub brightness: f32,
}

impl Default for ColorGrade {
    fn default() -> Self {
        Self {
            lut: String::new(),
            lut_strength: 1.0,
            vignette: 0.0,
            vignette_softness: 0.5,
            saturation: 1.0,
            contrast: 1.0,
            brightness: 1.0,
        }
    }
}

/// How a frame is projected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Projection {
    /// Normal perspective camera.
    #[default]
    Perspective,
    /// Equirectangular (2:1) for 360°/VR video.
    Equirectangular,
}

impl Projection {
    pub fn id(self) -> &'static str {
        match self {
            Projection::Perspective => "perspective",
            Projection::Equirectangular => "equirectangular",
        }
    }
}

/// Everything a render needs to know before it starts.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub format: VideoFormat,
    /// Override the encoder; empty uses [`VideoFormat::default_codec`].
    pub codec: String,
    /// Bits per second; 0 lets the encoder decide from the resolution.
    pub bitrate: u32,
    pub anti_aliasing: AntiAliasing,
    pub motion_blur: MotionBlur,
    pub depth_of_field: DepthOfField,
    pub tilt_shift: TiltShift,
    pub grade: ColorGrade,
    /// Aspect ratio for letterboxing (0 disables).
    pub letterbox: f32,
    /// Keep the alpha channel (a transparent background, for compositing).
    pub alpha: bool,
    pub projection: Projection,
    /// Render the world in two passes (opaque, then translucent and particles)
    /// and blend, for a cleaner depth of field and motion blur.
    pub multi_pass: bool,
    /// Chunks drawn by the replay renderer (0 keeps the game's setting).
    pub render_distance: u8,
    /// Where the finished file goes (a folder for a PNG sequence).
    pub output: String,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60,
            format: VideoFormat::Mp4,
            codec: String::new(),
            bitrate: 0,
            anti_aliasing: AntiAliasing::Msaa(4),
            motion_blur: MotionBlur::default(),
            depth_of_field: DepthOfField::default(),
            tilt_shift: TiltShift::default(),
            grade: ColorGrade::default(),
            letterbox: 0.0,
            alpha: false,
            projection: Projection::Perspective,
            multi_pass: false,
            render_distance: 12,
            output: "voltlapse-render.mp4".into(),
        }
    }
}

/// The frame rates a render may use.
pub const FRAME_RATES: [u32; 7] = [24, 30, 60, 120, 144, 240, 480];

impl RenderSettings {
    /// Clamp to what the renderer supports, and fix impossible combinations.
    pub fn sanitized(mut self) -> Self {
        // 720p through 8K, keeping the aspect the user asked for.
        self.width = self.width.clamp(640, 7680).next_multiple_of(2);
        self.height = self.height.clamp(360, 4320).next_multiple_of(2);
        if !FRAME_RATES.contains(&self.fps) {
            self.fps = FRAME_RATES
                .iter()
                .copied()
                .min_by_key(|fps| fps.abs_diff(self.fps))
                .unwrap_or(60);
        }
        self.motion_blur.samples = self.motion_blur.samples.clamp(2, 32);
        self.motion_blur.intensity = self.motion_blur.intensity.clamp(0.0, 1.0);
        self.depth_of_field.focal_distance = self.depth_of_field.focal_distance.clamp(0.1, 512.0);
        self.depth_of_field.aperture = self.depth_of_field.aperture.clamp(0.0, 1.0);
        self.depth_of_field.max_blur = self.depth_of_field.max_blur.clamp(0.0, 64.0);
        self.tilt_shift.center = self.tilt_shift.center.clamp(0.0, 1.0);
        self.tilt_shift.band = self.tilt_shift.band.clamp(0.0, 1.0);
        self.tilt_shift.strength = self.tilt_shift.strength.clamp(0.0, 1.0);
        self.grade.vignette = self.grade.vignette.clamp(0.0, 1.0);
        self.grade.lut_strength = self.grade.lut_strength.clamp(0.0, 1.0);
        self.grade.saturation = self.grade.saturation.clamp(0.0, 4.0);
        self.grade.contrast = self.grade.contrast.clamp(0.0, 4.0);
        self.grade.brightness = self.grade.brightness.clamp(0.0, 4.0);
        if self.letterbox > 0.0 {
            self.letterbox = self.letterbox.clamp(1.0, 4.0);
        } else {
            self.letterbox = 0.0;
        }
        if self.projection == Projection::Equirectangular {
            // A 360 frame is 2:1; the resolution is forced and the letterbox is
            // meaningless on a full sphere.
            self.height = (self.width / 2).max(360);
            self.letterbox = 0.0;
            self.alpha = false;
        }
        if self.codec.is_empty() {
            self.codec = self.format.default_codec().to_owned();
        }
        self
    }

    pub fn aspect(&self) -> f32 {
        if self.height == 0 {
            1.0
        } else {
            self.width as f32 / self.height as f32
        }
    }

    /// Sub-frames captured per output frame, from every feature that needs more
    /// than one.
    pub fn sub_frames(&self) -> u32 {
        let blur = if self.motion_blur.enabled { self.motion_blur.samples as u32 } else { 1 };
        let aa = self.anti_aliasing.sub_frames();
        let passes = if self.multi_pass { 2 } else { 1 };
        blur.max(aa) * passes
    }

    /// How many frames a recording of `ticks` produces.
    pub fn frame_count(&self, ticks: u64, tick_rate: u16) -> u64 {
        let tick_rate = tick_rate.max(1) as u64;
        // Round rather than truncate: a 1 tick recording at 60 fps is 3 frames.
        (ticks.saturating_mul(self.fps as u64) + tick_rate / 2) / tick_rate
    }

    /// The tick a frame is rendered at — fractional, so 60 fps over 20 tps
    /// gives three sub-tick camera positions per tick.
    pub fn frame_tick(&self, frame: u64, tick_rate: u16) -> f64 {
        frame as f64 * tick_rate.max(1) as f64 / self.fps.max(1) as f64
    }

    /// Estimated output size in bytes (a rough bitrate model, for the ETA and
    /// the disk-space check).
    pub fn estimated_bytes(&self, seconds: f64) -> u64 {
        let bits_per_second = if self.bitrate > 0 {
            self.bitrate as f64
        } else {
            // ~0.11 bits per pixel per frame, which tracks x264's default CRF on
            // gameplay footage closely enough for a progress estimate.
            self.width as f64 * self.height as f64 * self.fps as f64 * 0.11
        };
        (bits_per_second * seconds / 8.0) as u64
    }

    /// The post-processing filter chain, as ffmpeg's `-vf` argument.
    ///
    /// Every effect the render settings hold is applied here, after the frames
    /// leave the game: colour grading (exposure, contrast, saturation, a `.cube`
    /// LUT and the vignette), depth of field and tilt-shift as Gaussian blurs of
    /// the strength the settings ask for, motion blur as a temporal mix of the
    /// shutter's share of the frames, letterboxing to the target aspect, and the
    /// equirectangular stretch for a 360° export. An empty string means "no
    /// filter", so the encoder is handed the frames untouched.
    pub fn filter_chain(&self) -> String {
        let mut filters: Vec<String> = Vec::new();
        let grade = &self.grade;
        if (grade.brightness - 1.0).abs() > 1e-4
            || (grade.contrast - 1.0).abs() > 1e-4
            || (grade.saturation - 1.0).abs() > 1e-4
        {
            filters.push(format!(
                "eq=brightness={:.3}:contrast={:.3}:saturation={:.3}",
                (grade.brightness - 1.0).clamp(-1.0, 1.0),
                grade.contrast.clamp(0.0, 4.0),
                grade.saturation.clamp(0.0, 4.0)
            ));
        }
        if !grade.lut.trim().is_empty() {
            // ffmpeg wants forward slashes and escapes the colon in a path.
            let path = grade.lut.replace('\\', "/").replace(':', "\\:");
            filters.push(format!("lut3d=file='{path}':interp=tetrahedral"));
        }
        if grade.vignette > 0.0 {
            // `angle` is PI/strength: a larger strength darkens the corners more.
            let angle = std::f32::consts::PI / (0.5 + grade.vignette.clamp(0.0, 1.0) * 3.5);
            filters.push(format!("vignette=angle={angle:.4}"));
        }
        if self.depth_of_field.enabled {
            let sigma = (self.depth_of_field.max_blur * self.depth_of_field.aperture.max(0.05)).max(0.5);
            filters.push(format!("gblur=sigma={sigma:.2}:steps=2"));
        }
        if self.tilt_shift.enabled {
            let sigma = (self.tilt_shift.strength.clamp(0.0, 1.0) * 6.0).max(0.5);
            filters.push(format!("gblur=sigma={sigma:.2}:steps=2"));
        }
        if self.motion_blur.enabled && self.motion_blur.samples > 1 {
            let samples = self.motion_blur.samples.clamp(2, 32) as u32;
            filters.push(format!("tmix=frames={samples}:weights='1'"));
        }
        if self.letterbox > 0.01 {
            // Pad the frame to the target aspect with black bars.
            let ar = self.letterbox.clamp(0.1, 4.0);
            filters.push(format!("pad=ceil(ih*{ar:.4}/2)*2:ih:(ow-iw)/2:0:black"));
        }
        if self.projection == Projection::Equirectangular {
            // Stretch the 2:1 frame into an equirectangular panorama.
            filters.push("v360=input=flat:output=equirect:interp=LINEAR".into());
        }
        filters.join(",")
    }

    /// The ffmpeg argument list for turning a PNG sequence into the output.
    pub fn ffmpeg_args(&self, frame_pattern: &str) -> Vec<String> {
        let mut args: Vec<String> = vec!["-y".into()];
        if self.format == VideoFormat::PngSequence {
            // Nothing to encode; the caller keeps the frames.
            return args;
        }
        args.extend(["-framerate".into(), self.fps.to_string()]);
        args.extend(["-i".into(), frame_pattern.to_owned()]);
        let filters = self.filter_chain();
        if !filters.is_empty() {
            args.extend(["-vf".into(), filters]);
        }
        if self.alpha && self.format == VideoFormat::Mp4 {
            // A transparent MP4 needs a codec that carries alpha.
            args.extend(["-c:v".into(), "prores_ks".into(), "-pix_fmt".into(), "yuva444p10le".into()]);
        } else if self.alpha && self.format == VideoFormat::WebM {
            args.extend(["-c:v".into(), "libvpx-vp9".into(), "-pix_fmt".into(), "yuva420p".into()]);
        } else {
            args.extend(["-c:v".into(), self.codec.clone()]);
            if self.bitrate > 0 {
                args.extend(["-b:v".into(), self.bitrate.to_string()]);
            } else {
                args.extend(["-crf".into(), "18".into()]);
            }
        }
        if self.projection == Projection::Equirectangular {
            // Metadata so a player goes straight into 360 mode.
            args.extend(["-metadata".into(), "spatial-media:stereo_mode=mono".into()]);
        }
        args.push(self.output.clone());
        args
    }

    /// The ffmpeg argument list for one still.
    pub fn screenshot_args(&self, input: &str) -> Vec<String> {
        vec!["-y".into(), "-i".into(), input.to_owned(), self.output.clone()]
    }

    pub fn to_json(&self) -> Json {
        Json::Object(vec![
            ("width".into(), Json::int(self.width as i64)),
            ("height".into(), Json::int(self.height as i64)),
            ("fps".into(), Json::int(self.fps as i64)),
            ("format".into(), Json::String(self.format.id().into())),
            ("codec".into(), Json::String(self.codec.clone())),
            ("bitrate".into(), Json::int(self.bitrate as i64)),
            ("anti_aliasing".into(), Json::String(self.anti_aliasing.id())),
            (
                "motion_blur".into(),
                Json::Object(vec![
                    ("enabled".into(), Json::Bool(self.motion_blur.enabled)),
                    ("samples".into(), Json::int(self.motion_blur.samples as i64)),
                    ("intensity".into(), Json::Number(self.motion_blur.intensity as f64)),
                ]),
            ),
            (
                "depth_of_field".into(),
                Json::Object(vec![
                    ("enabled".into(), Json::Bool(self.depth_of_field.enabled)),
                    ("focal_distance".into(), Json::Number(self.depth_of_field.focal_distance as f64)),
                    ("aperture".into(), Json::Number(self.depth_of_field.aperture as f64)),
                    ("max_blur".into(), Json::Number(self.depth_of_field.max_blur as f64)),
                ]),
            ),
            (
                "tilt_shift".into(),
                Json::Object(vec![
                    ("enabled".into(), Json::Bool(self.tilt_shift.enabled)),
                    ("center".into(), Json::Number(self.tilt_shift.center as f64)),
                    ("band".into(), Json::Number(self.tilt_shift.band as f64)),
                    ("strength".into(), Json::Number(self.tilt_shift.strength as f64)),
                ]),
            ),
            (
                "grade".into(),
                Json::Object(vec![
                    ("lut".into(), Json::String(self.grade.lut.clone())),
                    ("lut_strength".into(), Json::Number(self.grade.lut_strength as f64)),
                    ("vignette".into(), Json::Number(self.grade.vignette as f64)),
                    ("vignette_softness".into(), Json::Number(self.grade.vignette_softness as f64)),
                    ("saturation".into(), Json::Number(self.grade.saturation as f64)),
                    ("contrast".into(), Json::Number(self.grade.contrast as f64)),
                    ("brightness".into(), Json::Number(self.grade.brightness as f64)),
                ]),
            ),
            ("letterbox".into(), Json::Number(self.letterbox as f64)),
            ("alpha".into(), Json::Bool(self.alpha)),
            ("projection".into(), Json::String(self.projection.id().into())),
            ("multi_pass".into(), Json::Bool(self.multi_pass)),
            ("render_distance".into(), Json::int(self.render_distance as i64)),
            ("output".into(), Json::String(self.output.clone())),
        ])
    }

    pub fn from_json(json: &Json) -> Self {
        let defaults = RenderSettings::default();
        let number = |path: &str, fallback: f64| json.path(path).and_then(Json::as_f64).unwrap_or(fallback);
        let flag = |path: &str, fallback: bool| json.path(path).and_then(Json::as_bool).unwrap_or(fallback);
        let mut settings = RenderSettings {
            width: number("width", defaults.width as f64) as u32,
            height: number("height", defaults.height as f64) as u32,
            fps: number("fps", defaults.fps as f64) as u32,
            format: json
                .get("format")
                .and_then(Json::as_str)
                .and_then(VideoFormat::from_id)
                .unwrap_or(defaults.format),
            codec: json.get("codec").and_then(Json::as_str).unwrap_or("").to_owned(),
            bitrate: number("bitrate", 0.0) as u32,
            anti_aliasing: json
                .get("anti_aliasing")
                .and_then(Json::as_str)
                .and_then(AntiAliasing::from_id)
                .unwrap_or(defaults.anti_aliasing),
            motion_blur: MotionBlur {
                enabled: flag("motion_blur/enabled", false),
                samples: number("motion_blur/samples", 8.0) as u8,
                intensity: number("motion_blur/intensity", 0.5) as f32,
            },
            depth_of_field: DepthOfField {
                enabled: flag("depth_of_field/enabled", false),
                focal_distance: number("depth_of_field/focal_distance", 12.0) as f32,
                aperture: number("depth_of_field/aperture", 0.35) as f32,
                max_blur: number("depth_of_field/max_blur", 8.0) as f32,
            },
            tilt_shift: TiltShift {
                enabled: flag("tilt_shift/enabled", false),
                center: number("tilt_shift/center", 0.5) as f32,
                band: number("tilt_shift/band", 0.25) as f32,
                strength: number("tilt_shift/strength", 0.6) as f32,
            },
            grade: ColorGrade {
                lut: json.path("grade/lut").and_then(Json::as_str).unwrap_or("").to_owned(),
                lut_strength: number("grade/lut_strength", 1.0) as f32,
                vignette: number("grade/vignette", 0.0) as f32,
                vignette_softness: number("grade/vignette_softness", 0.5) as f32,
                saturation: number("grade/saturation", 1.0) as f32,
                contrast: number("grade/contrast", 1.0) as f32,
                brightness: number("grade/brightness", 1.0) as f32,
            },
            letterbox: number("letterbox", 0.0) as f32,
            alpha: flag("alpha", false),
            projection: match json.get("projection").and_then(Json::as_str) {
                Some("equirectangular") => Projection::Equirectangular,
                _ => Projection::Perspective,
            },
            multi_pass: flag("multi_pass", false),
            render_distance: number("render_distance", 12.0) as u8,
            output: json.get("output").and_then(Json::as_str).unwrap_or("voltlapse-render.mp4").to_owned(),
        };
        if settings.render_distance == 0 {
            settings.render_distance = 12;
        }
        settings.sanitized()
    }
}

/// A queued render: a camera path over a range of a replay.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderJob {
    pub id: u64,
    pub name: String,
    /// The replay file this renders.
    pub replay: String,
    /// The camera path's JSON (empty renders the active path).
    pub camera: String,
    pub settings: RenderSettings,
    pub from_tick: u64,
    pub to_tick: u64,
    pub rendered_frames: u64,
    /// Frames finished from a previous run, resumed when the queue starts.
    pub total_frames: u64,
}

impl RenderJob {
    pub fn progress(&self) -> f64 {
        if self.total_frames == 0 {
            0.0
        } else {
            (self.rendered_frames as f64 / self.total_frames as f64).clamp(0.0, 1.0)
        }
    }

    pub fn is_done(&self) -> bool {
        self.total_frames > 0 && self.rendered_frames >= self.total_frames
    }
}

/// The batch queue: many camera paths, rendered one after another.
#[derive(Clone, Debug, Default)]
pub struct RenderQueue {
    jobs: Vec<RenderJob>,
    /// Index of the job being rendered.
    active: usize,
    next_id: u64,
    /// Seconds spent per frame so far, for the ETA (an exponential average).
    seconds_per_frame: f64,
}

impl RenderQueue {
    pub fn new() -> Self {
        Self { jobs: Vec::new(), active: 0, next_id: 1, seconds_per_frame: 0.0 }
    }

    /// Queue a job; returns its id.
    pub fn push(&mut self, mut job: RenderJob) -> u64 {
        job.id = self.next_id;
        self.next_id += 1;
        self.jobs.push(job);
        self.jobs.last().map(|j| j.id).unwrap_or(0)
    }

    pub fn jobs(&self) -> &[RenderJob] {
        &self.jobs
    }

    pub fn active(&self) -> Option<&RenderJob> {
        self.jobs.get(self.active)
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    /// Finish the current job and move to the next; returns the next job.
    pub fn advance(&mut self) -> Option<&RenderJob> {
        if self.active < self.jobs.len() {
            self.active += 1;
        }
        self.jobs.get(self.active)
    }

    /// Record one finished frame and update the ETA model.
    pub fn frame_done(&mut self, seconds: f64) {
        if let Some(job) = self.jobs.get_mut(self.active) {
            job.rendered_frames = (job.rendered_frames + 1).min(job.total_frames);
        }
        // Weighted average: the first frame sets the pace, later ones refine it.
        self.seconds_per_frame = if self.seconds_per_frame == 0.0 {
            seconds
        } else {
            self.seconds_per_frame * 0.9 + seconds * 0.1
        };
    }

    /// Seconds until the whole queue is done.
    pub fn eta_seconds(&self) -> f64 {
        let mut remaining = 0u64;
        for (index, job) in self.jobs.iter().enumerate() {
            if index < self.active || job.is_done() {
                continue;
            }
            remaining += job.total_frames.saturating_sub(job.rendered_frames);
        }
        remaining as f64 * self.seconds_per_frame
    }

    /// Whole-queue progress, 0..=1.
    pub fn progress(&self) -> f64 {
        let total: u64 = self.jobs.iter().map(|j| j.total_frames).sum();
        if total == 0 {
            return 0.0;
        }
        let done: u64 = self.jobs.iter().map(|j| j.rendered_frames.min(j.total_frames)).sum();
        (done as f64 / total as f64).clamp(0.0, 1.0)
    }

    pub fn is_finished(&self) -> bool {
        !self.jobs.is_empty() && self.jobs.iter().all(RenderJob::is_done)
    }

    pub fn clear(&mut self) {
        self.jobs.clear();
        self.active = 0;
        self.seconds_per_frame = 0.0;
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.jobs.len();
        self.jobs.retain(|job| job.id != id);
        if self.active >= self.jobs.len() {
            self.active = self.jobs.len().saturating_sub(1);
        }
        before != self.jobs.len()
    }
}

/// A parsed 3D colour LUT (`.cube`), applied per pixel.
#[derive(Clone, Debug, PartialEq)]
pub struct Lut3d {
    pub size: usize,
    /// RGB triples, blue-major as `.cube` stores them.
    pub data: Vec<[f32; 3]>,
    pub title: String,
}

impl Lut3d {
    /// Parse the text of a `.cube` file (the common 3D subset).
    pub fn parse(text: &str) -> Option<Self> {
        let mut size = 0usize;
        let mut data = Vec::new();
        let mut title = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            match parts.next()? {
                "TITLE" => title = line[5..].trim().trim_matches('"').to_owned(),
                "LUT_3D_SIZE" => size = parts.next()?.parse().ok()?,
                "DOMAIN_MIN" | "DOMAIN_MAX" => {}
                _ => {
                    let values: Vec<f32> = line.split_whitespace().filter_map(|v| v.parse().ok()).collect();
                    if values.len() == 3 {
                        data.push([values[0], values[1], values[2]]);
                    }
                }
            }
        }
        if size == 0 || data.len() != size * size * size {
            return None;
        }
        Some(Self { size, data, title })
    }

    /// Trilinear lookup of a 0..=1 colour.
    pub fn sample(&self, rgb: [f32; 3]) -> [f32; 3] {
        if self.size == 0 {
            return rgb;
        }
        let max = (self.size - 1) as f32;
        let scaled = [rgb[0].clamp(0.0, 1.0) * max, rgb[1].clamp(0.0, 1.0) * max, rgb[2].clamp(0.0, 1.0) * max];
        let base = [scaled[0].floor(), scaled[1].floor(), scaled[2].floor()];
        let frac = [scaled[0] - base[0], scaled[1] - base[1], scaled[2] - base[2]];
        let at = |r: usize, g: usize, b: usize| {
            let r = r.min(self.size - 1);
            let g = g.min(self.size - 1);
            let b = b.min(self.size - 1);
            self.data[(b * self.size + g) * self.size + r]
        };
        let (r0, g0, b0) = (base[0] as usize, base[1] as usize, base[2] as usize);
        let mut out = [0.0f32; 3];
        for channel in 0..3 {
            let c000 = at(r0, g0, b0)[channel];
            let c100 = at(r0 + 1, g0, b0)[channel];
            let c010 = at(r0, g0 + 1, b0)[channel];
            let c110 = at(r0 + 1, g0 + 1, b0)[channel];
            let c001 = at(r0, g0, b0 + 1)[channel];
            let c101 = at(r0 + 1, g0, b0 + 1)[channel];
            let c011 = at(r0, g0 + 1, b0 + 1)[channel];
            let c111 = at(r0 + 1, g0 + 1, b0 + 1)[channel];
            let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
            let c00 = lerp(c000, c100, frac[0]);
            let c10 = lerp(c010, c110, frac[0]);
            let c01 = lerp(c001, c101, frac[0]);
            let c11 = lerp(c011, c111, frac[0]);
            let c0 = lerp(c00, c10, frac[1]);
            let c1 = lerp(c01, c11, frac[1]);
            out[channel] = lerp(c0, c1, frac[2]);
        }
        out
    }
}

/// The vignette's darkening factor at a point (0..=1 across the frame):
/// multiplies the pixel's colour. 1 in the centre, less towards the edges.
pub fn vignette_factor(x: f32, y: f32, strength: f32, softness: f32) -> f32 {
    if strength <= 0.0 {
        return 1.0;
    }
    let dx = x * 2.0 - 1.0;
    let dy = y * 2.0 - 1.0;
    let distance = (dx * dx + dy * dy).sqrt() / std::f32::consts::SQRT_2;
    let softness = softness.clamp(0.01, 1.0);
    let falloff = ((distance - (1.0 - softness)) / softness).clamp(0.0, 1.0);
    (1.0 - falloff * strength).clamp(0.0, 1.0)
}

/// Tilt-shift blur strength at a point down the frame: 0 inside the sharp band,
/// rising towards the top and bottom edges.
pub fn tilt_shift_strength(y: f32, center: f32, band: f32, strength: f32) -> f32 {
    let distance = (y - center).abs();
    let band = band * 0.5;
    if distance <= band {
        return 0.0;
    }
    let ramp = ((distance - band) / band.max(1e-3)).clamp(0.0, 1.0);
    ramp * strength
}

/// Depth-of-field blur strength for a depth in blocks.
pub fn dof_strength(depth: f32, settings: DepthOfField) -> f32 {
    if !settings.enabled {
        return 0.0;
    }
    let distance = (depth - settings.focal_distance).abs();
    let coc = (distance / settings.focal_distance.max(0.1)) * settings.aperture;
    (coc * settings.max_blur).clamp(0.0, settings.max_blur)
}

/// Sub-frame offsets for motion blur across a frame's exposure.
pub fn motion_blur_offsets(samples: u8, intensity: f32) -> Vec<f64> {
    let samples = samples.clamp(2, 32);
    let exposure = intensity.clamp(0.0, 1.0) as f64;
    (0..samples)
        .map(|i| {
            let t = (i as f64 + 0.5) / samples as f64; // 0..1 across the shutter
            (t - 0.5) * exposure
        })
        .collect()
}

/// Apply the colour grade to one 0..=1 colour (LUT, then saturation, contrast,
/// brightness).
pub fn grade_pixel(rgb: [f32; 3], grade: &ColorGrade, lut: Option<&Lut3d>) -> [f32; 3] {
    let mut colour = rgb;
    if let Some(lut) = lut {
        let graded = lut.sample(colour);
        let strength = grade.lut_strength.clamp(0.0, 1.0);
        for i in 0..3 {
            colour[i] = colour[i] + (graded[i] - colour[i]) * strength;
        }
    }
    let luma = colour[0] * 0.2126 + colour[1] * 0.7152 + colour[2] * 0.0722;
    for channel in &mut colour {
        *channel = luma + (*channel - luma) * grade.saturation;
        *channel = (*channel - 0.5) * grade.contrast + 0.5;
        *channel = *channel * grade.brightness;
        *channel = channel.clamp(0.0, 1.0);
    }
    colour
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_clamp_resolution_and_frame_rate() {
        let settings = RenderSettings { width: 100, height: 100, fps: 37, ..RenderSettings::default() }.sanitized();
        assert_eq!(settings.width, 640);
        assert_eq!(settings.height, 360);
        assert_eq!(settings.fps, 30, "37 misses towards 30");
        let settings = RenderSettings { width: 9000, height: 9000, fps: 60, ..RenderSettings::default() }.sanitized();
        assert!(settings.width <= 7680 && settings.height <= 4320);
        assert!(settings.width % 2 == 0 && settings.height % 2 == 0);
        assert_eq!(settings.codec, "libx264");
    }

    #[test]
    fn equirectangular_forces_a_two_to_one_frame() {
        let settings = RenderSettings { projection: Projection::Equirectangular, width: 3840, height: 1080, letterbox: 2.35, alpha: true, ..RenderSettings::default() }.sanitized();
        assert_eq!(settings.height, 1920);
        assert_eq!(settings.letterbox, 0.0);
        assert!(!settings.alpha);
        assert_eq!(settings.aspect(), 2.0);
    }

    #[test]
    fn frame_counts_and_frame_ticks() {
        let settings = RenderSettings::default(); // 60 fps
        assert_eq!(settings.frame_count(20, 20), 60);
        assert_eq!(settings.frame_count(0, 20), 0);
        assert_eq!(settings.frame_count(1, 20), 3);
        assert!((settings.frame_tick(3, 20) - 1.0).abs() < 1e-9);
        assert!((settings.frame_tick(1, 20) - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn anti_aliasing_ids_round_trip_and_snap_samples() {
        for aa in [AntiAliasing::None, AntiAliasing::Msaa(8), AntiAliasing::Fxaa, AntiAliasing::Taa] {
            assert_eq!(AntiAliasing::from_id(&aa.id()), Some(aa));
        }
        assert_eq!(AntiAliasing::from_id("msaa3"), Some(AntiAliasing::Msaa(4)));
        assert_eq!(AntiAliasing::from_id("msaa32"), Some(AntiAliasing::Msaa(16)));
        assert_eq!(AntiAliasing::from_id("nope"), None);
        assert_eq!(AntiAliasing::Taa.sub_frames(), 2);
    }

    #[test]
    fn sub_frames_follow_motion_blur_and_passes() {
        let mut settings = RenderSettings::default();
        assert_eq!(settings.sub_frames(), 1);
        settings.motion_blur = MotionBlur { enabled: true, samples: 16, intensity: 0.5 };
        assert_eq!(settings.sub_frames(), 16);
        settings.multi_pass = true;
        assert_eq!(settings.sub_frames(), 32);
        settings.motion_blur.samples = 100;
        assert_eq!(settings.sanitized().sub_frames(), 64, "clamped to 32 samples, two passes");
    }

    #[test]
    fn motion_blur_offsets_are_centred_and_ordered() {
        let offsets = motion_blur_offsets(8, 1.0);
        assert_eq!(offsets.len(), 8);
        assert!(offsets[0] < 0.0 && offsets[7] > 0.0);
        assert!(offsets.windows(2).all(|w| w[0] < w[1]));
        let mean: f64 = offsets.iter().sum::<f64>() / offsets.len() as f64;
        assert!(mean.abs() < 0.01);
        // No exposure, no movement.
        assert!(motion_blur_offsets(8, 0.0).iter().all(|o| o.abs() < 1e-9));
    }

    #[test]
    fn vignette_darkens_the_edges_only() {
        assert!((vignette_factor(0.5, 0.5, 0.8, 0.5) - 1.0).abs() < 1e-6);
        assert!(vignette_factor(0.0, 0.0, 0.8, 0.5) < 0.5);
        assert_eq!(vignette_factor(0.0, 0.0, 0.0, 0.5), 1.0);
    }

    #[test]
    fn tilt_shift_leaves_the_band_sharp() {
        assert_eq!(tilt_shift_strength(0.5, 0.5, 0.4, 1.0), 0.0);
        assert!(tilt_shift_strength(0.0, 0.5, 0.4, 1.0) > 0.5);
        assert!(tilt_shift_strength(1.0, 0.5, 0.4, 1.0) > 0.5);
    }

    #[test]
    fn depth_of_field_grows_with_distance() {
        let dof = DepthOfField { enabled: true, focal_distance: 10.0, aperture: 0.5, max_blur: 10.0 };
        assert_eq!(dof_strength(10.0, dof), 0.0);
        assert!(dof_strength(20.0, dof) > 0.0);
        assert!(dof_strength(60.0, dof) > dof_strength(20.0, dof));
        assert!(dof_strength(60.0, dof) <= 10.0);
        assert_eq!(dof_strength(60.0, DepthOfField { enabled: false, ..dof }), 0.0);
    }

    #[test]
    fn luts_parse_and_sample() {
        // A 2×2×2 identity LUT.
        let mut text = String::from("TITLE \"identity\"\nLUT_3D_SIZE 2\n");
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    text.push_str(&format!("{r} {g} {b}\n"));
                }
            }
        }
        let lut = Lut3d::parse(&text).expect("parses");
        assert_eq!(lut.size, 2);
        assert_eq!(lut.title, "identity");
        let sampled = lut.sample([0.25, 0.5, 0.75]);
        assert!((sampled[0] - 0.25).abs() < 1e-5);
        assert!((sampled[1] - 0.5).abs() < 1e-5);
        assert!((sampled[2] - 0.75).abs() < 1e-5);
        assert!(Lut3d::parse("LUT_3D_SIZE 2\n0 0 0\n").is_none(), "incomplete data");
    }

    #[test]
    fn grading_respects_saturation_and_brightness() {
        let grade = ColorGrade::default();
        let colour = grade_pixel([0.2, 0.5, 0.8], &grade, None);
        assert!((colour[0] - 0.2).abs() < 1e-6);
        let grey = grade_pixel([0.2, 0.5, 0.8], &ColorGrade { saturation: 0.0, ..ColorGrade::default() }, None);
        assert!((grey[0] - grey[2]).abs() < 1e-6, "desaturated channels match");
        let bright = grade_pixel([0.2, 0.5, 0.8], &ColorGrade { brightness: 2.0, ..ColorGrade::default() }, None);
        assert!(bright[2] >= 0.99, "clamped at white");
    }

    #[test]
    fn ffmpeg_arguments_match_the_format() {
        let mp4 = RenderSettings::default();
        let args = mp4.ffmpeg_args("frames/%06d.png");
        assert!(args.windows(2).any(|w| w[0] == "-i" && w[1] == "frames/%06d.png"));
        assert!(args.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert_eq!(args.last().unwrap(), "voltlapse-render.mp4");

        let webm = RenderSettings { format: VideoFormat::WebM, codec: String::new(), ..RenderSettings::default() }.sanitized();
        assert!(webm.ffmpeg_args("x").windows(2).any(|w| w[0] == "-c:v" && w[1] == "libvpx-vp9"));

        let alpha = RenderSettings { alpha: true, ..RenderSettings::default() };
        assert!(alpha.ffmpeg_args("x").windows(2).any(|w| w[0] == "-pix_fmt" && w[1] == "yuva444p10le"));

        let png = RenderSettings { format: VideoFormat::PngSequence, ..RenderSettings::default() }.sanitized();
        assert_eq!(png.ffmpeg_args("x"), vec!["-y".to_owned()]);
    }

    #[test]
    fn render_settings_round_trip_through_json() {
        let settings = RenderSettings {
            width: 3840,
            height: 2160,
            fps: 120,
            format: VideoFormat::WebM,
            anti_aliasing: AntiAliasing::Taa,
            motion_blur: MotionBlur { enabled: true, samples: 12, intensity: 0.75 },
            grade: ColorGrade { lut: "film.cube".into(), vignette: 0.4, ..ColorGrade::default() },
            letterbox: 2.35,
            ..RenderSettings::default()
        }
        .sanitized();
        let back = RenderSettings::from_json(&settings.to_json());
        assert_eq!(back.width, 3840);
        assert_eq!(back.fps, 120);
        assert_eq!(back.format, VideoFormat::WebM);
        assert_eq!(back.anti_aliasing, AntiAliasing::Taa);
        assert_eq!(back.motion_blur.samples, 12);
        assert_eq!(back.grade.lut, "film.cube");
        assert!((back.letterbox - 2.35).abs() < 1e-6);
    }

    #[test]
    fn the_queue_renders_in_order_with_an_eta() {
        let mut queue = RenderQueue::new();
        let settings = RenderSettings::default();
        let first = queue.push(RenderJob {
            id: 0,
            name: "intro".into(),
            replay: "a.vrc".into(),
            camera: String::new(),
            settings: settings.clone(),
            from_tick: 0,
            to_tick: 100,
            rendered_frames: 0,
            total_frames: 300,
        });
        let second = queue.push(RenderJob {
            id: 0,
            name: "outro".into(),
            replay: "a.vrc".into(),
            camera: String::new(),
            settings,
            from_tick: 100,
            to_tick: 200,
            rendered_frames: 0,
            total_frames: 100,
        });
        assert_eq!((first, second), (1, 2));
        assert_eq!(queue.active().unwrap().name, "intro");
        for frame in 0..300 {
            queue.frame_done(if frame == 0 { 0.5 } else { 0.5 });
        }
        assert!(queue.jobs()[0].is_done());
        assert!((queue.progress() - 0.75).abs() < 1e-9);
        assert!((queue.eta_seconds() - 50.0).abs() < 1.0, "eta {}", queue.eta_seconds());
        assert_eq!(queue.advance().unwrap().name, "outro");
        assert!(!queue.is_finished());
        assert!(queue.remove(2));
        assert!(queue.is_finished());
        queue.clear();
        assert!(queue.jobs().is_empty());
    }

    #[test]
    fn estimated_size_is_plausible() {
        let settings = RenderSettings { bitrate: 8_000_000, ..RenderSettings::default() };
        assert_eq!(settings.estimated_bytes(10.0), 10_000_000);
        let auto = RenderSettings::default().estimated_bytes(60.0);
        assert!(auto > 10_000_000 && auto < 5_000_000_000, "{auto}");
    }
    #[test]
    fn the_filter_chain_covers_every_effect() {
        let mut settings = RenderSettings::default();
        assert_eq!(settings.filter_chain(), "", "defaults grade nothing");

        settings.grade.saturation = 1.4;
        settings.grade.vignette = 0.5;
        settings.grade.lut = "lut/cinematic.cube".into();
        settings.depth_of_field.enabled = true;
        settings.tilt_shift.enabled = true;
        settings.motion_blur.enabled = true;
        settings.motion_blur.samples = 8;
        settings.letterbox = 2.39;
        settings.projection = Projection::Equirectangular;
        let chain = settings.filter_chain();
        for part in ["eq=", "lut3d=file='lut/cinematic.cube'", "vignette=", "gblur=", "tmix=frames=8", "pad=", "v360="] {
            assert!(chain.contains(part), "{part} missing from {chain}");
        }
        let args = settings.ffmpeg_args("frames/frame_%06d.png");
        assert!(args.windows(2).any(|pair| pair[0] == "-vf" && pair[1] == chain));
        assert!(args.windows(2).any(|pair| pair[0] == "-i"));
        assert_eq!(args.last().map(String::as_str), Some("voltlapse-render.mp4"));
    }

    #[test]
    fn a_windows_lut_path_is_escaped_for_ffmpeg() {
        let mut settings = RenderSettings::default();
        settings.grade.lut = "C:\\LUTs\\film.cube".into();
        let chain = settings.filter_chain();
        assert!(chain.contains("C\\:/LUTs\\:film.cube"), "{chain}");
    }

    #[test]
    fn a_png_sequence_skips_the_encoder() {
        let mut settings = RenderSettings::default();
        settings.format = VideoFormat::PngSequence;
        settings.grade.vignette = 0.4;
        assert_eq!(settings.ffmpeg_args("frame_%06d.png"), vec!["-y".to_owned()]);
    }

}
