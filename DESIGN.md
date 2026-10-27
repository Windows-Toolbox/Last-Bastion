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
- **In the prototype:** picked on "New game" like TAB – 10 / 20 / 40 / 60 / 80 days (`data\game_durations.json`), each
  with its own wave rhythm and score %.

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
| Raw | **Wood** | Sawmill (forest) | Early buildings, walls, charcoal burner |
| Raw | **Stone** | Mine on a stone patch (question 28) | Barracks, walls, towers |
| Raw | **Iron ore** | Mine on an iron patch | → Iron |
| Raw | **Coal** | Mine on a coal patch | Power plant, smelter fuel |
| Raw | **Food** | Hunter cottage (forest), fishing hut (water), farm (grass – nothing grows on dirt) | Houses |
| Raw | **Oil** | Oil pump on an oil node (2 × 2, 2026-10-18) | Oil burning generator, later more ❓ |
| 1 | **Iron** | Foundry (ore + coal, decision 32) | Buildings (the machine gun tower first), tools |
| 1 | **Energy** | Command center (a little), charcoal burner (early, burns wood), windmill (big, costs upkeep), coal plant / solar later | Machines, houses, towers |
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
- **The market** (2026-10-21, not TAB's): no supply radius houses depend on. Founded early, it grows with the colony –
  houses whose middle lies within 2 tiles of it pay +50 % gold (one market per house), and it sells what lies above
  80 % of the storage, 1 good per 4 colonists of its houses per payout. A dense, rich centre – and a juicy target for
  an infection chain (pillar 1).
- **Rough check ❓:** an army of 80 units at ≈ 3 gold upkeep each costs ≈ 240 gold/min. That's 24 apartments or 120
  tents. When in the round should the player get there (tier 2 from ≈ 30 % of the round, section 4.2), and does
  that work for 40 days as well as for 80? Check it in a spreadsheet before writing code (open question 14).

### 5.4 Logistics (no belts)

- Like TAB: what gathering buildings produce goes straight into the **colony stock**. Production buildings take their
  inputs from it and put their outputs back. No belts, no item routing.
- The Factorio part is in the **production chains** (ore + coal → iron → tools …) and the **power grid**, not in
  moving items around.
- Warehouses raise the storage limit of the goods, banks the gold's; markets sell the surplus (5.3). Two storage
  types are enough – a third only with its own mechanic (an oil tank, when oil comes).

### 5.5 Power

- Power poles connect machines into a grid (Factorio).
- Not enough power → every machine on that grid slows down proportionally. Towers and houses on the grid suffer too.
- Destroyed poles split the grid → a zombie breach in one corner can black out half the base. (Pillar 1.)
- The poles are **tesla towers**: each covers a circle (like TAB's energy area) – a building that makes, takes or carries
  power has to stand in one. Two circles where one tower stands in the other's are one grid.

**In the prototype:** command center (+20, reaches 8 tiles like TAB's), tesla tower (reaches 7), windmill (+50, 8 gold upkeep –
few big mills instead of TAB's mill farms), charcoal burner (+20, burns 1 wood per payout). Like TAB, nothing that
takes power is built (or switched on again) without that much to spare in its grid. Grids and shares in `src/sim/economy.rs`. Short of power: gathering,
the tents' gold and training slow down by the grid's share. Towers don't use power yet.

### 5.6 Colonists

Colonists exist twice: as a **resource** (a number) and as **people walking around** (only for the eye).

#### Colonists as a resource (open question 24, 2026-10-15)

- Houses give **colonists** – they pay gold (5.3) – and **workers**, like TAB (a tent 4 colonists and
  2 workers). The command center gives some workers too (the founders, enough for the start units and a little more).
  The bottom bar shows colonists and free/all workers next to the gold (11.1).
- **Every other building and every unit takes workers** for as long as it stands or lives (`worker_cost` in
  `data\`, 2026-10-19). No free workers → **nothing new** can be built or trained. Walls take none for now – a wall
  line would eat all of them.
- **Later, when workers get lost** (an infected house) and there are fewer than in use, buildings are **switched
  off** until there are enough again (2026-10-15):
  - **Gathering buildings first** (sawmill, mine, hunter …), **spread over the resources**: one of each kind in
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
| **Gathering** | Hunter cottage, fishing hut, sawmill, mine (every ore patch: iron, coal, stone – no quarry), oil pump, farm | Work the fitting tiles in a square: their site plus `gather_range` tiles on every side (like TAB: the mine 2 → 6 × 6, 2026-10-21); two of a kind share the tiles they both reach. With `gather_below` the tiles under it count too: the mine stands on the ore and digs it (like Factorio's drill) – not like TAB, where ore lies in rock. A later stone mine can simply get a bigger range |
| **Production** | Foundry (smelter), wood workshop, stone workshop, lab | Take inputs from the colony stock (5.4). The workshops make nothing yet: like TAB's, they unlock buildings (wood → stone → munitions) |
| **Logistics and trade** | Warehouse, market, bank, road | Warehouse = storage; market = houses around it pay more, sells the surplus (5.3); bank = gold storage + credit (Dispo, 7.2); road = **units and zombies** faster |
| **Power** | Tesla tower (pole), charcoal burner (wood), windmill, oil burning generator, coal plant, solar, battery | Charcoal burner early, eats wood; windmill big but with upkeep; solar quieter but weaker |
| **Military** | Barracks, munitions factory, inn, siege factory (vehicles), tech lab | Barracks from the start. The munitions factory is a tech gate for firearms (question 26) and makes explosive barrels; the inn hires mercenaries (7.2); the foundry later the second weapon tier (MG tower, vehicles) |
| **Defence** | Wall, gate, tower (4 units inside), stone wall, stone tower, stake trap, explosive barrel, ballista tower, MG tower, flame tower | Stone ones go on the wooden ones (upgrade). Stake trap like TAB's: walked over, hurts zombies, wears out. Barrel (instead of mines, 2026-10-21): any damage sets it off, blasts chain, hurt everyone, loud; set like a trap or carried by units |
| **Special** | Radar, sound dampener, cleanser (removes creep/noise) | No menu yet. The command center scans like StarCraft's (2026-10-21). The beacon is out: a free, endless lure into the kill zone is too strong – a barrel lures once and costs |
| **Wonders** | ❓ (29) | Most expensive, very strong buffs |

- **Building like TAB** (open question 5): placed, paid at once, builds itself. No worker units.
- **Infection** (from TAB): HP + shield; shield down = **infected** – its people come out as zombies, like TAB:
  colonists + workers it needs − workers it gives, plus its `extra_infected` (tent 2 + 2, stone house 8 + 8), each one
  of TAB's infected at random (3/8 walker, 4/8 runner, 1/8 riot), all heading for the command center like a wave.
  Still being built, it falls. Your fallen units rise the same way. A breach should feel catastrophic. The infected building then belongs to the zombies: they stop
  hitting it, and its walls are broken – zombies and units walk right through it (changed 2026-10-18, before: around
  it). **Repairing** it cures it. A building with **workers** that falls is infected too: its workers come out as
  zombies where it stood. Every infection is very loud (8.3) – an outlying hunter cottage overrun draws the horde.
- **Repair** (2026-10-18): F on any selected building, **X / "Repair all"** for every building (X only while no
  units are selected – then it's the Line formation). Costs its share of the building's `repair_cost` (gold now,
  later also wood/stone …), paid at once; the damage comes back over `repair_seconds` (full repair 10 s). Not enough
  gold: cheapest first, as many as it pays for. Not with a zombie in the build zone (`can_repair` in
  `placement.rs`), later not on creep (8.4).
- **Demolish** (Del): any selected building but the command center, gone at once – part of its price comes back
  (`demolish_refund_percent` 25 %, still being built `demolish_unfinished_refund_percent` 75 %, `data\buildings.json`;
  as far as I know TAB gives nothing back). Not an infected one – it belongs to the zombies. In the editor: removed,
  nothing back.
- **Roads:** units **and zombies** are faster on them – a road into the base is also a fast lane for the horde.
- **Build menus (2026-10-18):** one per key on the command center, like TAB's categories – **Q** colonists,
  **W** resources, **E** energy, **A** industry and trade, **S** military, **D** defence, **Y** wonders. Planned
  buildings already sit on their spot, greyed out.

**In the prototype:** command center, tent, barracks (the only building that trains), wall, gate (3 × 1, door for
your units), tower (up to 4 units, +2 range and sight, can't be hit), sawmill, mine, hunter cottage, fishing hut,
farm (3 × 3, only on grass: plants the grass around it, dirt and ore give nothing), windmill, charcoal burner, tesla
tower (the mine and the tesla tower need no workshop, like TAB – the barracks needs stone, 2026-10-24), wood
workshop (unlocks farm, windmill, stake trap, ballista, stone workshop), stone workshop (unlocks munitions factory,
foundry, stone house, stone wall, stone tower), munitions factory (unlocks soldier and sniper – the ranger's bow needs
nothing; makes barrels), **foundry** (2026-10-24: unlocks the machine gun tower, smelts 2 iron ore + 1 coal into 1 iron,
up to 3 per payout), stone house, stone wall, stone tower, stake trap, market, bank, inn (mercenaries), explosive barrel,
the command center's scan (`src/sim/market.rs`, `debt.rs`, `blast.rs`, `scan.rs`), and the towers that shoot by
themselves (2026-10-24, `src/sim/towers.rs`, a `weapon` block each): **ballista** (a heavy bolt at the toughest zombie in
range, no power, no iron) and **machine gun tower** (fast fire at the nearest, power, 20 iron). Start (2026-10-20,
different from TAB): 4 rangers, 1 soldier, 100 gold, 20 wood. Prices in gold plus wood/stone (barracks 150 + 20 wood + 20 stone,
wood workshop 40 wood). Every full hour: burners take wood, gatherers deliver, tents eat 4 food, soldiers 1 – fed, they pay gold
(× power share), hungry, nothing. Placement rules all in `src/placement.rs`:
drag a line (walls diagonal, 1 wide like TAB), gate over walls (walls refund 50 %, never a saving), 1 free tile between
towers, no zombie within 4 tiles. Your own units may stand on the site – they're pushed off to the nearest free
spot (like StarCraft). Costs gold and workers (5.6). **Build time** like TAB (2026-10-24, `build_seconds` per building):
short, longer the bigger and the higher in the tech tree; it starts at 10 % of its hit points and grows, gets its shield
when finished, does nothing until then (no workers given, no gold, power or training) but takes its workers already and
can be hit. Repair (F) on every building, cures infected
ones.

---

## 7. Units and control

### 7.1 Control (the StarCraft part)

Pillar 2. The heart is still the TAB side (open question 13).

- Box/click select, double click = all of that type on screen, Shift = add/remove. Control groups 1–0.
- **Attack-move**, move, stop, hold, patrol, **follow** (move onto an own unit). Shift queues every command;
  letting go of Shift takes a waiting command off the mouse, pressing Shift again brings it back.
- **Hold** only means "don't walk by yourself": holding units can still be pushed by your units, and a squeezed crowd
  pushes itself apart. Against zombies a holding (or aiming) unit stands firm – TAB's rock (8.1).
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
| **Spearman** | – (2026-10-24) | 1 | 1 | Cheap melee, every hit pushes back | ❓ | Nearly silent | Walkers and runners at a gate | Spitters, infested, giants |
| **Soldier** | TAB | 1 | 2 | Rifle all-rounder, the backbone | *Stim*: faster, costs HP | Loud | Walkers, runners | Clumped swarms, brutes |
| **Sniper** | TAB | 2 | 3 | Very long range, slow, huge damage | *Headshot* | Very loud | Brutes, spitters, harpies | Anything that reaches it |
| **Flamethrower** | TAB (Lucifer) | 2 | 3 | Short cone, burns the ground | *Burn ground* (also removes creep) | Loud | Swarms | Spitters, brutes |
| **Medic** | SC | 2 | 2 | No attack, heals infantry nearby | *Restoration* | Silent | – (support) | Being targeted |
| **Machine gunner** | – (mech) | 3 | 5 | Walker with a gun that **spins up** (25 % → 100 % in ≈ 3 s) | *Keep spinning* (energy + constant noise) | Very loud | Large hordes, holding a line | Brutes, surprises |
| **Siege tank** | SC | 3 | 6 | Crude artillery, **only shoots sieged** (≈ 4 s to siege), minimum range | *Siege / unsiege* | Very loud | Hordes at range, nests | Anything up close |

**In the prototype:** soldier, ranger, sniper, medic, spearman, no abilities. Values in `data\units.json`. Firearms
(soldier, sniper) only with a munitions factory standing (`requires`); the ranger's bow, the spear and the medic need
nothing.

- **Medic (2026-10-24):** heals with its beam (4 HP/s), but only a unit that hasn't been hit for 3 s – not StarCraft's
  medic who keeps a whole army standing in the middle of a fight: the wounded have to be pulled back.
- **Spearman (2026-10-24):** the early melee unit next to the ranger – cheap, short reach (1.2 tiles), every hit pushes
  the zombie back (less a big one), **nearly silent** (pillar: noise – 2 per hit: a little more than a zombie's hit,
  less than a rifle). Holds a gate or a line early on; spitters and infested
  shoot it to pieces.

- **Upkeep instead of supply** (like TAB): every unit costs gold once and then every minute; **no supply cap**. Army
  size = income. **Dismiss** stops the upkeep. Running buildings cost upkeep too (TAB's values); **switched off (Y)**
  they cost nothing – no upkeep, workers or power – and do nothing.
- **In the prototype:** upkeep per payout (`upkeep` in `data\`), debt and desertion as below (`src/sim/debt.rs`, values
  in `data\colony.json`). **Dismiss** (Del, 2026-10-24): the units leave – no gold back.
- **Debt** (open question 12): gold can go below 0. 0 = nothing new; debt > 0 = −15 % damage and attack speed;
  > 1 min of upkeep = −30 % and slower; > 2 min = every ≈ 10 s one unit **deserts**, weighted
  `upkeep × (3 if not a veteran, else 1)`. Paying it off removes the penalties; deserters don't come back.
- **Credit (Dispo, 2026-10-21):** every bank gives 500 – debt up to it brings no penalties, only what's beyond counts
  for the stages; it costs 10 % interest per payout. Buying never slips into it by itself: 0 is the limit, unless gold
  is all that's missing and Shift is held. A bank that falls takes its share of that debt with it (and the gold above
  the smaller storage).
- **Mercenaries (the inn, question 29):** a stronger take on the veteran base units – own look, 2.5 × the price,
  double upkeep, **no workers**: their limit is the economy, nothing else. They work for gold: in debt they desert
  first, already at the second stage. Losing the inn costs nothing but the place to hire them.
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
- **In the prototype (2026-10-24, `src/sim/veterans.rs`):** walker 1, riot/spitter/infested/harpy 3, fat 10, giant
  100 XP, shared within 8 tiles of where the zombie fell; at 20 a unit is a veteran: +25 % HP, +20 % damage, a silver
  mark. Mercenaries take no share – they are better than veterans already.

---

## 8. The horde

### 8.1 Zombie types

| Zombie | Behaviour | Why it exists |
|---|---|---|
| **Walker** | Slow, weak, *lots* of them | The mass. Tests your walls. |
| **Runner** | Fast, weak | Punishes gaps and lone units. |
| **Brute** | Slow, huge HP, smashes walls | Needs focus fire (snipers). |
| **Armored runner** (TAB's, shown as "Riot") | Fast like a runner, armor eats most of an arrow | Rangers alone can't hold it – soldiers and snipers can. |
| **Spitter** (TAB's venom) | Ranged acid over walls, lands a moment later and splashes | Forces you out instead of turtling; step aside in time. |
| **Infested** (StarCraft 2's infested marine) | Shoots back, just under a soldier's range | Your line takes fire too – no free kills from behind a wall. |
| **Harpy** (like TAB's) | Fast, hears much further than the others, leaps over thin walls; every hit it lands is loud and it adds up | First at every fight – kill it before its noise brings the rest. |
| **Swarm** | Tiny, fast, many, explodes on contact | Counters clumped armies. |
| **Giant** (like TAB's) | Rare, huge, slow, smashes through walls and buildings | Late waves and the final wave. No other bosses. |

**In the prototype:** walker, runner, **fat** (TAB's "chubby": big, 600 HP), armored runner ("Riot", armor 3),
spitter (acid 3.5 tiles, flies, splashes 1 tile – units and buildings, never zombies), infested (shoots 4.5 tiles, the
soldier 5), **harpy** (alertness 8: noise 2–4× quieter than
the others need brings it, from 32 tiles – it comes first; each hit noise 200 every 0.75 s – faster than noise fades, so it adds up; zombies' noise, no
unrest; leaps over walls and gates up to 3 tiles thick when its goal lies behind them, hits thicker ones), **giant**
(TAB's a bit smaller: 3,000 HP, armor 6, a wall in 3 hits – only a few per map, far out; how many is up to the map
(`giants`, default 4 on 256 tiles), the difficulty scales it 50–225 %; not in the waves, but the final wave calls
them)
– values in `data\zombies.json`, the names too. The harder breeds lie further out (`mix` in `data\roaming.json`)
and roam further per trip (`stroll` per breed: walker 2–8 tiles … fat 4–30) – without a home they keep to (8.5).
TAB's breeds are ours like this: WeakA/B/C → walker, WorkerA/B and MediumA/B → runner, DressedA → riot, Chubby → fat,
Venom → spitter, Harpy, Giant; the infested is ours (StarCraft 2).

**What a zombie has in mind – TAB's decision tree** (from its code, `src/sim/zombies.rs`), the first point that fits:
1. **After something** – one of your units, a building with people in it, a wall in its way: there, and hit it.
2. **Lost sight of it:** to where it was then, and stay there (not the giant).
3. **With a wave:** to the command center, tearing down what's in the way.
4. **Contagion** (every ~4 s): at least 3 wave zombies of its rank or higher on its tile – it joins the wave (not the
   giant; ranks: walker 1, runner 2, riot and harpy 3, spitter and infested 4, fat 5, giant 6). A wave snowballs
   through the roamers it walks over – sleepers too.
5. **A sound** (8.3): there, and idle around it while it's loud.
6. **Idle:** rolled with TAB's weights 1300 stand / 150 growl / 150 roam – mostly they stand.
- The **giant** also stops every 12 s and roars, whatever it's doing.
- **Looking around** (TAB's watch): every 0.5 s – twice as long while it's after something, 3× out of your sight, 5×
  where you've never been. It picks the nearest of your units and **buildings with people in them** that it sees (sight
  per breed 5–8 tiles, not through forests): the command center, houses, buildings with workers, towers with units
  inside. **Not** walls, empty towers, power poles.
- **Wall rule:** after something further than half a tile away, a wall, gate or tower on the straight line to it (in
  the square of its size + 3 tiles) becomes its target – the nearest one. Seeing nothing, it stays on a wall it was
  tearing down. Waves and lured ones check the line to their next step. Their way goes **straight through walls**:
  no zombie ever looks for a gap. The harpy has no wall rule – it leaps.
- **Shot:** if it isn't after anything, it and every zombie on its tile and the four next to it that isn't either go
  for the shooter (a unit in a tower: the tower) – once more even out of sight, then to where it was.
- **Safety:** fewer than 100 zombies left – every 2 minutes each one without a way to the command center dies.
- Sight is computed from the few units outwards, not from thousands of zombies.
- **The crowd flows like a liquid, like TAB's physics** (`src/sim/crowd.rs`): every zombie is a smooth circle with a
  weight (`mass`). Each takes the step it wants, then in 8 rounds per tick everyone is pushed out of everyone it
  overlaps – each by its share, the lighter one further – and fully out of walls, buildings and solid ground. Smooth
  circles slide off each other: the press of the ones behind goes a row deeper each round, the front gives way to the
  sides – along a wall, around a corner, through a gap (300 through a one-tile gap in 12.5 s; a stream of 200 at a
  wall spreads along its whole side, about 4 rows deep). A crowd stands a little inside itself and still.
- **Rocks (TAB):** whoever has just stopped weighs 500 times as much for 5 s – a zombie that just got to its wall
  stands firm, the ones behind slide past it. Your units too while they hold or aim, so the crowd flows around a
  shooting soldier; otherwise the crowd shoves them by their weight. A giant (heavy) walks through walkers, a fat one
  shoves them aside.
- Walking on, a zombie keeps 30 % of its last step – it doesn't turn on the spot (without it a few at the corner of a
  gap stepped to and fro). Walking to a wall to hit it, its last step is only as long as needed; to a unit it walks
  until its body touches, an arm's length within its reach.
- **Not taken over:** TAB's step rhythm (speed swings with the walk animation) – only worth it with real walk
  animations, and then only in the picture.

### 8.2 Waves

- **Ambient:** zombies roam from the start (more and tougher further out) – as many as in TAB.
- **Villages of doom and loot** (like TAB): houses whose zombies come into being in **batches** when something
  happens – a sound it hears (half a batch goes there), every 12 hours to top up its guards, from day 20 every 5 days a
  **raid** on the command center, a hit (three batches at the closest unit, every house nearby sends one along), the
  final wave. A batch ≈ the house's size (small 1, medium 2, big 5) × the population. A house shot down drops loot,
  the rest never come (9.1). Loot has zombies hidden around it.
- **Noise waves:** your noise attracts groups (8.3).
- **Scripted waves:** on fixed days, growing, from an *announced* direction. Planned as a share of the round (4.1).
  The first ones only runners, then more and more specials.
- **Final wave:** on the last day, everything incl. giants – and like TAB it **calls every zombie on the map**
  (villages and loot guards too); they all belong to it. It also **reveals the map**. Countdown visible, direction
  announced ≈ 2 days ahead.
- **In the prototype:** waves planned from the duration (on which days, `data\game_durations.json`) and the difficulty
  (how strong, `waves_percent`) – the rest in `data\spawning.json`: runners, from a share of the round on
  riots, infested, spitters and fat ones along (one group of the wave, so out of the same spawners); the final wave
  from all sides with every breed. In the editor every wave has two switches: **Final** (calls everyone, counts for the
  win) and **Reveal** – and **groups**: each from its own place (a random spawner, 1–4 random sides, or **chosen
  spawners** – the count spread over them) with several breeds, e.g. 5,000 runners out of C, D, F, G and 200 spitters
  out of B in one wave. Killed zombies never come back.
- **Constant spawn:** walkers from day 2, runners instead from day 10, specials along from day 15 on (a
  share of every group, `specials` in `data\spawning.json`). Each group comes crowded on one spot of a spawner, not
  spread along a whole side. Bigger for good with the colony's unrest (8.3). In the editor a stage can have spawners
  of its own instead of those set to "Constant".

### 8.3 Noise (TAB's noise)

- Everything that happens makes **noise** where it happens; it adds up while it goes on and fades once it's quiet.
- Zombies and villages hear it: enough noise → they move towards its source.
- → Expanding and fighting make you stronger **and** louder. Quiet tech (wind, solar, dampeners) is a real choice.
- Noise overlay like Factorio's pollution map (later).

**In the prototype (TAB's rules from its code, numbers in `data\sound.json`):** noise adds up per cell of 4 × 4 tiles;
every second every cell halves, quiet or not – a fight that goes on settles at about twice what it makes in a second.
Per shot ranger 1, soldier 3, sniper 10; a zombie hit 1, a building placed 40, a unit dying 50, a building destroyed
100, a building infected 100 + 50 per zombie coming out – a building with workers that falls is infected (TAB's overrun
hunter cottage).
- **Listening:** every 4 s each zombie (each on another tick, sleepers too) takes the loud cell within 4 × its sight
  (in x and y) it hears best – `noise × 20 × awareness / distance²` (tiles) – and comes with that many percent, 100 and
  more for sure (awareness: walker 2, runner/fat/riot/infested 3, spitter 4, harpy 8). It goes to the middle of the
  cell and idles around it while it's loud there; quiet there at its next listening, it forgets it.
- Who is after something, heads for where it lost sight of it or comes with a wave doesn't listen.
- Houses of doom and loot guards listen every second, with their own awareness (2 and 1.5).
- `src/sim/sound.rs`; the way to the noise: a small field per place, `src/lure.rs`.

**Machines and unrest (Factorio's pollution as TAB's noise):** every 10 s each working building makes its
`noise` – hunter 50, sawmill and munitions factory 100, mine and charcoal burner 150, all at once, so neighbours add up
per cell – a short burst that halves away within seconds, so far away it rarely lures anyone, close by it does.
Everything the colony makes
(shots, machines, blasts, building – not the zombies' noise) also adds to the **unrest**, which never goes down: every
50,000 of it, each group of the constant spawn brings one zombie more, for good. Luring the near zombies into the
walls is fine – the noise brings new ones from the edge; a silenced army stays worth it to the end; tearing the
factory down before the final wave undoes nothing.

### 8.4 Creep (like StarCraft's, but for the zombies)

- **Nests** spread creep around them, growing over the round.
- On creep: no building, your units slowed, zombies regenerate.
- Burning it (flamethrowers, cleanser) removes it; destroying the nest stops it.
- Gives the army an offensive job.

### 8.5 How a zombie lives – the rules

Written down on purpose: if the zombies seem to behave strangely in a test, check it against this first.

- **At the start** the roamers lie almost everywhere, in packs of 1–3 on a tile – never more than 3 (`pack` in
  `data\roaming.json`). Near the base nearly only walkers, further out the harder breeds (TAB's balance: by breed, not
  by how many). How many: the difficulty (`roaming_zombies`).
- **How close each breed lies at the start** – the difficulty's `keep_out`, in tiles from the command center (Normal:
  walkers and runners 14, riots and infested 25, spitters 30, fat ones 35; Brutal: all 14). Right at that ring they
  lie thinly, one per tile – over `ring_ramp` (6) tiles they get as many as anywhere, in full packs. None lies closer
  at the start (there it's a walker instead). **Roaming they may come closer, like in TAB** – only the giant never roams
  within 54 tiles (`roam_ring`). On maps of TAB's generator the map lays the zombies out itself, each kind from its
  distance (`medium_from` …, 9.1) – `roaming_zombies` and `keep_out` only count on ours.
- **Asleep:** whoever lies far from your eyes sleeps – only an entry per block of 8 × 8 tiles (`src/sim/sleep.rs`).
  A sleeper doesn't see and costs next to nothing. In the editor's views "All" and "Zombies" they are drawn darker; in
  the player's view you never see one.
- **Sleepers idle too:** by the same rule and dice as an awake one (below) – TAB's stand / growl / roam rolled ahead
  until it comes up roam, then a trip to a spot found the same way. Nobody sees them, so they arrive at once and then
  wait as long as the walk would have taken, plus the standing and growling. Something in the straight way (water, a
  mountain): it stays. A trip that would end near your eyes wakes it where it starts, and it
  walks the rest awake – into sight from outside, fading in. **So the zombies never run out where you are:** a place you
  emptied (a loud border lured everything around) fills up again as the sleepers around drift in – holding it stays
  work until the region around is thinned out.
- **Waking** (whole blocks, or one by one for noise):
  1. **Your eyes:** every 0.5 s every unit and building wakes all blocks within its sight **plus 8 tiles** – so they're
     awake before you can see them.
  2. **Noise:** a sound wakes a sleeper with exactly the rule and dice of the zombies awake (8.3) – it then goes where
     the sound was. A fight wakes what would have heard it anyway.
  3. **The scan** wakes its circle, **a barrel** going off wakes its radius (and then hits them).
  4. **The final wave** wakes everyone, at least 2,000 a tick, the nearest to the base first. The round isn't won before
     the last one is awake and dead.
  5. **A wave walking over them:** the contagion (8.1) wakes a sleeper and takes it along.
- **Seen:** in the picture a zombie fades in over 0.6 s when it comes into sight and fades out over 0.8 s when it
  leaves it – nobody pops up at the edge of sight (like TAB).
- **Idle, without a radius (TAB):** stand 2 s, growl 2 s or roam – rolled 1300 / 150 / 150. A roam goes on each axis
  on its own `stroll` tiles away (walker 2–8 … fat 4–30, either way – a square, not a circle), 20 tries for free
  ground on the playing field, at its walking pace. **No home it keeps to:** trip after trip it can get anywhere.
- **Calm is cheap:** an idle one thinks only every 8th tick (it still looks around on its own beat) – calm crowds may overlap a little
  while they stroll. Only with a busy zombie on its tile or the tiles around does it move every tick: then it can be
  walked into and pushed (8.1).
- **Falling asleep again:** an idle one that has been calm for **20 s** and is further than sight **+ 16 tiles** from
  every eye falls asleep where it stands (its hit points kept) – and idles on asleep. Never after the final wave's
  call. Far away nothing costs anything, but the horde keeps moving.
- **Lured** zombies go there and then idle there: to the place they lost sight of a unit always, to a sound only
  while it's loud there (8.3); **wave** zombies head for the command center. Neither falls asleep while busy.

---

## 9. Map generation

A mix of Factorio (noise terrain, resource patches) and TAB (regions, points of interest).

### 9.1 Steps

1. **Terrain** from noise → grassland, dirt (no farms), forest, rock, swamp, water, ruins.
2. **Start area:** safe-ish, with wood, stone, small iron + coal patches, farmland, one oil node.
3. **Resource patches:** Factorio-style, richer and larger further out.
4. **Chokepoints:** rivers, cliffs, forests – obvious places to wall off.
5. **Nests:** away from the start, more towards the edges.
6. **Points of interest** (TAB's villages): guarded, reward = gold, items, a unit, a blueprint.
7. **Final wave direction** chosen at map creation.

**In the prototype:**

- **Generators:** **TAB** (the default), Wild (Factorio-like noise), Regions (TAB-like, walls with passes), Rings,
  Canyons, Blank – each with its own sliders. Same seed + same values = same map. The maps of `data\game_maps.json`
  are presets of them, like Factorio's: TAB's six landscapes (BR, TM, AL, DS, FA, VO), then ours – **Peaceful
  Lowlands** (2026-10-24, after TAB's) is Wild with small groves, smooth lakes and lots of dry land.
- **TAB's generator** (`mapgen/tab.rs`, rebuilt from its code, `docs\tab-nachbau.md` step 6): one generator, the
  landscapes only other numbers – estimated, `ZXRules.dat` stays closed. Noise layers like TAB's (`mapgen/layer.rs`,
  TAB's simplex noise in `noise.rs`) cut at thresholds: land/water, mountains in 2 × 2 blocks, grass, forest on grass
  and on dirt. The command center on the free spot nearest the middle; a new seed if fewer than 18 forest tiles lie
  within 11, if there's too little stone (4 within ±10) or iron (10 within ±25), or – ours – if less than 30 % of the
  playing field can be reached. Villages of doom (a house in the middle, a spiral of small, medium and big ones
  around), ruins near the base with 2–4 piles of loot, stone/iron/**gold** patches on free dirt (gold instead of our
  coal – coal comes back later, no mine takes gold yet), one oil node per block. **The zombies are the map's:** a noise
  over the reachable land, 1–3 per tile by how high it is, the kind by a roll and the distance (`medium_from` …), giants
  at least 70 tiles out, 40 apart. The difficulty is TAB's **population** (`population`: how many, half the villages
  below 0.5, the giants) – not `roaming_zombies`/`keep_out`. None of the shared steps (start area, ways in, safety
  net): what the base can't reach stays empty. Not taken over: old towers, ravens, supplies, mutants, decorations.
- **Wild like Factorio (2026-10-24):** one moisture noise decides – wet = forest ("Forest size": big forests or many
  groves), next to it green meadow, the driest open ground dirt. Lakes from a height noise ("Shore detail": smooth to
  frayed).
- **Start area like Factorio's (2026-10-24, `mapgen/start.rs`):** first look, then help – if a sawmill inside the
  command center's grid would work fewer than 20 forest tiles, groves grow at the edge of the grid until it does (the
  clearing stays free; is it wider than the grid, one tesla tower further). The start patches of ore are planned with
  the grid too: stone one tesla tower out (the barracks needs it), iron ore and coal one or two. A test checks every
  map with 10 seeds.
- **World border:** straight on screen like TAB's – the playing field is a rectangle twice as wide as high (a
  diamond in the grid), the minimap too. Around it a ring of open ground (16 steps, ≈ 11 tiles); the grid corners
  beyond are off the map. Units and the camera stop at the border. Spawners are rectangles on screen as well – in the
  editor also slanted like the tiles. Like SC2's map bounds inside its map size, the playing field is a rectangle of
  its own: a new map has it in the middle, the editor drags it anywhere on the grid (any size, the ground stays);
  minimap and camera follow it. The base is the command center, wherever it stands.
- **Spawners:** zombies only come out of them – 3 per side in the ring, covering nearly the whole side. Mode per
  spawner: constant / periodic / off. Each has a number (`id`) the waves name it by. Nests and wave directions can
  become spawners later.
- **Ways in:** winding open lanes from every side to the base that join like branches – the places to wall off.
  Fords in rivers, passes in region walls. A safety net connects anything cut off.
- **Ore:** iron ore, coal, stone patches; one of each next to the base, bigger and richer further out.
- **Oil (2026-10-18):** nodes of exactly **2 × 2 tiles** (an oil pump goes on one), never touching. One alone next to
  the base, further out groups of 2–6.
- **Dirt:** patches on 40 % of the open ground – walk and build there like on grass, but no farm. Not on Blank. On Wild
  where it's driest.
- **Villages of doom** (points of interest, step 6): far from the base. **How many and how big is the map's** (its
  setup: villages per 10,000 tiles, houses near the base and at the edge – Open Plains many small ones, Broken Lands two
  huge ones); the difficulty makes both more or fewer. A house lets out as many as its size and the population say
  (small 50, medium 100, big 250 at population 1), in batches like TAB's (8.2). A big village spreads wider and keeps further apart. A house keeps a ring of open ground, so it never cuts off
  a way – more houses only take open ground, the terrain stays.
- **Loot:** ≈ 6 piles per 10,000 tiles – 500 gold or 10 wood/stone/iron ore/coal, zombies hidden around each (Normal
  8–50, more further out).
- **Seed:** truly random per new map, `--seed N` on the command line, kept by "Restart".

### 9.2 Map sizes (first guess)

| Size | Tiles | Zombies at peak |
|---|---|---|
| Small | 128 × 128 | ~2,500 |
| Medium | 192 × 192 | ~7,000 |
| Large | 256 × 256 | ~73,000 (like TAB's Caustic Lands): ≈ 65,000 roaming almost everywhere in packs of 1–3 per tile, never more than 3 – near the base only walkers, further out the harder breeds; ≈ 8,000 asleep in villages and around loot |

- **Fog of war:** unexplored (black) / explored (dark, no zombies shown) / visible. Sight per unit and building,
  blocked by forest, computed in the simulation (co-op players share it).

---

## 10. Home screen and meta progression

### 10.1 Home screen (TAB-like)

- A **world map** of **regions** (each a biome with its own threats).
- Region → **round setup:** map size, zombie density, final wave strength, **number of days**, starting resources.
  Every setting changes the **score multiplier** (TAB's %); fewer days = harder.
- Buttons: *New round*, *Continue*, *Regions*, *Research*, *Records*, *Settings*.
- **In the prototype:** "New game" like TAB – a **map** (`data\game_maps.json`, for now one generator with its values
  each), a **difficulty** (`game_difficulties.json`: start, how many zombies, how close each breed comes, TAB's
  population, loot guards, waves, constant spawn) and a **duration** (`game_durations.json`). The score factor is the
  three multiplied – shown, no score yet. "Custom setup" shows every value of the generator. Like TAB's "Custom
  Levels", a second list: every map saved in the editor (`custom` in its file) – its round is in the file, so there's
  nothing to pick and no score factor.

### 10.2 Meta progression (like TAB)

- **Research points** after every round (score-based, a few for a loss too) → persistent **research tree**.
- **Regions unlock** by winning the previous one at ≥ a certain difficulty.
- Records per region: best score, fastest win, highest difficulty.

### 10.3 Other modes

- **Endless** (nice to have): the round goes on after the final wave.
- **Co-op** (wanted): one player hosts, see 12.8.
- **Campaign** (wanted, far off): like TAB's – research inside the round (at the workshops) and **in-between tech**:
  weaker early versions of buildings that research makes into the full ones (e.g. a small windmill first). The
  survival mode (what's being built now) has none of it: everything comes at its **standard strength**, which keeps
  development simple.
- No daily challenge.

---

## 11. UI and controls

### 11.1 Screen layout

**The bottom bar like TAB's (2026-10-18)** – no top bar. From left to right: minimap · clock (day, hour, pause button,
speed, next wave) · control groups · selection (portrait, name, life bar, text, values as small boxes) · command card
(5 × 3, build menus) · stock (colonists, workers, food, energy; gold, wood, stone, iron, oil). Placeholder look in
brass and dark wood until the real art (12.3). The minimap always shows the giants and one warning mark in the middle
of every swarm of a wave (each side of a wave is a swarm), fog or not. Later: noise/creep overlay on the minimap.

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
| Y (grid) | Selected buildings off / on again – no upkeep, workers, power (like TAB) |
| Space | Pause (orders still possible, singleplayer only) |
| Esc | Cancel → deselect → game menu |
| Tab | Next type in the selection (planned: noise/creep overlay elsewhere) |
| WASD / arrows / edge / wheel / middle mouse | Camera. **WASD only with nothing selected** |

- **Every key is changeable** (the table shows the defaults): page *Key mapping* in the Esc menu and on the home
  screen – tabs camera/time, selection/groups, units, command center (every build menu and **every building in it**,
  like TAB's hotkeys), buildings. Up to **2 keys per action**; a key twice on one card (or twice among the global
  keys) turns red.
- **Fixed:** Shift and Ctrl (queue, credit, 5 at once, groups), Esc, Ctrl+S and Ctrl+Z / Ctrl+Y in the editor.
- **The card comes first:** a global key that the current card also has only works without it (X: line formation for
  units, otherwise repair all). A camera key that's on any card only moves the camera with nothing selected.
- macroquad reads keys by their **US position**: on a German keyboard the grid key "Y" arrives as Z (12.2). The names
  shown are a German keyboard's.

### 11.3 Feedback

- Alerts with jump-to-location (*"Building under attack"*, *"Wave incoming from the west"*).
- Edge-of-screen arrows towards fights, clear sound cues per event.

### 11.4 Settings in one JSON file

- **`%APPDATA%\Last Bastion\settings.json`** (open question 16) – a fixed place, so the toolbox finds it too. Saves go
  next to it. A button *Open settings folder* makes it easy to find.
- Planned values: display, window mode, resolution, FPS limit/vsync, volume, UI scale, edge scrolling, language.
- **Never crash on a bad file:** missing values = defaults, unknown ones ignored.
- **In the prototype:** window mode, edge scrolling, scroll speed, confine mouse, health bars, full command chain,
  dev overlay, key bindings (`keys`: only the changed ones, action → first and second key, e.g. `"stop": ["Q", ""]`).

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
- **A big horde (`src/sim/zombies.rs`, `src/grid.rs`):** a tick of the simulation has 50 ms.
  - *Calm zombies are cheap:* a stroller with nothing to notice thinks only every 8th tick (each on another one) and
    walks on with what it decided. Only next to a busy one does it move every tick, so it can be pushed – a calm crowd
    far from them costs nothing more.
  - *Neighbours in memory:* every 2 s the list of zombies is sorted by where they stand, and the grid keeps each one's
    position and size in it, tile after tile – looking at the neighbours finds them in the processor's cache. The crowd
    looks up who it may touch once per tick and only goes through those few in its 8 rounds.
  - *All cores:* what each zombie wants and every round of pushing the crowd apart is worked out side by side (`rayon`)
    – each round only from where everyone stood after the round before ("Jacobi"), collected in order, so the result
    is the same on every PC.
  - *Asleep far away* (8.5): zombies far from your eyes are plain entries per 8 × 8 tiles and cost nothing until your
    eyes, a sound, the scan or the final wave wakes them; calm ones far from everything fall asleep again.
  - Measured (256, `horde_tick_time`): 65,000 zombies, ≈ 1,600 of them awake near the base ≈ 0.6 ms per tick; ≈ 17 ms
    when the final wave has called them all (the crowd like TAB's physics costs ≈ 15 % more than the old pushing).

### 12.2 Language and library

- **Rust** (open question 1) – learning it is a goal of its own. No Python (too slow for the horde). C++, C# and C
  were the alternatives.
- Think in data: objects refer to each other by ID, plain `Vec`s of structs, no ECS library for now.
- **macroquad** (open question 17): fastest start (only `cargo`), batches sprites by itself. Drawing stays behind a
  thin layer of our own, so SDL3/wgpu could replace it later.
- **egui** (through `egui-macroquad`) for panels full of fields – typed numbers and names, drop-downs, folding
  sections: the map editor's panel; later the dev tools in the game (12.6). macroquad draws it, after the HUD.
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
- **Planned – a console like Factorio's:** most dev tools as commands, **hidden from normal players** (a command only
  after typing `/`; panels for the bigger tools on egui, also in the game, opened from the console). In co-op the same
  box is the **chat**, with **pings** (a spot on the map – click it in the chat and the camera goes there).
  Hooks: a cheat changes the simulation, so it's an order like any other (deterministic, everyone sees it in the chat);
  like Factorio, a round with cheats should later count for no score or research (10.2).
- **In the prototype:** speed ×1–×8, F3 overlay (FPS, ms per tick, checksum, switches, key help – sections fold with a click), `--seed N`, an automatic balancing run
  of the mini round, PNG previews of the generators.
- **Map editor** (`LastBastionEditor.exe`, own program since 2026-10-17, like TAB's): opens a map file, paints terrain,
  ore and oil, places units, buildings, zombies and spawners, edits the waves and the round, **Edit** (no time) /
  **Test** (the round runs, afterwards the map is as before). Every change is an order, so it stays deterministic.
  Its panel is on egui (12.2): numbers are typed or dragged. The rework towards TAB + SC2 is planned in 12.9.
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
- **Chat = the console** (12.6), with clickable pings.
- **Everyone can save** (and continue alone), **only the host loads**. A dropped player can rejoin; if the host
  leaves, the round ends.

### 12.9 Map editor: TAB first, then SC2 (plan)

Goal: far more settings per map. TAB's editor (Info, Painter, Entities, Events) is the model for the first steps,
StarCraft 2's (regions, triggers, players and AI, data) for what follows.

- **Waves today:** a new round turns `spawning.json` + duration + difficulty into a concrete list – `waves` in the map
  file (tick, count, breed, from where, final, reveal; one entry per breed and moment) – and a copy of the constant
  spawn (`constant_spawn`). From then on the file counts: the editor's "Waves" tab edits that list, `data\` only
  changes new maps. Changing the days in the editor moves only the final wave.
- **Steps:**
  1. **UI foundation:** fields to type numbers and text into, drop-downs, scrolling lists, a properties panel, dialogs –
     the column of +/- buttons doesn't carry many settings. **egui** – in the editor, and hidden in the game for the dev tools (12.6). **Undo/redo.**
     *In the prototype:* the editor's panel is on egui (every number typed or dragged, drop-downs for breed and side,
     sections that fold, the tab scrolls), plus the constant spawn's specials. Undo/redo (20 steps) keeps **orders, not
     maps**: one copy of the map before the oldest step, and undo plays the other steps onto it again – exact, because
     the simulation is deterministic (dice included); new orders need nothing extra for it.
  2. **Map settings (TAB's Info):** every value of the difficulty and duration per map (start, `keep_out`, zombies per
     house, wave plan – planned anew from rules or by hand), description; the constant spawn's specials.
     *In the prototype:* the **playing field** (the world border) dragged anywhere and to any size, and the **grid**
     around it (grows or shrinks on every side through the map file, new ground is grass) – like SC2's map bounds and
     map size; waves with **groups** (several breeds, chosen spawners by name), stages of the constant spawn with
     spawners of their own (map format 4). Later a trigger action "playing field to region X" makes missions in parts.
  3. **Properties of what's placed (TAB's Entities):** select one or many and edit – zombies per house and their breeds,
     loot, a zombie's breed and state, HP, owner; move, copy, delete.
     *In the prototype:* a right click on a spawner opens its properties (name, straight or slanted, mode, remove); Del
     removes selected buildings (the command center too). The zombie brush works like TAB's: per tile up to 3 of each
     breed (a slider each), painting again only fills up; giants are placed one at a time.
  4. **Painter:** line, rectangle, fill, noise brush, a generator on a part of the map, copy and paste areas.
  5. **Regions and points (SC2):** named areas and spots – what triggers talk about ("north gate", "village 3").
  6. **Triggers (SC2):** events + conditions + actions, variables. Events: a time, a unit in a region, built/destroyed,
     zombies killed, a wave, noise in a region, a house razed. Actions: waves and zombies at a region, messages and
     objectives, reveal, resources, wait, victory/defeat, orders to allies. Edited as sentences ("When a unit enters
     North gate: 200 runners from spawner 2"). Checked in the simulation every tick in a fixed order – deterministic
     for co-op.
  7. **Players and ally AIs (SC2):** the colony (1–2 humans), ally AIs with colour, start units and buildings, shared
     vision or not; zombies against everyone. Behaviours: hold a region, patrol, guard, attack-move, retreat (building
     later). Steered by triggers ("Ally 1: defend West") and reacting to events ("ally base attacked").
     **Owner of everything placed:** today a unit in the map file is only type and place, a building type, tile and
     turn – everything is the colony's. Then each gets an `owner`: colony (old files), an ally, or **neutral** – can't
     be selected, ordered, repaired or built over, costs and pays nothing; whether zombies go for it is set per thing.
     A trigger can hand it over ("rescue the outpost": neutral → colony).
  8. **Data per map (SC2's data editor, light):** a map overrides values of `data\` for itself.
- **Hooks:** allies need an **owner on every unit and building** – it reaches combat, vision, selection, upkeep. Co-op
  (12.8) needs the same, so build it once, early. Triggers must stay deterministic (integers, fixed order). Players and
  triggers make map format 5.

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
| **6. Production chains** | Drill → stock → smelter → workshop, with power | A small chain runs by itself | Mine → foundry → iron → machine gun tower; tools missing |
| **7. Houses, gold, upkeep** | Houses consume and pay, units cost upkeep | Economy and army linked | |
| **8. Map generation** | Terrain, patches, nests, POIs, fog | Two seeds feel different | Partly (TAB's generator and ours, ore, oil, fog, villages of doom, loot, dirt) |
| **9. Noise and creep** | Noise attracts waves, nests spread creep | Expanding is a decision | Noise like TAB (adds up, lures, villages) – creep missing |
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
| 27 | Two tesla towers (power pole vs. weapon)? | Open – one needs another name |
| 29 | What do the wonders do? (The inn: mercenaries, 2026-10-21) | Open |

**Decided** (number = question, referenced elsewhere):

- **1** Rust · **2** isometric, logic on a square grid · **3** co-op, one player hosts, lockstep · **4** supply by
  market radius, no belts · **5** TAB-style building, no workers · **6** ammo free for now (see 26) · **7** active
  pause, singleplayer only · **8** hero parked · **9** saving like Factorio · **10** research tree like TAB ·
  **11** name *Last Bastion* · **12** debt in stages, then desertion · **13** the TAB side is the heart ·
  **15** own repo · **16** settings in `%APPDATA%\Last Bastion` · **19** one shared colony · **20** direct IP /
  Tailscale, Steam if released · **23** first power: command center + charcoal burners (wood) · **24** colonists are a
  resource · **25** a unit is free again when idle and in no control group · **26** no ammo item: the munitions factory only unlocks firearms, after the stone workshop
  (2026-10-20) · **28** one mine for every ore patch
  (iron, coal, stone), no quarry (2026-10-20) · **30** the final wave calls every zombie on the map, villages and loot
  guards too · **31** the waves bring the special breeds, more and more over the round (both 2026-10-20) · **32** the
  foundry is both: a tech building like TAB's (it unlocks the iron tier) that smelts iron ore + coal into iron while it
  runs, like the charcoal burner burns wood – no extra building, the Factorio chain stays · **33** new towers
  (ballista, MG tower) shoot by themselves; the ballista is early game like TAB's – unlocked by the wood workshop, no
  iron (both 2026-10-24).

---

## 15. Idea parking lot

Ideas that don't serve a pillar yet, or are too big for now.

- **Day/night cycle:** more zombies at night, lights cost energy.
- **Weather:** fog cuts sight, storms knock out solar.
- **Trains** between distant ore fields, **drones** delivering items.
- **Refugees** mid-round – maybe infected. 😈
- **Mutations:** a random horde modifier per round.
- **Mod support** via the JSON files. **Replays** (seed + orders, almost free with lockstep).
- **Air units** (until the era is clear – a hot-air balloon would fit low tech).
- **Rally point into a control group.**
- **Several maps in one round** – regions or Factorio's planets, travelled by train or plane. On **one grid** (regions
  the playing field moves between, like SC2's missions in parts) it comes almost free with the playing field and
  triggers (12.9 steps 5–6). **Separate grids** ("planets", each simulated all the time): `World` would hold several
  surfaces – every system and every co-op checksum per surface, and the cost of the horde times their number. And it
  pulls against the core: TAB's tension is one base against one horde. Maybe for a campaign or an endless mode.
- Taken out: belts, the crawler, scout/engineer/grenadier units, the hero (question 8).
