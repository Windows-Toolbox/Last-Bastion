//! Sound like in They Are Billions – TAB's `SysSoundSystem` (`ZX.Systems\SysSoundSystem.cs`) on macroquad's mixer. Its
//! files lie in `assets\sounds\<category>\` (`tools\tab_sounds.py`), in TAB's six categories; which sound goes with
//! which unit, zombie and building is in `data\audio.json`, the rest (clicks, alerts, music) below.
//!
//! How TAB plays them – and so do we:
//! - **Lists:** a name stands for every file of its folder whose name contains it (`ZombieRoarMedium` → 15 files),
//!   played one after another from a random start (TAB's `GetNextSoundIndex`).
//! - **Where:** a sound at a place only plays if that place is on the screen (10 % round it), quieter the further it
//!   is from the middle: full up to 40 % of the screen's width, nothing at its width – and zoomed out, everything is
//!   further away (TAB's `ActorHeight`).
//! - **Not too many:** the same list at most every 150 ms (zombie roars 1 s, infections 2 s …), the same file every
//!   400 ms, at most 16 sounds at once – a zombie's roar only while fewer than 10 play.
//! - **One voice at a time:** while a unit talks, the next one says nothing; "yes" and "attack" of the same unit not
//!   again within 3.5 s. The officer's alerts the same – each kind at most every 10 s, and not when the spot is on
//!   the screen anyway.
//! - **Music:** the landscape's own sound loops (water louder where the camera is over it, rain on the volcanic map);
//!   an announced swarm's countdown and then its music push it away, it comes back when the swarm is beaten.
//! - **Loading:** a file is decoded the first time it's wanted, on a thread of its own (`loader`) – so the game never
//!   waits for it. A short sound wanted before it's there is left out that once; music starts when it's ready. A round
//!   loads its usual sounds at its start (`preload`).
//!
//! Only the picture's side: the simulation never waits for or knows of a sound (co-op, DESIGN.md 12.8).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::io::Cursor;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::task::{Context, Poll, Waker};

use macroquad::audio::{load_sound_from_bytes, play_sound, set_sound_volume, stop_sound, PlaySoundParams, Sound};
use macroquad::prelude::*;

use crate::data;
use crate::iso::{Camera, START_ZOOM};
use crate::kinds::{BuildingKind, UnitKind};
use crate::map::Terrain;
use crate::settings::Volumes;
use crate::sim::{Breed, Fallen, Fx, Motion, Nest, Struck, Weather, World};

/// TAB's six folders – each with its own volume.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Category {
    Interface,
    Music,
    Ambient,
    Effects,
    UnitVoices,
    Notifications,
}

impl Category {
    const ALL: [Category; 6] =
        [Category::Interface, Category::Music, Category::Ambient, Category::Effects, Category::UnitVoices, Category::Notifications];

    fn folder(self) -> &'static str {
        match self {
            Category::Interface => "Interface",
            Category::Music => "Music",
            Category::Ambient => "Ambient",
            Category::Effects => "Effects",
            Category::UnitVoices => "UnitVoices",
            Category::Notifications => "Notifications",
        }
    }

    /// Its share of the whole (`Volumes`, percent).
    fn volume(self, v: &Volumes) -> f32 {
        let own = match self {
            Category::Interface => v.interface,
            Category::Music => v.music,
            Category::Ambient => v.ambient,
            Category::Effects => v.effects,
            Category::UnitVoices => v.voices,
            Category::Notifications => v.notifications,
        };
        (own / 100.0).clamp(0.0, 1.0) * (v.global / 100.0).clamp(0.0, 1.0)
    }
}

/// TAB's numbers (`SysSoundSystem.OnSetCamera`, `SoundListParams`, `DicMinTimeToPlaySameSoundList`): seconds.
const SAME_LIST: f64 = 0.15;
const SAME_FILE: f64 = 0.4;
const MOST: usize = 16;
const MOST_LOW: usize = 10;
const SAME_ALERT: f64 = 10.0;
/// "Yes" and "attack" of the same unit not again so soon (TAB: 3 s + 0.5 s).
const SAME_ANSWER: f64 = 3.5;
/// Lists that wait longer before they play again.
const SLOW_LISTS: [(&str, f64); 8] = [
    ("ZombieRoar", 1.0),
    ("InfectionBuilding", 2.0),
    ("Worker_Cut", 0.5),
    ("Worker_Pick", 0.5),
    ("Worker_Harvest", 0.5),
    ("Invalid", 0.5),
    ("InfectedNormalExplosion", 0.5),
    ("InfectedBigExplosion", 0.5),
];
/// TAB's camera is 24 wide when a map starts (`SetCameraCenter(…, 24f)`) – ours shows the same at `START_ZOOM`.
const TAB_START_WIDTH: f32 = 24.0;
/// A zombie roars at most this often (TAB: 3 s), a running one every 5 s (TAB's `IsIAFrameModule(300)`).
const ROAR_GAP: f64 = 3.0;
const RUN_ROAR: f64 = 5.0;
/// Running: as fast as this (tiles per second) or faster.
const RUNS: f32 = 1.5;
/// A giant's steps: walking every 70 frames, running every 22 (TAB, 60 frames a second).
const GIANT_WALK_STEP: f64 = 70.0 / 60.0;
const GIANT_RUN_STEP: f64 = 22.0 / 60.0;
/// The landscape fades in over 10 s, out over 3 s; music in 1 s, out 1–4 s (TAB).
const AMBIENT_IN: f32 = 10.0;
const AMBIENT_OUT: f32 = 3.0;
/// The rain starting or stopping (`World::weather`): its sound fades this many seconds, like its picture.
const WEATHER_FADE: f32 = 4.0;

/// What the loader hands back: the file, and its sound as a WAV with its length in seconds (`None`: it couldn't).
type Loaded = (String, Option<(Vec<u8>, f64)>);

/// A loaded sound and how long it plays.
struct Clip {
    sound: Sound,
    seconds: f64,
}

/// A list of files and how it was played: whose turn is next, when and where last.
struct List {
    files: Vec<String>,
    next: usize,
    last: Option<(f64, Vec2)>,
    gap: f64,
}

