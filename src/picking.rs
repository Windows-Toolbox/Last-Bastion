//! The editor's selection beyond the game's, and its clipboard (Ctrl+C, Ctrl+X, Ctrl+V, Del).
//!
//! The game selects units and buildings (and looks at one zombie or house – `ui.rs`). In the editor's Edit mode more
//! can be selected: a sleeping zombie, a neutral thing or decoration by a click, and **everything in a box** – units,
//! buildings, zombies awake and asleep, houses of doom, loot, neutral things, big rocks, ruins and towers. All of it
//! together are the `pieces` the clipboard works on; what the game doesn't hold itself is in `Ui::picked`.
//!
//! Copying only reads the world (`World::copy`, `sim/clipboard.rs`) and keeps the copy in the editor (`Editor::clip`);
//! cutting, pasting and deleting are orders – so Ctrl+Z and Ctrl+Y take them back and bring them again.

use std::collections::HashSet;

use macroquad::prelude::*;

use crate::draw::draw_ring;
use crate::editor::{ClipAction, Module, Sight};
use crate::iso::Camera;
use crate::sim::{count_pieces, ornament_middle, Order, OrnamentKind, Piece, RailPiece, Thing, World};
use crate::ui::{tile_of, Place, Ui};

/// The mark on what's picked – the yellow of the editor's tools on the mouse.
const PICKED: Color = Color::new(1.0, 0.85, 0.3, 0.9);

impl Ui {
    /// Everything selected right now, as pieces: the units and buildings, the zombie or house looked at, the spawner,
    /// and what only the editor picks (`picked`).
    pub(crate) fn pieces(&self) -> Vec<Piece> {
        let mut pieces: Vec<Piece> = self.selected.iter().map(|&id| Piece::Unit(id)).collect();
        pieces.extend(self.selected_buildings.iter().map(|&id| Piece::Building(id)));
        pieces.extend(self.selected_zombie.map(Piece::Zombie));
        pieces.extend(self.selected_place.map(|place| match place {
            Place::Doom(id) => Piece::Doom(id),
            Place::Loot(id) => Piece::Loot(id),
        }));
        pieces.extend(self.editor.spawner.map(Piece::Spawner));
        pieces.extend(self.picked.iter().copied());
        pieces
    }

    /// Every frame, before the keys: what's picked and gone (cut, undone, a test started) is forgotten – and the editor
    /// learns how much is selected (Edit menu, status line).
    pub(crate) fn sync_pieces(&mut self, world: &World) {
        if self.editor.testing() {
            self.picked.clear();
        }
        if !self.picked.is_empty() {
            // The sleepers and zombies by a set each – a box may have picked thousands, and looking each one up in the
            // list would take thousands times thousands of steps. A `HashSet` is like Python's `set`.
            let key = |p: Vec2| (p.x.to_bits(), p.y.to_bits());
            let wanted: HashSet<(u32, u32)> = self.picked.iter().filter_map(|p| if let Piece::Sleeper(at) = p { Some(key(*at)) } else { None }).collect();
            let asleep: HashSet<(u32, u32)> = if wanted.is_empty() { HashSet::new() } else { world.sleep.iter().map(|s| key(s.pos)).filter(|k| wanted.contains(k)).collect() };
            let awake: HashSet<u32> = world.zombies.iter().filter(|z| z.is_alive()).map(|z| z.id).collect();
            self.picked.retain(|piece| match *piece {
                Piece::Building(id) => world.find_building(id).is_some(),
                Piece::Unit(id) => world.find_unit(id).is_some(),
                Piece::Neutral(id) => world.find_neutral(id).is_some(),
                Piece::Zombie(id) => awake.contains(&id),
                Piece::Sleeper(at) => asleep.contains(&key(at)),
                Piece::Doom(id) => world.find_doom(id).is_some(),
                Piece::Loot(id) => world.find_loot(id).is_some(),
                Piece::Spawner(id) => world.find_spawner(id).is_some(),
                Piece::BigRock(rock) => world.map.big_rocks().contains(&rock),
                Piece::Ornament(ornament) => world.ornaments.contains(&ornament),
            });
        }
        let pieces = self.pieces();
        self.editor.pieces = pieces.len();
        // More than units and buildings: counted by kind ("40 zombies, 1 house of doom") – else the editor names them.
        let more = !self.picked.is_empty() || self.selected_zombie.is_some() || self.selected_place.is_some();
        self.editor.picked_text = more.then(|| count_pieces(&pieces));
        self.editor.selection = pieces;
    }

