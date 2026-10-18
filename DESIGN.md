# Last Bastion – Design Document

> A real-time strategy survival game: StarCraft's unit control, They Are Billions' hordes and run
> structure, Factorio's production chains and power grid – in rounds of 2–4 hours (like TAB: you pick the number of
> days).
>

---

## Contents

1. [Pitch and pillars](#1-pitch-and-pillars)
2. [What we take from each game](#2-what-we-take-from-each-game)
3. [Core loops](#3-core-loops)
4. [A round, phase by phase](#4-a-round-phase-by-phase)
5. [Economy](#5-economy)
6. [Buildings](#6-buildings)
7. [Units and control](#7-units-and-control)
8. [The horde](#8-the-horde)
9. [Map generation](#9-map-generation)
10. [Home screen and meta progression](#10-home-screen-and-meta-progression)
11. [UI and controls](#11-ui-and-controls)
12. [Technical notes](#12-technical-notes)
13. [Prototype roadmap](#13-prototype-roadmap)
14. [Open questions](#14-open-questions)
15. [Idea parking lot](#15-idea-parking-lot)

---

## 1. Pitch and pillars

>*Build a compact industrial colony, command an elite strike force, and survive until the final wave against a world
being swallowed by the horde.*

### Design pillars

Every feature has to serve at least one pillar. If it serves none, it goes to the [parking lot](#15-idea-parking-lot).

| # | Pillar | What it means in practice |
|---|---|---|
| 1 | **The factory is the base – and the base is the target.** | Power lines, machines and houses are both your economy and your weak spot. Expanding production makes you stronger *and* louder. |
| 2 | **An army, not a blob – lots of control.** | A StarCraft-sized army (≈ 50–120 units, as many as your economy can pay for) against a TAB-sized horde (thousands). Every unit matters and has something to click. |
| 3 | **One run, one story.** | 2–4 hours (the number of days is picked before the round, like TAB), clear escalation, a known climax (attack waves + the final one). Win or lose, the round tells a story. |

**If only one could stay (open question 13, 2026-10-15):** the **TAB side** – the horde against a base built on a
grid, seen **isometrically**. That's the heart of the game; the StarCraft control and the Factorio chains serve it.

### Who is it for?

- People who love StarCraft's control but don't want PvP stress or 300 APM.
- People who love TAB's tension but want more to *do* with their army.
- People who love Factorio's satisfaction but don't have 40 hours per save.

---

## 2. What I take from each game

| Source | Take | Leave out |
|---|---|---|
| **They Are Billions** | Huge hordes, infection cascades (one breach → chain reaction), known attack waves, run-based rounds, colony home screen, difficulty modifiers as score %, unit upkeep in gold, active pause | Stat-check blob armies, punishing one-save rule (❓ maybe as an "Ironman" option) |
| **StarCraft** | Control groups, smart-casting, unit abilities, hard counters, rally points, attack-move, crisp unit responsiveness | PvP, APM pressure, three asymmetric factions (scope!) |
| **Factorio** | Ore patches, simple production chains, power grid, pollution for nest/zombie attacks (in that sense units firing should produce pollution too) | Belts, hundreds of items, megabase scale, saves that last for days or weeks |
| **Anno** *(bonus)* | Houses that upgrade by being supplied with goods | Trade routes, ships, diplomacy |

---

## 3. Core loops

The game runs on four loops at different time scales. Each must be fun on its own.

| Time scale | Loop | Player feeling |
|---|---|---|
| **Seconds** | Select units → give orders → use abilities → watch the result | "My army does exactly what I want." |
| **Minutes** | Place machines → connect power → supply houses → earn gold → pay for army and defences | "My colony is growing and humming." |
| **One round (2–4 h, 40–80 days)** | Scout → expand → survive waves → prepare → final wave | "That was close – and I made it." |
| **Across rounds** | Win → unlock tech / regions / modifiers → harder round | "Next time I'll try the other strategy." |

---

## 4. A round, phase by phase

The skeleton every system has to fit into. If a phase has nothing interesting happening, a system is missing.

### 4.1 Round length: days, like TAB

- Before the round the player picks how many **days** it lasts (round setup, 10.1), like TAB's survival mode. The
  final wave comes on the last day.
- A day is a fixed span of game time: **≈ 3 minutes** ❓ (open question 18). That's **longer than TAB's days**, so a
  round has fewer of them – each day counts for more. That gives:

| Days | Time at 1× speed (without pauses) |
|---|---|
| 40 | ≈ 2:00 h |
| 50 | ≈ 2:30 h |
| 60 | ≈ 3:00 h |
| 80 | ≈ 4:00 h |

- **Fewer days = harder** (less time to build up before the final wave) = higher score multiplier, like TAB.
- Phases and waves are planned as a **share of the round**, not in fixed minutes, so they stretch with the number of
  days. A longer round gives more time to build, but also brings more waves in between.
- "Day" is only a unit of time here. A real day/night cycle is still in the [parking lot](#15-idea-parking-lot).

### 4.2 Phases

| Share of the round | With 50 days | Phase | What happens | Threat |
|---|---|---|---|---|
| 0–10 % | day 1–5 (0:00–0:15) | **Landing** | Command center lands. Scout with the starting squad, find ore and fields, first houses, first market. | Stragglers only (single zombies wandering in). |
| 10–30 % | day 6–15 (0:15–0:45) | **Setting up** | First production chains, power grid, first walls. Barracks, first real units. | First mini-hordes, drawn by your **noise**. First small wave. |
| 30–60 % | day 16–30 (0:45–1:30) | **Expansion** | Tier 2 houses, second and third ore field, workshops. Army clears **nests** and **points of interest** for loot. | Waves every few days from random directions. **Creep** starts spreading. |
| 60 % – last day | day 31–49 (1:30–2:27) | **Preparing** | Tier 3, late tech, heavy defences, scaling up. Banner: *"Final wave in 2 days – from the north-east"*. | Hordes get special zombies and the first giants, nests expand. |
| Last day | day 50 | **Final wave** | Everything comes at once, including giants. | Survive = win. |

- **The middle is the long part.** In 2–4 hours, expansion and preparing are most of the round. They need enough to
  do: several waves, nests to clear, points of interest, tier 3, a second front. If the middle drags, a system is
  missing.
- **Saving is a must** with rounds this long: free saving and loading like Factorio (11.5, open question 9).

Optional: **endless mode** afterwards (score keeps counting, waves keep growing).

---

## 5. Economy

### 5.1 Resources and items

Keep it to **about 10 item types**. Even in a round of several hours the factory is a means, not the goal – more items
would pull the focus away from the army.

| Tier | Item | Made from | Used for |
|---|---|---|---|
| Raw | **Wood** | Lumber camp (forest) | Early buildings, walls |
| Raw | **Stone** | Quarry (rock) | Walls, towers |
| Raw | **Iron ore** | Drill on ore patch | → Iron |
| Raw | **Coal** | Drill on coal patch | Power plant, smelter fuel |
| Raw | **Food** | Farm, fishery, hunter | Houses |
| Raw | **Oil** | Oil pump on an oil node (2 × 2, 2026-10-18) | Oil burning generator, later more ❓ |
| 1 | **Iron** | Smelter (ore + coal) | Buildings, tools, ammo |
| 1 | **Energy** | Command center (a little), wood generator (early, inefficient), coal plant / later wind, solar | Machines, houses, towers |
| 2 | **Tools** | Workshop (iron + wood) | Tier 2 houses, units |
| 2 | ~~Ammo~~ | ~~Armory (iron + coal)~~ | **Free for now** (open question 6, 2026-10-15) – ammo as an item is parked |
| 3 | **Medicine** | Lab – recipe later (tier 3 is far away) | Tier 3 houses, healing |
| — | **Gold** | Houses (taxes) | Buying units and their **upkeep** (7.2), research, buildings |

Oil (2026-10-18) makes it 11 item types – still about 10. The bottom bar already shows the TAB-like stock (11.1):
colonists, workers, food, energy, gold, wood, stone, iron, oil.

### 5.2 How it all connects

```
 Iron ore ──┐
 Coal ──────┼──► Smelter ──► Iron ──┬──► Workshop ──► Tools ──────┐
 Coal ──────┴──► Power plant ──► Energy ──────────────────────────┤
 Wood ─────────────────────────────┘                              │
 Food ────────────────────────────────────────────────────────────┼──► Houses ──► Gold, workers
                                                                  │                    │
                                                    Medicine ─────┘                    ▼
                                                                         Barracks / Factory ──► Units
                                                                                                  │
                                                                     every unit costs gold/min ◄──┘ (upkeep)

 Every machine ──► NOISE ──► attracts zombies (see 8.3)
```

### 5.3 Houses (the key system)

Houses link economy and army – this is what makes the Factorio part matter for the fight.

| House tier | Build cost | Needs (per minute, first guess) | Gives | Unlocks |
|---|---|---|---|---|
| **Tent** | Wood | Food, energy | 2 gold/min, 2 colonists | Basic infantry |
| **Stone house** | Wood, stone | More food, energy, tools | 5 gold/min, 4 colonists | Vehicles, tier 2 towers |
| **Apartment** | later | Food, energy, tools, medicine | 10 gold/min, 6 colonists | Elite units, tier 3 research |

- **First only tents and stone houses** (2026-10-15). Tier 3 comes much later in development.
- Tents need **energy from day 1** (open question 23, 2026-10-15): the **command center gives a little**, and when that's
  not enough you build **small wood-fired generators** – cheap but inefficient (lots of wood, little energy, loud).
  The coal plant replaces them later.
- A house that isn't supplied stops paying gold. You can downgrade it if you have to. 
- The same gold pays for the army's **upkeep** (7.2). There is no supply cap: the army is as big as your income
  allows, so every gold is a choice between growing the colony and growing the army.
- Houses are delivered to by a **distribution radius** around a market/warehouse (TAB-like, simpler).
- **Rough check ❓:** an army of 80 units at ≈ 3 gold upkeep each costs ≈ 240 gold/min. That's 24 apartments or 120
  tents. When in the round should the player get there (tier 2 from ≈ 30 % of the round, section 4.2), and does
  that work for 40 days as well as for 80? Check it in a spreadsheet before writing code (open question 14).

### 5.4 Logistics (no belts)

- Like TAB: what gathering buildings produce goes straight into the **colony stock**. Production buildings take their
  inputs from it and put their outputs back. No belts, no item routing.
- The Factorio part is in the **production chains** (ore + coal → iron → tools …) and the **power grid**, not in
  moving items around.
- Warehouses raise the storage limit; markets supply houses within their radius (5.3).

### 5.5 Power

- Power poles connect machines into a grid (Factorio).
- Not enough power → every machine on that grid slows down proportionally. Towers and houses on the grid suffer too.
- Destroyed poles split the grid → a zombie breach in one corner can black out half the base. (Pillar 1.)

### 5.6 Colonists

Colonists exist twice: as a **resource** (a number) and as **people walking around** (only for the eye).

#### Colonists as a resource (open question 24, 2026-10-15)

- Houses give **colonists** – they pay gold (5.3) – and **workers**, like TAB (2026-10-19: a tent 6 colonists and
  2 workers). The command center gives some workers too (the founders, enough for the start units and a little more).
  The bottom bar shows colonists and free/all workers next to the gold (11.1).
- **Every other building and every unit takes workers** for as long as it stands or lives (`worker_cost` in
  `data\`, 2026-10-19). No free workers → **nothing new** can be built or trained. Walls take none for now – a wall
  line would eat all of them.
- **Later, when workers get lost** (an infected house) and there are fewer than in use, buildings are **switched
  off** until there are enough again (2026-10-15):
  - **Gathering buildings first** (lumber camp, quarry, drill …), **spread over the resources**: one of each kind in
    turn, so no single resource drops to zero while the others keep running.
  - **Farms and power generators come last** (2026-10-15). Switching off food or power would leave houses unsupplied
    → less gold, maybe fewer colonists → even more buildings switched off: a downward spiral like in TAB.
  - In between: production and defence ❓ (order still open).
- **Units take workers too:** **1 per infantry unit, more for vehicles** (2026-10-15, exact numbers in the data
  files). Taken when it goes into the production queue (back if cancelled), gone for as long as the unit lives.
- → More houses = more workers = more buildings and a bigger army. Houses are the bottleneck, as in TAB.

#### The people walking around (decoration)

The colony should look alive, like in TAB, but these figures have **no function in the game** (2026-10-15).

- They **walk visibly** from house to workplace and back. They can't be controlled, can't be caught and can't turn
  into zombies.
- **Fleeing:** when zombies come close, they go into the **nearest house within a short distance** and stay there until
  it's quiet. Not like TAB: no running across the whole map to some lit building, no back and forth between house and
  street, no running into mountains and fading away.
- Number: a few hundred at most. Enough to look busy; each one only needs simple path-finding to its two targets.

---

## 6. Buildings

Costs and stats live in `data\buildings.json` (12.4).

| Category | Building | Notes |
|---|---|---|
| **Core** | Command center | Start building, small storage, losing it = losing the game. Builds everything, trains nothing |
| **Housing** | Tent → stone house → apartment | Upgrade when the materials and the next tier's goods are there (5.3) |
| **Gathering** | Hunter cottage, fishing hut, saw mill, quarry, oil pump, farm | Placed on/near the right tiles, work on their own |
| **Production** | Foundry (smelter), wood workshop, stone workshop, lab | Take inputs from the colony stock (5.4) |
| **Logistics** | Warehouse, market, road | Warehouse = storage; market = supply radius for houses; road = **units and zombies** faster |
| **Power** | Tesla tower (pole), generator (wood), oil burning generator, coal plant, windmill, solar, battery | Wood generator early, cheap, inefficient; wind/solar quieter but weaker |
| **Military** | Barracks, siege factory (vehicles), munitions factory (❓ 26), tech lab | Production queue + rally point |
| **Defence** | Wall, gate, tower (4 units inside), mines, trip wire, ballista tower, MG tower, flame tower | Towers use energy |
| **Special** | Radar, sound dampener, cleanser (removes creep/noise), beacon (lures zombies to a kill zone) | No menu yet |
| **Wonders** | ❓ (29) | Most expensive, very strong buffs |

- **Building like TAB** (open question 5): placed, paid at once, builds itself. No worker units.
- **Infection** (from TAB): HP + shield; shield down = **infected** – colonists come out as zombies, more depending on
  the building. A breach should feel catastrophic. The infected building then belongs to the zombies: they stop
  hitting it, and its walls are broken – zombies and units walk right through it (changed 2026-10-18, before: around
  it). **Repairing** it cures it.
- **Repair** (2026-10-18): F on any selected building, **X / "Repair all"** for every building (X only while no
  units are selected – then it's the Line formation). Costs its share of the building's `repair_cost` (gold now,
  later also wood/stone …), paid at once; the damage comes back over `repair_seconds` (full repair 10 s). Not enough
  gold: cheapest first, as many as it pays for. Not with a zombie in the build zone (`can_repair` in
  `placement.rs`), later not on creep (8.4).
- **Roads:** units **and zombies** are faster on them – a road into the base is also a fast lane for the horde.
- **Build menus (2026-10-18):** one per key on the command center, like TAB's categories – **Q** colonists,
  **W** resources, **E** energy, **A** industry and trade, **S** military, **D** defence, **Y** wonders. Planned
  buildings already sit on their spot, greyed out.

**In the prototype:** command center, tent, barracks (the only building that trains), wall, gate (3 × 1, door for
your units), tower (up to 4 units, +2 range and sight, can't be hit). Placement rules all in `src/placement.rs`:
drag a line (walls diagonal, 1 wide like TAB), gate over walls (walls refund 50 %, never a saving), 1 free tile between
towers, no zombie within 4 tiles. Your own units may stand on the site – they're pushed off to the nearest free
spot (like StarCraft). Costs gold and workers (5.6). No build time yet. Repair (F) on every building, cures infected
ones.

---

## 7. Units and control

### 7.1 Control (the StarCraft part)

Pillar 2. The heart is still the TAB side (open question 13).

- Box/click select, double click = all of that type on screen, Shift = add/remove. Control groups 1–0.
- **Attack-move**, move, stop, hold, patrol, **follow** (move onto an own unit). Shift queues every command;
  letting go of Shift takes a waiting command off the mouse, pressing Shift again brings it back.
- **Hold** only means "don't walk by yourself": holding units can still be pushed, and a squeezed crowd pushes itself
  apart (2026-10-19 – before they stood like walls).
- **Holding a training key** presses it again and again, like StarCraft with Windows' key repeat (2026-10-19).
- **Smart-casting:** an ability key uses the best-suited unit of the selection, not all.
- **Smart targeting:** fire is spread over different zombies, no overkill (StarCraft 2). *In since 2026-10-17 – without
  it 160 soldiers fired like 15.*
- **Rally points** for production buildings, also onto a unit.
- **Formations like *Diplomacy is Not an Option*:** line, box, circle – only when **dragging**; a click sends everyone
  to the point. Soldiers in front, the rest behind. Travels with the order (same on both PCs in co-op).
- **Active pause** (Space, singleplayer only): orders still work and are shown at once, carried out on the next tick.
- **Game menu** (Esc): pauses everything in singleplayer, nothing in co-op.

### 7.2 Unit roster (first draft)

Small army, strong units: infantry from **TAB**, support and vehicles from **StarCraft**. Each unit has **exactly
one active ability** (low APM, clear identity).

| Unit | From | Tier | Upkeep (gold/min) | Role | Ability | Noise | Counters | Weak against |
|---|---|---|---|---|---|---|---|---|
| **Ranger** | TAB | 1 | 1 | Fast, cheap, bow, weak damage | *Sprint* ❓ | Silent | Single runners, scouting | Masses, brutes |
| **Soldier** | TAB | 1 | 2 | Rifle all-rounder, the backbone | *Stim*: faster, costs HP | Loud | Walkers, runners | Clumped swarms, brutes |
| **Sniper** | TAB | 2 | 3 | Very long range, slow, huge damage | *Headshot* | Very loud | Brutes, spitters, screamers | Anything that reaches it |
| **Flamethrower** | TAB (Lucifer) | 2 | 3 | Short cone, burns the ground | *Burn ground* (also removes creep) | Loud | Swarms | Spitters, brutes |
| **Medic** | SC | 2 | 2 | No attack, heals infantry nearby | *Restoration* | Silent | – (support) | Being targeted |
| **Machine gunner** | – (mech) | 3 | 5 | Walker with a gun that **spins up** (25 % → 100 % in ≈ 3 s) | *Keep spinning* (energy + constant noise) | Very loud | Large hordes, holding a line | Brutes, surprises |
| **Siege tank** | SC | 3 | 6 | Crude artillery, **only shoots sieged** (≈ 4 s to siege), minimum range | *Siege / unsiege* | Very loud | Hordes at range, nests | Anything up close |

**In the prototype:** soldier, ranger, sniper, medic (beam only, no healing yet), no abilities. Values in
`data\units.json`.

- **Upkeep instead of supply** (like TAB): every unit costs gold once and then every minute; **no supply cap**. Army
  size = income. **Dismiss** stops the upkeep.
- **Debt** (open question 12): gold can go below 0. 0 = nothing new; debt > 0 = −15 % damage and attack speed;
  > 1 min of upkeep = −30 % and slower; > 2 min = every ≈ 10 s one unit **deserts**, weighted
  `upkeep × (3 if not a veteran, else 1)`. Paying it off removes the penalties; deserters don't come back.
- **Army size to plan for:** ≈ 50–120 units (likely 60–80).
- **Noise per unit** links the army to 8.3: rangers can go out quietly, snipers and tanks give themselves away.
- **Area damage hits everyone:** the player's splash/fire/mines hurt your own units and buildings; zombie area damage
  hurts other zombies (exceptions ❓).
- **Look:** TAB's roles with a StarCraft-*like* look of our own (era: open question 21) – ranger like a Reaper without
  jetpack, soldier like a makeshift marine, sniper like a Ghost, flamethrower like a Firebat, medic in white, machine
  gunner a small two-legged walker, siege tank patched together. Swappable PNGs in an `images` folder.

### 7.3 Veterancy

- Killed zombies give XP (walker 1, brute 10, giant 100 ❓), **split among all your units nearby**.
- Enough XP → **veteran** (the only level): more HP and damage, a visible mark.
- Veterans count as half a share, so fresh units next to them still level at a sensible pace:
  `XP × own share / sum of shares`.
- Makes losing a squad hurt; veterans are less likely to desert (7.2).

---

## 8. The horde

### 8.1 Zombie types

| Zombie | Behaviour | Why it exists |
|---|---|---|
| **Walker** | Slow, weak, *lots* of them | The mass. Tests your walls. |
| **Runner** | Fast, weak | Punishes gaps and lone units. |
| **Brute** | Slow, huge HP, smashes walls | Needs focus fire (snipers). |
| **Spitter** | Ranged acid, outranges short-range units | Forces you out instead of turtling. |
| **Screamer** | Calls nearby zombies when it sees you | Kill it first! |
| **Swarm** | Tiny, fast, many, explodes on contact | Counters clumped armies. |
| **Giant** (like TAB's) | Rare, huge, slow, smashes through walls and buildings | Late waves and the final wave. No other bosses. |

**In the prototype:** walker, runner and **fat** (TAB's "chubby": big, 600 HP) – values in `data\zombies.json`.

- **Sight** like TAB, per breed: 5–6 tiles, not through forests. **Hearing** 3 tiles. **Shot** → storms for 5 s.
- **Noise** of every shot wakes all zombies around (ranger 1, soldier 3, sniper 10 tiles) for 20 s – the local part
  of 8.3.
- Having noticed a unit, it storms at it: around water and mountains, but **almost straight through walls** (tears
  them down). A shot zombie and its 3 neighbours look for the gap for 10 s instead.
- Without a unit: the nearest building within 5 tiles, else the command center (buildings in the way only avoided if
  the detour is short).
- Sight is computed from the few units outwards, not from thousands of zombies.

### 8.2 Waves

- **Ambient:** zombies roam from the start (more and tougher further out).
- **Noise waves:** your noise attracts groups (8.3).
- **Scripted waves:** on fixed days, growing, from an *announced* direction. Planned as a share of the round (4.1).
- **Final wave:** on the last day, everything incl. giants; countdown visible, direction announced ≈ 2 days ahead.
- **In the prototype:** waves planned from the days (`data\spawning.json`), final wave from all sides with fat, a
  constant spawn from day 2. Killed zombies never come back.

### 8.3 Noise (Factorio's pollution × TAB's noise)

- Every machine, tower shot and explosion makes **noise**, spreading per map chunk like Factorio's pollution.
- Zombies and nests "drink" it: enough noise → they move towards its source.
- → Expanding makes you stronger **and** louder. Quiet tech (wind, solar, dampeners) is a real choice.
- Noise overlay like Factorio's pollution map.

### 8.4 Creep (like StarCraft's, but for the zombies)

- **Nests** spread creep around them, growing over the round.
- On creep: no building, your units slowed, zombies regenerate.
- Burning it (flamethrowers, cleanser) removes it; destroying the nest stops it.
- Gives the army an offensive job.

---

## 9. Map generation

A mix of Factorio (noise terrain, resource patches) and TAB (regions, points of interest).

### 9.1 Steps

1. **Terrain** from noise → grassland, forest, rock, swamp, water, ruins.
2. **Start area:** safe-ish, with wood, stone, small iron + coal patches, farmland, one oil node.
3. **Resource patches:** Factorio-style, richer and larger further out.
4. **Chokepoints:** rivers, cliffs, forests – obvious places to wall off.
5. **Nests:** away from the start, more towards the edges.
6. **Points of interest** (TAB's villages): guarded, reward = gold, items, a unit, a blueprint.
7. **Final wave direction** chosen at map creation.

**In the prototype:**

- **Generators:** Wild (Factorio-like noise), Regions (TAB-like, walls with passes), Rings, Canyons, Blank – each with
  its own sliders. Same seed + same values = same map.
- **World border:** a ring of 16 tiles of open ground outside it. Units and the camera stop at the border.
- **Spawners:** zombies only come out of them – 3 per side in the ring, covering nearly the whole side. Mode per
  spawner: constant / periodic / off. Nests and wave directions can become spawners later.
- **Ways in:** winding open lanes from every side to the base that join like branches – the places to wall off.
  Fords in rivers, passes in region walls. A safety net connects anything cut off.
- **Ore:** iron ore, coal, stone patches; one of each next to the base, bigger and richer further out.
- **Oil (2026-10-18):** nodes of exactly **2 × 2 tiles** (an oil pump goes on one), never touching. One alone next to
  the base, further out groups of 2–6.
- **Seed:** truly random per new map, `--seed N` on the command line, kept by "Restart".

### 9.2 Map sizes (first guess)

| Size | Tiles | Zombies at peak |
|---|---|---|
| Small | 128 × 128 | ~1,000 |
| Medium | 192 × 192 | ~3,000 |
| Large | 256 × 256 | ~6,000 |

- **Fog of war:** unexplored (black) / explored (dark, no zombies shown) / visible. Sight per unit and building,
  blocked by forests and trees, computed in the simulation (co-op players share it).

---

## 10. Home screen and meta progression

### 10.1 Home screen (TAB-like)

- A **world map** of **regions** (each a biome with its own threats).
- Region → **round setup:** map size, zombie density, final wave strength, **number of days**, starting resources.
  Every setting changes the **score multiplier** (TAB's %); fewer days = harder.
- Buttons: *New round*, *Continue*, *Regions*, *Research*, *Records*, *Settings*.

### 10.2 Meta progression (like TAB, but without a campaign)

- **Research points** after every round (score-based, a few for a loss too) → persistent **research tree**.
- **Regions unlock** by winning the previous one at ≥ a certain difficulty.
- Records per region: best score, fastest win, highest difficulty.

### 10.3 Other modes

- **Endless** (nice to have): the round goes on after the final wave.
- **Co-op** (wanted): one player hosts, see 12.8.
- No campaign, no daily challenge.

---

## 11. UI and controls

### 11.1 Screen layout

**The bottom bar like TAB's (2026-10-18)** – no top bar. From left to right: minimap · clock (day, hour, pause button,
speed, next wave) · control groups · selection (portrait, name, life bar, text, values as small boxes) · command card
(5 × 3, build menus) · stock (colonists, workers, food, energy; gold, wood, stone, iron, oil). Placeholder look in
brass and dark wood until the real art (12.3). Later: noise/creep overlay on the minimap.

### 11.2 Keys

| Key | Action |
|---|---|
| Left click / drag | Select / box select (Shift add/remove, on nothing = deselect) |
| Right click (drag) | Smart command: move, attack, follow, enter a tower (drag = formation) |
| T, P, M, S, H | Attack-move, patrol, move (wait for a click), stop, hold. Attack is T so that A stays free |
| Shift + command | Queue at the end of the chain |
| Q W E … (grid) | Command card, StarCraft grid layout. The command center's build menus: Q W E / A S D / Y |
| Ctrl / Shift / Ctrl+Shift + 1–0 | Set / add to / steal control group; 1–0 select, twice = camera |
| Ctrl + click, double click | All of that type on screen |
| F2 | Whole army |
| F5–F8 (Ctrl: save) | Camera bookmarks |
| Backspace | Camera to the base |
| Space | Pause (orders still possible, singleplayer only) |
| Esc | Cancel → deselect → game menu |
| Tab | Next type in the selection (planned: noise/creep overlay elsewhere) |
| WASD / arrows / edge / wheel / middle mouse | Camera. **WASD only with nothing selected** |

- **Every key will be changeable** in the settings (11.4).
- macroquad reads keys by their **US position**: on a German keyboard the grid key "Y" arrives as Z (12.2).

### 11.3 Feedback

- Alerts with jump-to-location (*"Building under attack"*, *"Wave incoming from the west"*).
- Edge-of-screen arrows towards fights, clear sound cues per event.

### 11.4 Settings in one JSON file

- **`%APPDATA%\Last Bastion\settings.json`** (open question 16) – a fixed place, so the toolbox finds it too. Saves go
  next to it. A button *Open settings folder* makes it easy to find.
- Planned values: display, window mode, resolution, FPS limit/vsync, volume, UI scale, edge scrolling, language, key
  bindings (as action → key name, e.g. `"attack_move": "T"`).
- **Never crash on a bad file:** missing values = defaults, unknown ones ignored.
- **In the prototype:** window mode, edge scrolling, scroll speed, confine mouse, health bars, full command chain,
  dev overlay. Key bindings still missing.

### 11.5 Save and load (like Factorio)

- Save freely (named slots), **autosave**, **quick save / load**.
- **Load screen:** name, date, time played, day X of Y, region, difficulty, screenshot.
- Saves in `%APPDATA%\Last Bastion\saves\`. *Ironman* (one save, like TAB) maybe later.
- Co-op: everyone can save, only the host loads (12.8).
- ❓ **One game = one file:** every round already has its file in `maps\` (the round as it starts). Mid-round saving
  could go into that same file, "Save as" for extra copies. To decide at step 10.

---

## 12. Technical notes

### 12.1 The hard parts

| Problem | Solution |
|---|---|
| **Thousands of zombies pathing** | **Flow fields** – one per target, every zombie follows it |
| **Collisions between thousands** | **Spatial grid** – only check neighbours |
| **Drawing thousands of sprites** | Batching in texture atlases, only what's on screen |
| **Group movement** | Steering + formations |
| **Path-finding for your own units** | A small flow field per move order + steering |
| **Determinism** (seeds, replays, co-op) | Fixed ticks, own seeded random generator, rules in 12.8 |

- **In the prototype:** one **line-of-sight flow field** per target (Dijkstra, 8 neighbours, each tile keeps the
  furthest visible "anchor" like Theta\*) – any angle, taut around corners, 2–4 ms per field.
- **Options for later:** Theta\* for single units, HPA\*/flow field tiles for big maps, a density cost field or
  continuum crowds so the horde flows around jams, RVO/ORCA for smoother crowds at gates.

### 12.2 Language and library

- **Rust** (open question 1) – learning it is a goal of its own. No Python (too slow for the horde). C++, C# and C
  were the alternatives.
- Think in data: objects refer to each other by ID, plain `Vec`s of structs, no ECS library for now.
- **macroquad** (open question 17): fastest start (only `cargo`), batches sprites by itself. Drawing stays behind a
  thin layer of our own, so SDL3/wgpu could replace it later.
- **Throwaway prototype (2026-10-15):** 10,000 zombies at 59 FPS, 2.1 ms simulation per tick – plenty of room.
- **What macroquad can't do:**
  - fullscreen only borderless on the main monitor, no monitor list, vsync only a hint;
  - keys by **US position** (good for key grids, but bindings must show the player's own labels ❓);
  - Alt + key probably beeps → no Alt shortcuts;
  - no "mouse left the window"/focus event → solved with own Windows calls in `src\cursor.rs` (focus, real mouse
    position, mouse kept in the window like StarCraft 2);
  - borderless = a window exactly the size of the screen → Windows/driver take it for exclusive fullscreen, popups
    (screenshot tools) make it blink → `cursor.rs` keeps it one pixel taller.
- **Setup:** `winget install Rustlang.Rustup` + Visual Studio with "Desktop development with C++" (linker, SDK).

### 12.3 Graphics: the TAB look

- **Isometric like TAB** (open question 2): pre-rendered sprites on a square grid, 8–16 directions per animation.
- Texture atlases (macroquad batches them), depth by sorting on the feet's y, fog/light/fire as render textures +
  shaders, colour grading, own particles. The HUD is custom-made.
- **The hard part is the art:** animations × directions per unit, usually Blender + a render script. Where the
  pictures come from is open (question 22). Placeholder shapes until then.
- *In the prototype:* sprite sheets in `assets\` (one row per direction, idle/walk/attack side by side, feet as
  anchor, mipmaps), named per kind in `data\`; a generated grey test figure. Buildings, corpses, deaths: still shapes.
- Performance is decided by the horde, not by units or colonists.

### 12.4 Data-driven from the start

- **JSON files in `data\`, no database** (open question 14): read once into Rust structs – as fast as numbers in the
  code, readable, easy to compare in git.
- Balancing = editing numbers. A broken file keeps the old values and shows a warning.
- **Hot reload** while running ❓ – singleplayer only, or sent as an order in co-op (12.8).
- **Economy check** over 40–80 days: a small calculation over the same files instead of a spreadsheet (still to do).

### 12.5 Simulation and drawing apart

- Logic in **fixed ticks** (20 per second), drawing interpolates in between → pause, slow-mo, fast-forward for free.
- Same seed + same orders = same result: bugs repeatable, base for co-op and replays.
- The logic works on a plain **2D grid**; isometric is only drawing.
- The whole state must be saveable as plain data (11.5); flow fields are recalculated.

### 12.6 Developer tools

A 2–4 hour round is far too slow to test by playing.

- **Planned:** jump to day X, speed ×4+, give gold, spawn zombies, start a wave, reveal the map, very short test
  rounds, debug views (flow fields, grid, noise), hot reload.
- **In the prototype:** speed ×1–×8, F3 overlay (FPS, ms per tick, checksum, switches, key help – sections fold with a click), `--seed N`, an automatic balancing run
  of the mini round, PNG previews of the generators.
- **Map editor** (`LastBastionEditor.exe`, own program since 2026-10-17, like TAB's): opens a map file, paints terrain,
  ore and oil, places units, buildings, zombies and spawners, edits the waves and the round, **Edit** (no time) /
  **Test** (the round runs, afterwards the map is as before). Every change is an order, so it stays deterministic.
- **One game = one file:** every new round writes its map file to `%APPDATA%\Last Bastion\maps\`; "Restart" reads it.

### 12.7 Repository and toolbox

- **Own git repository**, separate from the windows-toolbox. The toolbox only has a small page that starts the game.
- **Keep stable:** the game at **`dist\LastBastion.exe`**, the design at `DESIGN.md`, settings at a fixed place (11.4).
- **Fresh PC:** clone, rustup, Visual Studio with C++, `.\build.ps1`.

### 12.8 Co-op: one player hosts

No servers of our own: the host's game **is** the server.

#### Lockstep (like StarCraft and Age of Empires)

- Only **orders** go over the network, not the state of thousands of zombies. Every PC runs the same simulation.
- The host collects the orders per tick and sends them to everyone. An order runs a few ticks later (**input delay
  ≈ 100–200 ms** ❓, adapted to the ping); the click marker and the unit's answer come at once. Singleplayer has no
  delay.
- Late orders → everyone waits briefly (stutter, no drift). Very little data.

#### The price: strict determinism

Same seed + same orders → **the same state, bit for bit**. Rules for the simulation code:

- Randomness only from the simulation's seeded generator; eye candy uses a second one.
- Everything units decide by themselves (targets, firing, zombie movement) is simulation – worked out on every PC,
  nothing sent.
- Fixed processing order, ties go to the lower ID. Never iterate a `HashMap`/`HashSet` in the simulation.
- Animations are only drawing; the simulation decides when a shot hits.
- Floats are fine on the same `.exe`; `sin`/`cos` & co. from `libm` (the same everywhere).
- Player orders reach the simulation only through the order list – singleplayer too.
- **Desync check:** checksums of the state are compared every few seconds; a test runs the same seed twice.

#### Connecting without servers of our own

- **Direct IP / Tailscale** during development, **Steam networking** if released (open question 20). Epic Online
  Services as an alternative.
- The network code sits behind a thin layer (*send orders*, *receive orders*), so the way of connecting can change.

#### What co-op changes in the game

- **2 players, one colony, both control everything** (open question 19). One colony each maybe later.
- **Hands off what your ally uses:** a unit belongs to a player while it's in their control group or carrying out
  their order; idle and in no group → free again (open question 25).
- **No pause**, speed 1×; the Esc menu doesn't stop the game.
- **Everyone can save** (and continue alone), **only the host loads**. A dropped player can rejoin; if the host
  leaves, the round ends.

---

## 13. Prototype roadmap

Riskiest and most fun-critical first. Each step ends in something playable.

**Before step 1 – done:** language, library, co-op yes, saving yes, who builds, repository, build on a fresh PC.
**Still open:** fixed tick + seeded random generator are in, but the **economy check** (5.3, question 14) waits until
before step 6.

| Step | Goal | Done when | Status |
|---|---|---|---|
| **1. Unit control** | Select, groups, commands, one ability | Moving 10 units *feels good* | All controls in, abilities missing |
| **2. Horde stress test** | 2,000+ zombies with flow fields + grid | Stable 60 FPS | ✓ (10,000) |
| **3. Walls, towers, infection** | Build on a grid, zombies attack, infection | A breach feels dangerous | ✓ |
| **4. Mini round** | 10 days ≈ 30 min, tents pay gold, 3 unit and zombie types, waves + final wave | **Is it fun?** | Playable – playing and tuning left |
| **5. Co-op mini round** | 2 players, direct IP, orders per tick, desync check | No desync to the end | |
| **6. Production chains** | Drill → stock → smelter → workshop, with power | A small chain runs by itself | |
| **7. Houses, gold, upkeep** | Houses consume and pay, units cost upkeep | Economy and army linked | |
| **8. Map generation** | Terrain, patches, nests, POIs, fog | Two seeds feel different | Partly (generators, ore, oil, fog) |
| **9. Noise and creep** | Noise attracts waves, nests spread creep | Expanding is a decision | Local noise only |
| **10. Full round** | Section 4 timeline, save and load | A 40-day round playable | |
| **11. Home screen and meta** | Regions, setup, score %, research | "One more round" | |
| **12. Polish** | Sounds, effects, alerts, balancing, tutorial | Someone else can play it | |

---

## 14. Open questions

| # | Question | Status |
|---|---|---|
| 14 | Does the economy work over 40–80 days? | Check by a small calculation over `data\` – still to do |
| 17 | Library? | macroquad in use and fast enough; the feel of the controls still to judge |
| 18 | How long is a day? | First guess 3 min, decide by playtesting |
| 21 | Which era / technology? | Open – decides the look, towers, air units |
| 22 | Where do the pictures come from? | Open – Blender / packs / artist / AI |
| 26 | Ammo after all? (re-opens 6) | Open – munitions factory planned just in case |
| 27 | Two tesla towers (power pole vs. weapon)? | Open – one needs another name |
| 28 | What gets iron ore and coal out of their patches? | Open – a mine/drill in the resources menu? |
| 29 | What do the Inn and the wonders do? | Open |

**Decided** (number = question, referenced elsewhere):

- **1** Rust · **2** isometric, logic on a square grid · **3** co-op, one player hosts, lockstep · **4** supply by
  market radius, no belts · **5** TAB-style building, no workers · **6** ammo free for now (see 26) · **7** active
  pause, singleplayer only · **8** hero parked · **9** saving like Factorio · **10** research tree like TAB ·
  **11** name *Last Bastion* · **12** debt in stages, then desertion · **13** the TAB side is the heart ·
  **15** own repo · **16** settings in `%APPDATA%\Last Bastion` · **19** one shared colony · **20** direct IP /
  Tailscale, Steam if released · **23** first power: command center + wood generators · **24** colonists are a
  resource · **25** a unit is free again when idle and in no control group.

---

## 15. Idea parking lot

Ideas that don't serve a pillar yet, or are too big for now.

- **Day/night cycle:** more zombies at night, lights cost energy.
- **Weather:** fog cuts sight, storms knock out solar.
- **Trains** between distant ore fields, **drones** delivering items.
- **Refugees** mid-round – maybe infected. 😈
- **Mutations:** a random horde modifier per round.
- **Editor triggers** and scripted events like StarCraft 2's.
- **Mod support** via the JSON files. **Replays** (seed + orders, almost free with lockstep).
- **Air units** (until the era is clear – a hot-air balloon would fit low tech).
- **Rally point into a control group.**
- Taken out: belts, the crawler, scout/engineer/grenadier units, the hero (question 8).