/// What's still to come a little later (TAB's `ExecuteWithDelay`).
enum Later {
    Effect { names: String, at: Vec2 },
    Alert { file: String },
    /// A one-off piece of music from a list (`GameStart`) …
    Music { names: String },
    /// … or this file – waiting for it to load, but not longer than `until`.
    MusicFile { file: String, until: f64 },
}

/// A looping sound (music, the landscape, water, rain): it fades towards `target` and stops at 0.
struct Track {
    file: String,
    category: Category,
    playing: Option<Sound>,
    volume: f32,
    target: f32,
    /// Change of volume per second.
    fade: f32,
}

/// Music and swarms, as TAB's `ZXSystem_GameLevel` and `ZXLevelState` play them.
#[derive(Default)]
struct Round {
    on: bool,
    look: String,
    /// The countdown of an announced event is on (its file).
    countdown: Option<String>,
    /// The swarm's music (its file) and its end.
    swarm: Option<(String, String)>,
    /// The round is over – won or lost was played.
    over: bool,
}

struct Player {
    /// The file names in each folder, and their sizes (TAB weighs voices by them).
    names: HashMap<Category, Vec<(String, u64)>>,
    clips: HashMap<String, Clip>,
    pending: HashSet<String>,
    to_loader: Option<Sender<String>>,
    from_loader: Option<Receiver<Loaded>>,
    lists: HashMap<(Category, String), List>,
    /// When the short sounds playing now end.
    playing: Vec<f64>,
    /// Music and landscape pieces played once (TAB's "game start", a wave's end, won, lost): the sound, its category,
    /// its own volume, until when – their loudness follows the settings while they play (`update_pieces`), like the
    /// looping tracks'. Before, a slider moved during one changed nothing.
    pieces: Vec<(Sound, Category, f32, f64)>,
    last_file: HashMap<String, f64>,
    /// TAB's two "unique" sounds: what a unit says and what the officer says – until when.
    voice_until: f64,
    alert_until: f64,
    /// The list said last, and when – for "yes" and "attack" (`SAME_ANSWER`).
    last_said: Option<(String, f64)>,
    last_voice: HashMap<String, String>,
    last_alert: HashMap<String, f64>,
    /// A selected building's sound: which list, until when.
    building_song: Option<(String, Sound, f64)>,
    later: Vec<(f64, Later)>,
    tracks: HashMap<&'static str, Track>,
    round: Round,
    /// Zombies: when each last roared (by `id`), and the giants' last step.
    roared: HashMap<u32, f64>,
    stepped: HashMap<u32, f64>,
    /// How each train (`World::trains`) moved last frame – braking plays its sound once.
    train_motion: Vec<Motion>,
    last_rat: f64,
    /// The camera this frame: middle (tiles), the map's part of the window, zoom.
    camera: Option<(Vec2, f32)>,
    volumes: Volumes,
    now: f64,
}

thread_local! {
    /// The one player – macroquad plays and draws on one thread only.
    static PLAYER: RefCell<Option<Player>> = const { RefCell::new(None) };
}

/// Runs `f` with the player – made the first time.
fn with<T>(f: impl FnOnce(&mut Player) -> T) -> T {
    PLAYER.with(|p| {
        let mut p = p.borrow_mut();
        let player = p.get_or_insert_with(Player::new);
        f(player)
    })
}

/// A future that's ready at once (macroquad's loading is, on Windows) – waited for right here.
fn now<T>(future: impl Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
    }
}