    /// The editor's table asked for these (`Editor::select`): select them on the map – units and buildings as the
    /// game holds them, a spawner as the editor does, the rest as `picked`. `add`: each in or out of what's selected
    /// already. No voice answers, unlike a click on the map.
    pub(crate) fn select_pieces(&mut self, pieces: Vec<Piece>, add: bool) {
        if !add {
            self.deselect();
            self.editor.select_spawner(None);
        }
        // In the list: out of it – not in it: into it.
        fn toggle<T: PartialEq>(list: &mut Vec<T>, item: T) {
            match list.iter().position(|other| *other == item) {
                Some(k) => drop(list.remove(k)),
                None => list.push(item),
            }
        }
        for piece in pieces {
            match piece {
                Piece::Unit(id) => toggle(&mut self.selected, id),
                Piece::Building(id) => toggle(&mut self.selected_buildings, id),
                Piece::Spawner(id) => self.editor.select_spawner(Some(id)),
                other => toggle(&mut self.picked, other),
            }
        }
        // The editor's panel is drawn before the next `sync_pieces` – it should show these already.
        self.editor.selection = self.pieces();
    }

    /// Edit mode: Ctrl+C, Ctrl+X, Ctrl+V and Del – or the same from the Edit menu (`Editor::clip_request`). `true`: a
    /// key was used, and nothing else may have it this frame (X is also "repair all", Del the card's "demolish").
    pub(crate) fn clip_keys(&mut self, world: &World, camera: &Camera, orders: &mut Vec<Order>, mouse: Vec2, typing: bool) -> bool {
        let menu = self.editor.clip_request.take();
        if !self.editor.editing() || self.editor.testing() {
            return false;
        }
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        let pressed = |key: KeyCode| !typing && is_key_pressed(key);
        // `or_else`: the menu's entry, or else the key – like Python's `menu or key()`.
        let action = menu.or_else(|| {
            if ctrl && pressed(KeyCode::C) {
                Some(ClipAction::Copy)
            } else if ctrl && pressed(KeyCode::X) {
                Some(ClipAction::Cut)
            } else if ctrl && pressed(KeyCode::V) {
                Some(ClipAction::Paste)
            } else if pressed(KeyCode::Delete) {
                Some(ClipAction::Delete)
            } else {
                None
            }
        });
        let Some(action) = action else {
            return false;
        };
        let pieces = self.pieces();
        // The Terrain module has a clipboard of its own, for ground (`editor\terrain.rs`): with an area selected and
        // nothing else, Ctrl+C copies the area's ground – and Ctrl+V takes what was copied last in hand, if that
        // was ground.
        if self.editor.module == Module::Terrain {
            match action {
                ClipAction::Copy if pieces.is_empty() && self.editor.area.is_some() => {
                    self.editor.copy_area(world);
                    return true;
                }
                ClipAction::Paste if self.editor.terrain_clip.is_some() && (self.editor.ground_copied || self.editor.clip.is_none()) => {
                    self.editor.take_paste();
                    return true;
                }
                _ => {}
            }
        }
        match action {
            ClipAction::Copy | ClipAction::Cut => {
                let clip = world.copy(&pieces);
                if !clip.is_empty() {
                    self.editor.clip = Some(clip);
                    self.editor.ground_copied = false;
                    if action == ClipAction::Cut {
                        self.remove(pieces, orders);
                    }
                }
            }
            // Nothing selected: the key isn't taken – it may be for a region or a point (`ui.rs`).
            ClipAction::Delete if pieces.is_empty() => return false,
            ClipAction::Delete => self.remove(pieces, orders),
            ClipAction::Paste => {
                if let Some(clip) = &self.editor.clip {
                    // At the mouse – from the menu, or with the mouse beside the map, in the map's middle.
                    let on_map = menu.is_none() && crate::view::rect().contains(mouse) && !self.editor.has_mouse();
                    let at = if on_map { camera.screen_to_world(mouse) } else { camera.center };
                    orders.push(Order::Paste { clip: Box::new(clip.clone()), to: tile_of(at) });
                }
            }
        }
        true
    }

    /// The pieces off the map (one order – one step of undo), and nothing selected any more.
    fn remove(&mut self, pieces: Vec<Piece>, orders: &mut Vec<Order>) {
        if pieces.is_empty() {
            return;
        }
        orders.push(Order::RemovePieces { pieces });
        self.deselect();
        self.editor.select_spawner(None);
    }

    /// A box in Edit: everything in it – the units and buildings as the game holds them, the rest as `picked`.
    /// `add` (Shift): to what's selected already.
    pub(crate) fn pick_box(&mut self, world: &World, camera: &Camera, rect: Rect, add: bool) {
        let inside = |p: Vec2| rect.contains(camera.world_to_screen(p));
        let units: Vec<u32> = world.units.iter().filter(|u| !u.is_inside() && inside(u.pos)).map(|u| u.id).collect();
        let buildings: Vec<u32> = world.buildings.iter().filter(|b| inside(b.center())).map(|b| b.id).collect();
        let mut picked: Vec<Piece> = Vec::new();
        let zombies = world.zombies.iter().filter(|z| z.is_alive() && self.shows_zombie(world, z.pos) && inside(z.pos));
        picked.extend(zombies.map(|z| Piece::Zombie(z.id)));
        // The sleepers only where you see them – not through the player's fog.
        if self.editor.sight() != Sight::Player {
            picked.extend(world.sleep.iter().filter(|s| inside(s.pos)).map(|s| Piece::Sleeper(s.pos)));
        }
        picked.extend(world.doom.iter().filter(|d| inside(d.center())).map(|d| Piece::Doom(d.id)));
        picked.extend(world.loot.iter().filter(|l| inside(l.pos)).map(|l| Piece::Loot(l.id)));
        picked.extend(world.neutral.iter().filter(|n| inside(n.center())).map(|n| Piece::Neutral(n.id)));
        let rock_middle = |r: &crate::map::BigRock| vec2(r.tile.0 as f32, r.tile.1 as f32) + r.size as f32 / 2.0;
        picked.extend(world.map.big_rocks().iter().filter(|r| inside(rock_middle(r))).map(|r| Piece::BigRock(*r)));
        picked.extend(world.ornaments.iter().filter(|o| inside(ornament_middle(**o))).map(|o| Piece::Ornament(*o)));
        if !add {
            self.set_selection(units, buildings);
            self.picked = picked;
            return;
        }
        for id in units {
            if !self.selected.contains(&id) {
                self.selected.push(id);
            }
        }
        for id in buildings {
            if !self.selected_buildings.contains(&id) {
                self.selected_buildings.push(id);
            }
        }
        for piece in picked {
            if !self.picked.contains(&piece) {
                self.picked.push(piece);
            }
        }
    }

    /// A click in Edit on what the game can't select: a sleeping zombie, a neutral thing, a big rock, a ruin or a
    /// tower – the first under the mouse. `add` (Shift): in or out of what's picked; else it alone.
    pub(crate) fn pick_click(&mut self, world: &World, camera: &Camera, mouse: Vec2, add: bool) -> bool {
        let Some(piece) = self.piece_at(world, camera, mouse) else {
            return false;
        };
        if !add {
            self.deselect();
            self.picked = vec![piece];
        } else if let Some(k) = self.picked.iter().position(|&p| p == piece) {
            self.picked.remove(k);
        } else {
            self.picked.push(piece);
        }
        true
    }