impl Player {
    fn new() -> Player {
        let root = data::folder("assets").map(|a| a.join("sounds")).filter(|d| d.is_dir());
        let mut names = HashMap::new();
        for category in Category::ALL {
            let mut files = Vec::new();
            if let Some(dir) = root.as_ref().map(|r| r.join(category.folder())) {
                for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let size = entry.metadata().map_or(1, |m| m.len());
                    files.push((name, size));
                }
            }
            files.sort();
            names.insert(category, files);
        }
        // The loader: files in, decoded sound out – on its own thread, so nothing waits for it.
        let (to_loader, from_loader) = match &root {
            Some(root) => {
                let (send_file, files) = channel::<String>();
                let (send_done, done) = channel();
                let root = root.clone();
                std::thread::spawn(move || {
                    for file in files {
                        let decoded = decode(&root.join(&file));
                        if send_done.send((file, decoded)).is_err() {
                            return;
                        }
                    }
                });
                (Some(send_file), Some(done))
            }
            None => (None, None),
        };
        Player {
            names,
            clips: HashMap::new(),
            pending: HashSet::new(),
            to_loader,
            from_loader,
            lists: HashMap::new(),
            playing: Vec::new(),
            pieces: Vec::new(),
            last_file: HashMap::new(),
            voice_until: 0.0,
            alert_until: 0.0,
            last_said: None,
            last_voice: HashMap::new(),
            last_alert: HashMap::new(),
            building_song: None,
            later: Vec::new(),
            tracks: HashMap::new(),
            round: Round::default(),
            roared: HashMap::new(),
            train_motion: Vec::new(),
            stepped: HashMap::new(),
            last_rat: 0.0,
            camera: None,
            volumes: Volumes::default(),
            now: 0.0,
        }
    }

    /// TAB's `GetSoundList`: the files of `category` whose names contain one of `names` (separated by spaces).
    fn list(&mut self, category: Category, names: &str) -> Option<&mut List> {
        if names.trim().is_empty() {
            return None;
        }
        let key = (category, names.to_owned());
        if !self.lists.contains_key(&key) {
            let all = &self.names[&category];
            let mut files: Vec<String> = Vec::new();
            for name in names.split([' ', ',', ';']).filter(|n| !n.is_empty()) {
                files.extend(all.iter().filter(|(f, _)| f.contains(name)).map(|(f, _)| format!("{}/{f}", category.folder())));
            }
            let gap = SLOW_LISTS.iter().find(|(k, _)| files.first().is_some_and(|f| f.contains(k))).map_or(SAME_LIST, |&(_, g)| g);
            // Shuffled, starting anywhere (TAB's `Disorder` and a random first one).
            let n = files.len();
            for i in (1..n).rev() {
                files.swap(i, rand::gen_range(0, i + 1));
            }
            let next = if n > 0 { rand::gen_range(0, n) } else { 0 };
            self.lists.insert(key.clone(), List { files, next, last: None, gap });
        }
        self.lists.get_mut(&key).filter(|l| !l.files.is_empty())
    }

    /// The loaded clip of `file` – or `None`, and it's asked for.
    fn clip(&mut self, file: &str) -> Option<(Sound, f64)> {
        if let Some(clip) = self.clips.get(file) {
            return Some((clip.sound.clone(), clip.seconds));
        }
        self.want(file);
        None
    }

    fn want(&mut self, file: &str) {
        if !self.clips.contains_key(file)
            && self.pending.insert(file.to_owned())
            && let Some(tx) = &self.to_loader
        {
            let _ = tx.send(file.to_owned());
        }
    }

    /// Loads what the loader has decoded – a few per frame, so a frame never gets long.
    fn take_loaded(&mut self) {
        let Some(rx) = &self.from_loader else {
            return;
        };
        let mut bytes = 0;
        while bytes < 8_000_000 {
            let Ok((file, decoded)) = rx.try_recv() else {
                break;
            };
            self.pending.remove(&file);
            if let Some((wav, seconds)) = decoded {
                bytes += wav.len();
                if let Ok(sound) = now(load_sound_from_bytes(&wav)) {
                    self.clips.insert(file, Clip { sound, seconds });
                }
            }
        }
    }

    /// How many short sounds play right now – and the tracks.
    fn count(&mut self) -> usize {
        let now = self.now;
        self.playing.retain(|&end| end > now);
        self.playing.len() + self.tracks.values().filter(|t| t.playing.is_some()).count()
    }

    /// Plays `file` once at `volume` (0..1, before the category's) – unless the same file just played.
    fn play_file(&mut self, file: &str, category: Category, volume: f32) -> Option<f64> {
        let loud = volume * category.volume(&self.volumes);
        if loud <= 0.0 {
            return None;
        }
        if self.last_file.get(file).is_some_and(|&t| self.now < t + SAME_FILE) {
            return None;
        }
        let (sound, seconds) = self.clip(file)?;
        play_sound(&sound, PlaySoundParams { looped: false, volume: loud });
        self.last_file.insert(file.to_owned(), self.now);
        let end = self.now + seconds;
        self.playing.push(end);
        if matches!(category, Category::Music | Category::Ambient) {
            self.pieces.push((sound, category, volume, end));
        }
        Some(end)
    }

    /// The next file of a list (TAB's `GetNextSoundIndex`).
    fn next_of(&mut self, category: Category, names: &str) -> Option<String> {
        let list = self.list(category, names)?;
        let file = list.files[list.next % list.files.len()].clone();
        list.next = (list.next + 1) % list.files.len();
        Some(file)
    }

    /// TAB's `PlaySound(category, list)`: the next of the list, anywhere – not at a place.
    fn play_list(&mut self, category: Category, names: &str) -> Option<f64> {
        let file = self.next_of(category, names)?;
        self.play_file(&file, category, 1.0)
    }

    /// TAB's `PlaySoundAtPosition`: on the screen only, quieter further from its middle, not too many, not the same
    /// list again too soon (unless the last one was far away and this one is close).
    fn play_at(&mut self, category: Category, names: &str, at: Vec2, low: bool) {
        let Some((distance, width)) = self.distance(at) else {
            return;
        };
        let most = if low { MOST_LOW } else { MOST };
        if self.count() > most {
            return;
        }
        let now = self.now;
        let Some(list) = self.list(category, names) else {
            return;
        };
        let (last, gap) = (list.last, list.gap);
        let may = match last {
            None => true,
            Some((time, _)) if now >= time + gap => true,
            // Heard far away just now – this one is close: it plays too (TAB's `CanPlaySoundList`).
            Some((_, spot)) => {
                let before = self.distance(spot).map_or(f32::MAX, |(d, _)| d);
                before > width * 0.5 && distance < width * 0.5 && (distance - before).abs() > width * 0.25
            }
        };
        if !may {
            return;
        }
        let Some(file) = self.list(category, names).map(|list| {
            list.last = Some((now, at));
            let file = list.files[list.next % list.files.len()].clone();
            list.next = (list.next + 1) % list.files.len();
            file
        }) else {
            return;
        };
        self.play_file(&file, category, loudness(distance, width));
    }

    /// How far `at` is from the camera's middle (TAB's scene units) and how wide the camera is – `None` if `at` isn't
    /// on the screen (10 % round it, TAB's `_MaxAreaCamaraForSounds`).
    fn distance(&self, at: Vec2) -> Option<(f32, f32)> {
        let (center, zoom) = self.camera?;
        let view = crate::view::rect();
        let camera = Camera { center, zoom };
        let s = camera.world_to_screen(at);
        let middle = vec2(view.x + view.w / 2.0, view.y + view.h / 2.0);
        let (half_w, half_h) = (view.w * 0.55, view.h * 0.55);
        if (s.x - middle.x).abs() > half_w || (s.y - middle.y).abs() > half_h {
            return None;
        }
        // The camera's width in TAB's units: 24 at the start, more zoomed out.
        let width = TAB_START_WIDTH * START_ZOOM / zoom;
        let distance = s.distance(middle) / view.w.max(1.0) * width;
        Some((distance, width))
    }

    /// One of TAB's "unique" sounds – a voice or an alert: not while the last one still plays.
    fn unique(&mut self, alert: bool, category: Category, names: &str) -> Option<f64> {
        let until = if alert { self.alert_until } else { self.voice_until };
        if self.now < until {
            return None;
        }
        let answer = names.contains("_Yes") || names.contains("_Attack");
        if answer && self.last_said.as_ref().is_some_and(|(n, t)| n == names && self.now < t + SAME_ANSWER) {
            return None;
        }
        let end = self.play_list(category, names)?;
        self.last_said = Some((names.to_owned(), self.now));
        if alert {
            self.alert_until = end;
        } else {
            self.voice_until = end;
        }
        Some(end)
    }

    /// TAB's `PlayUnitVoiceSoft`: like `unique`, but the short files more often than the long ones, never the same
    /// one twice in a row.
    fn voice_soft(&mut self, names: &str) {
        if self.now < self.voice_until {
            return;
        }
        let answer = names.contains("_Yes") || names.contains("_Attack");
        if answer && self.last_said.as_ref().is_some_and(|(n, t)| n == names && self.now < t + SAME_ANSWER) {
            return;
        }
        let Some(list) = self.list(Category::UnitVoices, names) else {
            return;
        };
        let files = list.files.clone();
        let sizes = &self.names[&Category::UnitVoices];
        let weight = |file: &String| {
            let name = file.rsplit('/').next().unwrap_or("");
            let kb = sizes.iter().find(|(n, _)| n == name).map_or(1, |&(_, s)| (s / 1024).max(1));
            (1_000_000 / (kb * kb + 15 * kb)).max(1)
        };
        let last = self.last_voice.get(names).cloned();
        let total: u64 = files.iter().map(weight).sum();
        let mut roll = rand::gen_range(0, total.max(1));
        let mut chosen = files[0].clone();
        for file in &files {
            let w = weight(file);
            if roll < w && Some(file) != last.as_ref() {
                chosen = file.clone();
                break;
            }
            roll = roll.saturating_sub(w);
        }
        self.last_voice.insert(names.to_owned(), chosen.clone());
        if let Some(end) = self.play_file(&chosen, Category::UnitVoices, 1.0) {
            self.voice_until = end;
            self.last_said = Some((names.to_owned(), self.now));
        }
    }

    /// TAB's `PlayNotification`: the officer – not when `at` is on the screen, the same kind at most every 10 s.
    fn alert(&mut self, kind: &str, at: Option<Vec2>) {
        if let Some(at) = at {
            if self.distance(at).is_some() {
                return;
            }
            if self.last_alert.get(kind).is_some_and(|&t| self.now < t + SAME_ALERT) {
                return;
            }
            self.last_alert.insert(kind.to_owned(), self.now);
        }
        self.unique(true, Category::Notifications, kind);
    }

    /// TAB's `PlayBuildingSound`: a selected building's own sound – one at a time, the old one stops.
    fn building_song(&mut self, names: &str) {
        if self.building_song.as_ref().is_some_and(|(n, _, end)| n == names && self.now < *end) {
            return;
        }
        if let Some((_, sound, _)) = self.building_song.take() {
            stop_sound(&sound);
        }
        let Some(file) = self.next_of(Category::Effects, names) else {
            return;
        };
        let loud = Category::Effects.volume(&self.volumes);
        if let Some((sound, seconds)) = self.clip(&file).filter(|_| loud > 0.0) {
            play_sound(&sound, PlaySoundParams { looped: false, volume: loud });
            self.building_song = Some((names.to_owned(), sound, self.now + seconds));
        }
    }

    /// A looping track: start it (fading in over `fade_in` s) or let it fade to `volume`.
    fn track(&mut self, slot: &'static str, category: Category, file: &str, volume: f32, fade_in: f32) {
        let same = self.tracks.get(slot).is_some_and(|t| t.file == file);
        if !same {
            if let Some(sound) = self.tracks.remove(slot).and_then(|old| old.playing) {
                stop_sound(&sound);
            }
            self.want(file);
            self.tracks.insert(slot, Track { file: file.to_owned(), category, playing: None, volume: 0.0, target: volume, fade: 0.0 });
        }
        let track = self.tracks.get_mut(slot).expect("just put in");
        track.target = volume;
        track.fade = if fade_in > 0.0 { 1.0 / fade_in } else { 100.0 };
    }

    /// Lets a track fade out over `seconds` – then it stops and its sound is let go.
    fn fade_out(&mut self, slot: &'static str, seconds: f32) {
        if let Some(track) = self.tracks.get_mut(slot) {
            track.target = 0.0;
            track.fade = if seconds > 0.0 { 1.0 / seconds } else { 100.0 };
        }
    }

    /// Every frame: the pieces played once take the volumes of the settings – at 0 they stop – and are forgotten
    /// once over.
    fn update_pieces(&mut self) {
        let (now, volumes) = (self.now, &self.volumes);
        self.pieces.retain(|(sound, category, volume, end)| {
            let loud = volume * category.volume(volumes);
            if loud <= 0.0 {
                stop_sound(sound);
                return false;
            }
            set_sound_volume(sound, loud);
            now < *end
        });
    }

    /// Every frame: the tracks fade, start once loaded, stop at 0.
    fn update_tracks(&mut self, dt: f32) {
        let mut done = Vec::new();
        let volumes = self.volumes.clone();
        let slots: Vec<&'static str> = self.tracks.keys().copied().collect();
        for slot in slots {
            let file = self.tracks[slot].file.clone();
            let ready = self.clips.get(&file).map(|c| c.sound.clone());
            let track = self.tracks.get_mut(slot).expect("listed");
            if track.playing.is_none() {
                if track.target <= 0.0 {
                    done.push(slot);
                    continue;
                }
                let Some(sound) = ready else {
                    continue;
                };
                play_sound(&sound, PlaySoundParams { looped: true, volume: 0.0 });
                track.playing = Some(sound);
            }
            let step = track.fade * dt;
            track.volume = if track.volume < track.target {
                (track.volume + step).min(track.target)
            } else {
                (track.volume - step).max(track.target)
            };
            let sound = track.playing.as_ref().expect("playing");
            set_sound_volume(sound, track.volume * track.category.volume(&volumes));
            if track.volume <= 0.0 && track.target <= 0.0 {
                stop_sound(sound);
                done.push(slot);
            }
        }
        for slot in done {
            // Let go of a long one (music, the landscape): it takes a lot of room decoded.
            if let Some(track) = self.tracks.remove(slot)
                && (track.category == Category::Music || track.category == Category::Ambient)
            {
                self.clips.remove(&track.file);
            }
        }
    }

    /// What's due from `later`.
    fn run_later(&mut self) {
        let now = self.now;
        let (due, wait): (Vec<_>, Vec<_>) = std::mem::take(&mut self.later).into_iter().partition(|(t, _)| *t <= now);
        self.later = wait;
        for (_, later) in due {
            match later {
                Later::Effect { names, at } => self.play_at(Category::Effects, &names, at, false),
                Later::Alert { file } => {
                    self.unique(true, Category::Notifications, &file);
                }
                Later::Music { names } => {
                    if let Some(file) = self.next_of(Category::Music, &names) {
                        self.later.push((now, Later::MusicFile { file, until: now + 20.0 }));
                    }
                }
                // Loaded: it plays. Not yet: asked for, and looked at again in a moment.
                Later::MusicFile { file, until } => {
                    if self.clips.contains_key(&file) {
                        self.play_file(&file, Category::Music, 1.0);
                    } else if now < until {
                        self.want(&file);
                        self.later.push((now + 0.25, Later::MusicFile { file, until }));
                    }
                }
            }
        }
    }
}