    fn piece_at(&self, world: &World, camera: &Camera, mouse: Vec2) -> Option<Piece> {
        // Like `zombie_under_mouse`: the body a bit above the feet.
        let reach = 14.0 * camera.zoom.max(0.5);
        if self.editor.sight() != Sight::Player {
            let body = |p: Vec2| camera.world_to_screen(p) - vec2(0.0, 11.0 * camera.zoom);
            let nearest = world.sleep.iter().map(|s| (body(s.pos).distance(mouse), s.pos)).filter(|&(d, _)| d < reach).min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, at)) = nearest {
                return Some(Piece::Sleeper(at));
            }
        }
        let ground = camera.screen_to_world(mouse);
        let (x, y) = tile_of(ground);
        for n in &world.neutral {
            let hit = match n.thing {
                Thing::Building { kind, tile, turned } => {
                    let (w, h) = kind.footprint(turned);
                    (tile.0..tile.0 + w).contains(&x) && (tile.1..tile.1 + h).contains(&y)
                }
                Thing::Unit { pos, .. } => camera.world_to_screen(vec2(pos.0, pos.1)).distance(mouse) < reach * 1.4,
            };
            if hit {
                return Some(Piece::Neutral(n.id));
            }
        }
        // A piece of track only where it lies – not in the empty corner of a curve's 6 × 6.
        let ornament = world.ornaments.iter().rev().find(|o| match o.kind {
            OrnamentKind::Rail(shape) => RailPiece { shape, tile: o.tile }.covers((x, y)),
            _ => o.covers((x, y)),
        });
        if let Some(o) = ornament {
            return Some(Piece::Ornament(*o));
        }
        world.map.big_rocks().iter().find(|r| r.covers(x, y)).map(|r| Piece::BigRock(*r))
    }

    /// The marks on what's picked: a ring under a zombie, loot or a neutral unit, a frame round a house, a neutral
    /// building, a rock or a ruin. With the zombies as tiles their tiles are framed instead.
    pub(crate) fn draw_picked(&self, world: &World, camera: &Camera) {
        let tiles = self.editor.zombie_tiles(camera.zoom);
        let zombie = |at: Vec2| {
            if tiles {
                frame(camera, at.floor(), Vec2::ONE);
            } else {
                draw_ring(camera, at, 0.2, 1.5, PICKED);
            }
        };
        let corner = |t: (i32, i32)| vec2(t.0 as f32, t.1 as f32);
        for piece in &self.picked {
            match *piece {
                Piece::Zombie(id) => {
                    if let Some(z) = world.find_zombie(id) {
                        zombie(world.zombies[z].pos);
                    }
                }
                Piece::Sleeper(at) => zombie(at),
                Piece::Doom(id) => {
                    if let Some(d) = world.find_doom(id) {
                        let house = &world.doom[d];
                        frame(camera, corner(house.tile), vec2(house.size.0 as f32, house.size.1 as f32));
                    }
                }
                Piece::Loot(id) => {
                    if let Some(l) = world.find_loot(id) {
                        draw_ring(camera, world.loot[l].pos, 0.35, 1.5, PICKED);
                    }
                }
                Piece::Neutral(id) => {
                    if let Some(n) = world.find_neutral(id) {
                        match world.neutral[n].thing {
                            Thing::Building { kind, tile, turned } => {
                                let (w, h) = kind.footprint(turned);
                                frame(camera, corner(tile), vec2(w as f32, h as f32));
                            }
                            Thing::Unit { pos, .. } => draw_ring(camera, vec2(pos.0, pos.1), 0.3, 1.5, PICKED),
                        }
                    }
                }
                Piece::BigRock(rock) => frame(camera, corner(rock.tile), Vec2::splat(rock.size as f32)),
                Piece::Ornament(o) => {
                    let (w, h) = o.kind.footprint();
                    frame(camera, corner(o.tile), vec2(w as f32, h as f32));
                }
                Piece::Building(_) | Piece::Unit(_) | Piece::Spawner(_) => {}
            }
        }
    }
}

/// A frame on the ground round a rectangle of tiles (corner + size) – on screen a diamond's edges.
pub(crate) fn frame(camera: &Camera, corner: Vec2, size: Vec2) {
    let c = [corner, corner + vec2(size.x, 0.0), corner + size, corner + vec2(0.0, size.y)].map(|p| camera.world_to_screen(p));
    for k in 0..4 {
        let (a, b) = (c[k], c[(k + 1) % 4]);
        draw_line(a.x, a.y, b.x, b.y, 1.5, PICKED);
    }
}