/// TAB's `GetVolumeFor3DSound`: full up to 40 % of the camera's width, nothing at its width – and the camera's height
/// above the map counts as distance too (zoomed out, everything is further away).
fn loudness(distance: f32, width: f32) -> f32 {
    let height = if width < 15.0 { 0.0 } else { 1.2 * (width - 15.0) };
    let d = distance.max(height);
    let (min, max) = (0.4 * width, width);
    if d <= min {
        1.0
    } else if d <= max {
        (1.0 - (d - min) / (max - min)).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// The loader's work: a file decoded into a WAV in memory (what macroquad's mixer reads quickest) and its length in
/// seconds. `.ogg` through `lewton`; a `.wav` is taken as it is.
fn decode(path: &Path) -> Option<(Vec<u8>, f64)> {
    let bytes = std::fs::read(path).ok()?;
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav")) {
        let seconds = wav_seconds(&bytes)?;
        return Some((bytes, seconds));
    }
    let mut reader = lewton::inside_ogg::OggStreamReader::new(Cursor::new(bytes)).ok()?;
    let channels = reader.ident_hdr.audio_channels as u16;
    let rate = reader.ident_hdr.audio_sample_rate;
    if !(1..=2).contains(&channels) || rate == 0 {
        return None;
    }
    let mut samples: Vec<i16> = Vec::new();
    while let Ok(Some(packet)) = reader.read_dec_packet_itl() {
        samples.extend(packet);
    }
    let seconds = samples.len() as f64 / channels as f64 / rate as f64;
    Some((wav(&samples, channels, rate), seconds))
}

/// 16-bit samples as a WAV file: the 44-byte header, then the samples.
fn wav(samples: &[i16], channels: u16, rate: u32) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// How long a WAV file plays: its data's bytes over the bytes per second of its "fmt " chunk.
fn wav_seconds(bytes: &[u8]) -> Option<f64> {
    let u32_at = |i: usize| bytes.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let mut i = 12;
    let mut per_second = None;
    while i + 8 <= bytes.len() {
        let id = &bytes[i..i + 4];
        let size = u32_at(i + 4)? as usize;
        if id == b"fmt " {
            per_second = u32_at(i + 16);
        } else if id == b"data" {
            return Some(size as f64 / per_second? as f64);
        }
        i += 8 + size + size % 2;
    }
    None
}

// ---------------------------------------------------------------------------------------------------------------
// What the game calls.

/// Once per frame: where the camera is (`None`: no map, the home screen), the volumes; the loader's sounds come in,
/// tracks fade, what's due plays.
pub fn frame(camera: Option<&Camera>, volumes: &Volumes) {
    with(|p| {
        let dt = (get_time() - p.now).clamp(0.0, 0.25) as f32;
        p.now = get_time();
        p.camera = camera.map(|c| (c.center, c.zoom));
        if p.volumes != *volumes {
            p.volumes = volumes.clone();
        }
        p.take_loaded();
        p.run_later();
        p.update_tracks(dt);
        p.update_pieces();
    });
}

/// A click, an "invalid", a window opening (`Interface`).
pub fn interface(names: &str) {
    with(|p| {
        p.play_list(Category::Interface, names);
    });
}

/// The officer says something (TAB's `PlayNotification` without a place) – not while he's still talking.
pub fn alert(kind: &str) {
    with(|p| p.alert(kind, None));
}

/// An effect not at a place (pause, power on/off).
pub fn effect(names: &str) {
    with(|p| {
        p.play_list(Category::Effects, names);
    });
}

/// What a unit says when selected or ordered – the start of its voice files and what (`What`, `Yes`, `Attack` …).
pub fn voice(kind: UnitKind, what: &str) {
    let Some(prefix) = data::get().audio.units.get(&kind).map(|s| s.voice.clone()).filter(|v| !v.is_empty()) else {
        return;
    };
    let names = format!("{prefix}_{what}");
    with(|p| match what {
        // TAB: selected and plain orders softly (`PlayUnitVoiceSoft`), the rest as a unique voice.
        "What" | "Yes" => p.voice_soft(&names),
        // Trained: not unique (`Train.cs` plays it plainly).
        "Trained" => {
            p.play_list(Category::UnitVoices, &names);
        }
        _ => {
            p.unique(false, Category::UnitVoices, &names);
        }
    });
}

/// A selected building's own sound (`PlayBuildingSound`).
pub fn building_selected(kind: BuildingKind) {
    let Some(names) = data::get().audio.buildings.get(&kind).map(|s| s.selected.clone()) else {
        return;
    };
    with(|p| p.building_song(&names));
}

/// The pause sound (Space).
pub fn paused(paused: bool) {
    effect(if paused { "PauseGame" } else { "ContinueGame" });
}

/// The home screen: TAB's intro music, the round's sounds stop.
pub fn home() {
    with(|p| {
        for slot in ["ambient", "rain", "water", "countdown", "swarm"] {
            p.fade_out(slot, 1.0);
        }
        p.round = Round::default();
        p.track("music", Category::Music, "Music/Intro.ogg", 1.0, 2.0);
    });
}

/// Everything that loops fades out – the editor while it edits.
pub fn quiet() {
    with(|p| {
        for slot in ["music", "ambient", "rain", "water", "countdown", "swarm", "train"] {
            p.fade_out(slot, 1.0);
        }
        p.round = Round::default();
    });
}

/// A round starts: the intro music stops, the landscape's sound and (if it rains, `World::weather`) the rain fade in,
/// TAB's "game start" piece plays, the round's usual sounds load.
pub fn round_start(world: &World) {
    let audio = &data::get().audio;
    let look = world.look.clone();
    let ambient = audio.ambient.get(&look).cloned().unwrap_or_else(|| "Ambient_Badland".to_owned());
    let rain = world.weather == Weather::Rain;
    with(|p| {
        p.fade_out("music", 1.0);
        for slot in ["countdown", "swarm"] {
            p.fade_out(slot, 1.0);
        }
        p.round = Round { on: true, look, ..Round::default() };
        p.track("ambient", Category::Ambient, &format!("Ambient/{ambient}.ogg"), 1.0, AMBIENT_IN);
        if rain {
            p.track("rain", Category::Ambient, "Ambient/RainLoopA.ogg", 1.0, AMBIENT_IN);
        } else {
            p.fade_out("rain", 1.0);
        }
        p.later.push((p.now + 0.2, Later::Music { names: "GameStart".to_owned() }));
        preload(p, world);
    });
}

/// The round's usual sounds, loaded now so they're there when it gets loud: its units', zombies' and buildings'.
fn preload(p: &mut Player, world: &World) {
    let audio = &data::get().audio;
    let mut lists: Vec<(Category, String)> = Vec::new();
    for s in audio.units.values() {
        for what in ["What", "Yes", "Attack", "Die", "Trained"] {
            lists.push((Category::UnitVoices, format!("{}_{what}", s.voice)));
        }
        lists.extend([s.shot.clone(), s.melee.clone(), s.hit.clone()].into_iter().map(|n| (Category::Effects, n)));
    }
    let mut breeds: Vec<Breed> = world.zombies.iter().map(|z| z.breed).collect();
    breeds.sort_by_key(|b| *b as u8);
    breeds.dedup();
    for breed in breeds {
        if let Some(s) = audio.zombies.get(&breed) {
            lists.extend([s.roar.clone(), s.attack.clone(), s.died.clone()].into_iter().map(|n| (Category::Effects, n)));
        }
    }
    for s in audio.buildings.values() {
        lists.extend([s.placed.clone(), s.destroyed.clone(), s.infected.clone(), s.shot.clone()].into_iter().map(|n| (Category::Effects, n)));
    }
    for n in ["ButtonConsoleClick", "ButtonAnalogicClick", "Invalid", "MapNotification", "VeteranLevelUp", "WindowOpen", "WindowClose"] {
        lists.push((Category::Interface, n.to_owned()));
    }
    lists.push((Category::Notifications, "Officer_".to_owned()));
    lists.push((Category::Effects, "PauseGame ContinueGame PickUpRessource Explosion_Fire RatRun Raven_".to_owned()));
    // A map with a railway: a script may send its train.
    if world.ornaments.iter().any(|o| matches!(o.kind, crate::sim::OrnamentKind::Rail(_))) {
        lists.push((Category::Effects, "Train_".to_owned()));
    }
    for (category, names) in lists {
        if let Some(list) = p.list(category, &names) {
            let files = list.files.clone();
            for file in files {
                p.want(&file);
            }
        }
    }
}

/// What happened in the last tick (`World::fx`) – called after every tick, like `effects.rs`.
pub fn take_fx(world: &World) {
    let audio = &data::get().audio;
    with(|p| {
        for fx in &world.fx {
            match *fx {
                Fx::UnitShot { from, kind, flight, .. } => {
                    if let Some(s) = audio.units.get(&kind) {
                        // Thanatos: its missile flies, its blow up close doesn't.
                        let names = if flight == 0 && !s.melee.is_empty() { &s.melee } else { &s.shot };
                        p.play_at(Category::Effects, names, from, false);
                    }
                }
                Fx::Flame { from, .. } => {
                    if let Some(s) = audio.units.get(&UnitKind::Lucifer) {
                        p.play_at(Category::Effects, &s.shot, from, false);
                    }
                }
                Fx::TowerShot { kind, from, .. } => {
                    if let Some(s) = audio.buildings.get(&kind) {
                        p.play_at(Category::Effects, &s.shot, from, false);
                    }
                }
                // The shocking tower: its own sound when it lets go, and where each ray hits (TAB's `ShockingTower`).
                Fx::Shock { from, to, .. } => {
                    if let Some(s) = audio.buildings.get(&BuildingKind::ShockingTower) {
                        p.play_at(Category::Effects, &s.shot, from, false);
                        p.play_at(Category::Effects, &s.hit, to, false);
                    }
                }
                Fx::Blast { at, by: Some(kind), .. } => {
                    if let Some(s) = audio.units.get(&kind) {
                        p.play_at(Category::Effects, &s.hit, at, false);
                    }
                }
                Fx::Blast { by: None, .. } => {} // a barrel or a mine: their own sound as they fall (`Fell`)
                Fx::ZombieDied { at, breed } => {
                    if let Some(s) = audio.zombies.get(&breed) {
                        p.play_at(Category::Effects, &s.died, at, false);
                        // A giant: its death roar, and 0.7 s later it hits the ground.
                        if !s.death.is_empty() {
                            p.play_at(Category::Effects, &s.death, at, false);
                            p.later.push((p.now + 0.7, Later::Effect { names: s.fall.clone(), at }));
                        }
                    }
                }
                Fx::UnitDied { at, kind } => {
                    p.play_at(Category::Effects, "InfectedNormalExplosion", at, false);
                    if let Some(s) = audio.units.get(&kind).filter(|s| !s.voice.is_empty()) {
                        p.unique(false, Category::UnitVoices, &format!("{}_Die", s.voice));
                    }
                }
                Fx::ZombieStruck { at, breed, what } => {
                    if let Some(s) = audio.zombies.get(&breed) {
                        p.play_at(Category::Effects, &s.attack, at, false);
                    }
                    let kind = match what {
                        Struck::Unit => "UnitsAttacked",
                        Struck::Building => "InfectedAttackingBuildings",
                        Struck::Wall => "InfectedAttackingWalls",
                    };
                    p.alert(kind, Some(at));
                }
                Fx::Acid { .. } => {}
                Fx::Fell { what, tile, size } => {
                    let at = vec2(tile.0 as f32 + size.0 as f32 / 2.0, tile.1 as f32 + size.1 as f32 / 2.0);
                    let names = match what {
                        // A mine going off: TAB's `TrapMine.Explode` – its blast, not a building falling.
                        Fallen::Building(kind) => audio.buildings.get(&kind).map(|s| if s.hit.is_empty() || kind != BuildingKind::MineTrap { s.destroyed.clone() } else { s.hit.clone() }),
                        Fallen::Doom(Nest::Small) => Some(audio.doom_small.clone()),
                        Fallen::Doom(_) => Some(audio.doom_big.clone()),
                    };
                    if let Some(names) = names {
                        p.play_at(Category::Effects, &names, at, false);
                    }
                }
                Fx::Infected { tile, size, kind } => {
                    let at = vec2(tile.0 as f32 + size.0 as f32 / 2.0, tile.1 as f32 + size.1 as f32 / 2.0);
                    if let Some(s) = audio.buildings.get(&kind) {
                        p.play_at(Category::Effects, &s.infected, at, false);
                    }
                    p.alert("BuildingsInfected", Some(at));
                }
                // TAB's `Build.cs`: placing plays its "placed" and its own sound – not at a place.
                Fx::Placed { kind, .. } => {
                    if let Some(s) = audio.buildings.get(&kind) {
                        p.play_list(Category::Effects, &s.placed);
                        p.play_list(Category::Effects, &s.selected);
                    }
                }
                // Finished: "building completed" (not for walls, towers, traps) – a wonder its fanfare.
                Fx::Built { kind, .. } => {
                    let structure = audio.buildings.get(&kind).is_none_or(|s| s.structure);
                    if kind.is_wonder() {
                        p.play_list(Category::Interface, "WonderCompleted");
                    }
                    if !structure {
                        p.alert("BuildingCompeted", None);
                    }
                }
                Fx::Trained { kind, .. } => {
                    if let Some(s) = audio.units.get(&kind).filter(|s| !s.voice.is_empty()) {
                        p.play_list(Category::UnitVoices, &format!("{}_Trained", s.voice));
                    }
                }
                Fx::Veteran => {
                    p.play_list(Category::Interface, "VeteranLevelUp");
                }
                Fx::Picked { at } => p.play_at(Category::Effects, "PickUpRessource", at, false),
                // TAB's `Locomotive.Start` / `Continue`.
                Fx::TrainStart { at } => p.play_at(Category::Effects, "Train_Start", at, false),
                Fx::Deserted { kind } => {
                    if let Some(s) = audio.units.get(&kind).filter(|s| !s.voice.is_empty()) {
                        p.unique(false, Category::UnitVoices, &format!("{}_Bye", s.voice));
                    }
                }
            }
        }
    });
}

/// Research done (TAB's `NotifyResearchComplete`): "research completed", then "new buildings / units available".
pub fn researched(buildings: bool, units: bool) {
    with(|p| {
        p.alert("ResearchCompeted", None);
        let next = if buildings { Some("NewBuildingsAvailable") } else if units { Some("NewUnitsAvailable") } else { None };
        if let Some(next) = next {
            let after = p.alert_until.max(p.now) + 0.6;
            if let Some(file) = p.next_of(Category::Notifications, next) {
                p.later.push((after, Later::Alert { file: file.rsplit('/').next().unwrap_or("").replace(".ogg", "") }));
            }
        }
    });
}

/// An announced event (TAB's `ZXLevelEvent.Notify`): the countdown music, the officer – how many, then (0.6 s after)
/// from where. `sides`: `Spawner::side` (0 north, 1 south, 2 west, 3 east), all of them: from all sides.
pub fn swarm_coming(zombies: u32, sides: &[u32], final_wave: bool, number: u32) {
    let size = if final_wave {
        "Officer_AlertInfectedBillions"
    } else if zombies < 200 {
        "Officer_AlertInfectedDozens"
    } else if zombies < 1500 {
        "Officer_AlertInfectedHundreds"
    } else {
        "Officer_AlertInfectedThousands"
    };
    let from = match sides {
        [] => None,
        [_, _, _, _] => Some("Officer_AlertInfectedFromAll"),
        [first, ..] => Some(match first {
            0 => "Officer_AlertInfectedFromN",
            1 => "Officer_AlertInfectedFromS",
            2 => "Officer_AlertInfectedFromW",
            _ => "Officer_AlertInfectedFromE",
        }),
    };
    let countdown = if final_wave {
        "Music/FinalCountdownLoop.ogg".to_owned()
    } else {
        ["Music/CountdownLoop.ogg", "Music/CountdownLoop2.ogg", "Music/CountdownLoop3.ogg"][number as usize % 3].to_owned()
    };
    let swarm = if final_wave {
        ("Music/FinalSwarmIncomingLoop.ogg", "Music/FinalSwarmEnd.ogg")
    } else {
        [
            ("Music/SwarmIncomingLoop.ogg", "Music/SwarmIncomingEnd.ogg"),
            ("Music/SwarmIncomingLoop2.ogg", "Music/SwarmIncomingEnd2.ogg"),
            ("Music/SwarmIncomingLoop3.ogg", "Music/SwarmIncomingEnd3.ogg"),
        ][number as usize % 3]
    };
    with(|p| {
        // TAB's `PlaySwarmCountdown`: the landscape goes, a swarm's music too.
        p.fade_out("ambient", AMBIENT_OUT);
        p.fade_out("swarm", 1.0);
        p.track("countdown", Category::Music, &countdown, 1.0, 1.0);
        p.round.countdown = Some(countdown.clone());
        p.round.swarm = Some((swarm.0.to_owned(), swarm.1.to_owned()));
        // Wanted now, so it's there when the swarm comes.
        p.want(swarm.0);
        if let Some(end) = p.unique(true, Category::Notifications, size)
            && let Some(from) = from
        {
            p.later.push((end + 0.6, Later::Alert { file: from.to_owned() }));
        }
    });
}

/// The announced swarm is here (TAB's `PlaySwarmMusic`): the countdown stops, its music starts a second later.
pub fn swarm_came() {
    with(|p| {
        p.fade_out("countdown", 1.0);
        p.fade_out("ambient", AMBIENT_OUT);
        p.round.countdown = None;
        if let Some((music, _)) = p.round.swarm.clone() {
            p.track("swarm", Category::Music, &music, 1.0, 1.0);
        }
    });
}

/// Every frame in a round: a swarm's music ends when no swarm is left on the map and none is announced (TAB's
/// `UpdateMiniMapIndicators`) – its end plays, the landscape comes back. The round over: won or lost.
pub fn round_frame(world: &World) {
    let swarms = !world.swarms().is_empty();
    let announced = world.next_announced().is_some();
    with(|p| {
        if !p.round.on {
            return;
        }
        if let Some(outcome) = world.outcome.as_ref().filter(|_| !p.round.over) {
            p.round.over = true;
            for slot in ["countdown", "swarm"] {
                p.fade_out(slot, 2.0);
            }
            let won = matches!(outcome, crate::sim::Outcome::Won { .. });
            let file = if won { "Music/GameWon.ogg" } else { "Music/GameOver.ogg" };
            p.later.push((p.now, Later::MusicFile { file: file.to_owned(), until: p.now + 20.0 }));
            return;
        }
        let swarm_playing = p.tracks.get("swarm").is_some_and(|t| t.target > 0.0);
        if swarm_playing && !swarms && !announced {
            p.fade_out("swarm", 1.5);
            if let Some((_, end)) = p.round.swarm.take() {
                p.want(&end);
                p.later.push((p.now + 1.5, Later::MusicFile { file: end, until: p.now + 20.0 }));
            }
            let ambient = data::get().audio.ambient.get(&p.round.look).cloned().unwrap_or_else(|| "Ambient_Badland".to_owned());
            p.track("ambient", Category::Ambient, &format!("Ambient/{ambient}.ogg"), 1.0, AMBIENT_IN);
        }
        // The rain's sound comes and goes with the rain (TAB: on the volcanic landscape and with the map condition
        // Rainy) – fading like its picture.
        let raining = p.tracks.get("rain").is_some_and(|t| t.target > 0.0);
        if (world.weather == Weather::Rain) != raining {
            match raining {
                false => p.track("rain", Category::Ambient, "Ambient/RainLoopA.ogg", 1.0, WEATHER_FADE),
                true => p.fade_out("rain", WEATHER_FADE),
            }
        }
        // Water: as loud as the camera's 8 × 8 block is water, a bit quieter zoomed out (TAB's `AmbientSound`).
        if let Some((center, zoom)) = p.camera {
            let (bx, by) = ((center.x as i32 >> 3) << 3, (center.y as i32 >> 3) << 3);
            let mut water = 0;
            for y in by..by + 8 {
                for x in bx..bx + 8 {
                    if world.map.terrain(x, y) == Terrain::Water {
                        water += 1;
                    }
                }
            }
            let width = TAB_START_WIDTH * START_ZOOM / zoom;
            let height = if width < 15.0 { 0.0 } else { 1.2 * (width - 15.0) };
            let most = (1.0 - height / width).clamp(0.0, 1.0);
            let share = (water as f32 / 64.0).min(most);
            if share > 0.0 || p.tracks.contains_key("water") {
                p.track("water", Category::Ambient, "Ambient/Water_Stream_River_A.ogg", share, 2.0);
            }
        }
        // The trains (TAB's `Locomotive.UpdateSounds`): at full speed their rolling loops – as loud as the loudest one
        // on the screen –, braking they play their arrival once and the loop fades out over 3 s.
        let mut rolling: f32 = 0.0;
        for (i, train) in world.trains.iter().enumerate() {
            let at = train.line.point_at(train.offset);
            let before = p.train_motion.get(i).copied();
            if train.motion == Motion::Decelerating && before != Some(Motion::Decelerating) {
                p.play_at(Category::Effects, "Train_Arrival", at, false);
            }
            if train.motion == Motion::FullSpeed && train.speed > 0.0 {
                rolling = rolling.max(p.distance(at).map_or(0.0, |(d, width)| loudness(d, width)));
            }
        }
        p.train_motion = world.trains.iter().map(|t| t.motion).collect();
        if rolling > 0.0 {
            p.track("train", Category::Effects, "Effects/Train_Moving_Loop.ogg", rolling, 0.5);
        } else {
            p.fade_out("train", TRAIN_FADE);
        }
    });
}

/// The train's rolling fades out over this many seconds when it brakes (TAB: 3000 ms).
const TRAIN_FADE: f32 = 3.0;

/// Zombies on the screen roar now and then – an idle one when it growls, a running one every 5 s – and a giant's
/// steps shake the ground (TAB's `Zombie.OnUpdate`, `ZombieGiant.OnUpdate`). Only the ones you see.
pub fn zombies(world: &World, looks: &crate::draw::Looks) {
    let audio = &data::get().audio;
    with(|p| {
        if p.camera.is_none() {
            return;
        }
        let now = p.now;
        let mut alive = HashSet::new();
        for zombie in &world.zombies {
            let look = looks.get(zombie.id);
            if look.alpha < 1.0 || p.distance(zombie.pos).is_none() {
                continue;
            }
            alive.insert(zombie.id);
            let Some(s) = audio.zombies.get(&zombie.breed) else {
                continue;
            };
            if !s.steps.is_empty() && look.walking {
                let every = if look.speed >= RUNS { GIANT_RUN_STEP } else { GIANT_WALK_STEP };
                if p.stepped.get(&zombie.id).is_none_or(|&t| now >= t + every) {
                    p.stepped.insert(zombie.id, now);
                    p.play_at(Category::Effects, &s.steps, zombie.pos, false);
                }
            }
            if p.roared.get(&zombie.id).is_some_and(|&t| now < t + ROAR_GAP) {
                continue;
            }
            let growls = look.growl_from.is_some() && !look.walking;
            // A running one: every 5 s, each on its own beat.
            let runs = look.walking && look.speed >= RUNS && {
                let phase = (zombie.id % 300) as f64 / 60.0;
                ((now + phase) % RUN_ROAR) < 1.0 / 30.0
            };
            if growls || runs {
                p.roared.insert(zombie.id, now);
                p.play_at(Category::Effects, &s.roar, zombie.pos, true);
            }
        }
        p.roared.retain(|id, _| alive.contains(id));
        p.stepped.retain(|id, _| alive.contains(id));
    });
}

/// For the overlay (F3): how many sounds are loaded, still loading, playing – or that the folder is missing.
pub fn status() -> String {
    with(|p| {
        if p.to_loader.is_none() {
            return r"Sounds: no assets\sounds folder (tools\tab_sounds.py)".to_owned();
        }
        let playing = p.count();
        format!("Sounds {} loaded, {} loading, {} playing", p.clips.len(), p.pending.len(), playing)
    })
}

/// A raven flies up (its wings and its caw), a rat dashes off (`critters.rs`) – where you see it.
pub fn critter(raven: bool, at: Vec2) {
    with(|p| {
        if raven {
            p.play_at(Category::Effects, "Raven_StartFlight", at, false);
            p.play_at(Category::Effects, "Raven_Caw", at, false);
        } else if p.now >= p.last_rat + 3.0 {
            p.last_rat = p.now;
            p.play_at(Category::Effects, "RatRun", at, false);
        }
    });
}
