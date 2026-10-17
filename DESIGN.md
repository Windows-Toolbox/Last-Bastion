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
| 1 | **Iron** | Smelter (ore + coal) | Buildings, tools, ammo |
| 1 | **Energy** | Command center (a little), wood generator (early, inefficient), coal plant / later wind, solar | Machines, houses, towers |
| 2 | **Tools** | Workshop (iron + wood) | Tier 2 houses, units |
| 2 | ~~Ammo~~ | ~~Armory (iron + coal)~~ | **Free for now** (open question 6, 2026-10-15) – ammo as an item is parked |
| 3 | **Medicine** | Lab – recipe later (tier 3 is far away) | Tier 3 houses, healing |
| — | **Gold** | Houses (taxes) | Buying units and their **upkeep** (7.2), research, buildings |

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

- Houses give colonists (5.3). The top bar shows them like gold: *free / total*.
- **Buildings need colonists to run** (like TAB's workers). If there aren't enough, buildings are **switched off**
  until there are (2026-10-15):
  - **Gathering buildings first** (lumber camp, quarry, drill …), **spread over the resources**: one of each kind in
    turn, so no single resource drops to zero while the others keep running.
  - **Farms and power generators come last** (2026-10-15). Switching off food or power would leave houses unsupplied
    → less gold, maybe fewer colonists → even more buildings switched off: a downward spiral like in TAB.
  - In between: production and defence ❓ (order still open).
- **Units need colonists too:** training a unit takes colonists from the colony – **1 per infantry unit, more for
  vehicles** (2026-10-15, exact numbers in the data files). They're gone for as long as the unit lives.
- → More houses = more colonists = more buildings and a bigger army. Houses are the bottleneck, as in TAB.

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

First draft. Costs/stats come later in a data file (like the cube game's JSON files).

| Category | Building | Notes |
|---|---|---|
| **Core** | Command center | Start building. Small storage, losing it = losing the game. No workers (open question 5) |
| **Housing** | Tent → Stone house → Apartment | can be upgraded when materials ready and the next tier item is supplied |
| **Gathering** | Lumber camp, quarry, drill, farm, fishery | Placed on/near the right tile type, work on their own |
| **Production** | Smelter, workshop, lab | Take inputs from the colony stock (5.4). Armory parked (ammo is free for now) |
| **Logistics** | Warehouse, market, road | Warehouse = more storage; market = supply radius for houses; road = **units and zombies** move faster on it |
| **Power** | Power pole, wood generator, coal plant, wind turbine, solar field, battery | Wood generator = early, cheap, inefficient; wind/solar = quieter but weaker |
| **Military** | Barracks, factory (vehicles), tech lab | Production queue + rally point. No air units for now (parking lot) |
| **Defence** | Wood wall, stone wall, gate, watchtower (can put in 4 ranged units), ballista tower, flame tower, tesla tower, mine field | Towers use energy (ammo is free for now) |

**In the prototype (2026-10-16):** the command center (3 × 3, 1,500 HP) and tents (2 × 2, 300 HP). Click the
command center to select it: it trains soldiers and snipers for testing (what the barracks will do later: queue of 5,
rally point on a point or a unit) and places tents on the grid (green/red preview, Shift for several). Buildings block
the way for units and zombies; zombies hit any building they touch and walk towards the command center. Not yet:
costs, build time, infection, losing the round when the command center falls.

**Walls, gate, tower (prototype, 2026-10-16):** walls (1 × 1, 400 HP) dragged diagonally stand corner to corner, one
tile wide like in TAB – still closed, because walking (and every "is the line free" check) never crosses a corner. A
placeholder skin joins neighbouring walls, gate posts and towers into one line (straight or diagonal pieces from a post
in the middle). **Gate** (3 × 1, turnable, 400 HP): the middle tile is a door – open for your units, closed for
zombies, who have to tear the gate down. **Watchtower** (1 × 1, 600 HP = wall + 50 %, later also more expensive): up
to 4 soldiers/snipers go in (right click), they shoot and see **+2 tiles**, can't be hit (the zombies hit the tower)
and come out with "unload" (on the side of the tower's rally point, then they walk to it) or when the tower falls. The building stays on the mouse after placing until right click /
Esc.

**Placement rules (prototype, 2026-10-16):** all in one file (`src/placement.rs`). A **gate can be placed over
walls** (like in TAB) – the walls under it are torn down and each refunds 50 % of its price, so the gate is cheaper
there, but walls + gate still cost more than the gate alone (never a trick to save). **Towers keep 1 free tile** to
other towers (also diagonally). First prices, only shown for now: tent 50, wall 10, gate 40, tower 30.

**Building like TAB (open question 5, 2026-10-15):** you place a building, it's paid at once and builds itself over
time. No worker units, nothing to gather by hand – gathering buildings work on their own.

**Roads (2026-10-15):** your units **and the zombies** are faster on roads. A road into the base is also a fast lane
for the horde.
| **Special** | Radar (reveals part of the map), sound dampener (less noise), cleanser (removes pollution), beacon (lure zombies to a kill zone) | |

**Infection (from TAB):** If zombies damage a building enough they infect it. This Spawns more zombies (amount and type depending on the building). Breaches should feel catastrophical. Same concept as TAB in the sense of hp and shield for implementation. (shield down = building infected, can be damaged more until destroyed)

---

## 7. Units and control

### 7.1 Control (the StarCraft part)

The most important feel of the army part (pillar 2). The heart of the game is the TAB side, though (1, open question
13). The prototype steps keep their order anyway (13): without units you can't fight the horde.

- Box select, click select, double-click = all of that type on screen, Shift = add/remove.
- **Control groups** 1–0 (Ctrl+number = set, number = select, number twice = jump camera).
- **Attack-move** (A + click), move, stop, hold position, patrol. Right click or move onto an own unit = **follow**
  it (2026-10-16).
- **Smart-casting**: ability key casts it with the best-suited unit of the selection, not all of them.
- **Smart targeting**: units choosing their own targets spread their fire over different enemies instead of all
  shooting the same one, and don't waste shots on a zombie that's already as good as dead (StarCraft 2's overkill
  prevention).
- **Rally points** for production buildings, also onto a unit: new units then follow it (2026-10-15). Rally into a
  control group is parked.
- **Formations like *Diplomacy is Not an Option*** (2026-10-16, replaces SC2's magic box): choose a shape on the
  command card – **line**, **box** or **circle** – and **drag with the right mouse button**: the units line up along
  the dragged line (circle: the drag is the radius). With T/P/M waiting, the left mouse button drags the same way
  (attack-move/patrol in formation). Only a **drag** forms the shape: a plain click (any command, minimap too) sends
  everyone to the clicked point (changed 2026-10-16; before, a click also formed the shape). Soldiers
  go in front (outside in the circle), snipers and medics behind; within a row units keep their left-to-right order,
  so nobody crosses paths. The formation travels with the order, so it is the same on both PCs in co-op.
  *Superseded:* the magic box (2026-10-15/16) with its 6-tile size limit and its two settings.
- **Active pause** (Space, singleplayer only): give orders while paused. Makes micro accessible without high APM. No
  pause in co-op (open question 7). *Prototype (2026-10-16):* orders given in the pause are taken over at once and
  shown (command lines, production queues, rally points, new buildings), but nothing moves until the game goes on –
  exactly as if they had been given with the next tick, so it stays deterministic.
- **Game menu** (Esc, like StarCraft 2): small overlay with save, load, settings, leave (11.5). In singleplayer it
  **pauses everything** (2026-10-15): unlike the Space pause, you can't select anything or give orders – only the menu
  works. In co-op it doesn't pause (12.8). *Prototype (2026-10-16): continue, settings, restart, new map, quit; save
  and load are greyed out until step 10.*

### 7.2 Unit roster (first draft)

Small army, strong units. The infantry is taken from **TAB**, while the support units and vehicles come from
**StarCraft**. Each unit has **a role** and **exactly one active ability** (TAB units have none, so these are new). One,
not more: it keeps the APM low and gives every unit a clear identity (2026-10-15).

| Unit | From | Tier | Upkeep (gold/min) | Role | Ability | Noise | Counters | Weak against |
|---|---|---|---|---|---|---|---|---|
| **Ranger** | TAB | 1 | 1 | Fast, cheap, bow, weak damage | *Sprint*: short speed burst ❓ | Silent | Single runners, scouting | Masses, brutes |
| **Soldier** | TAB | 1 | 2 | Rifle all-rounder, the army's backbone | *Stim* (SC marine): faster attack and movement, costs HP | Loud | Walkers, runners | Swarms when clumped, brutes |
| **Sniper** | TAB | 2 | 3 | Very long range, slow fire, huge damage per shot | *Headshot*: one big shot, long cooldown | Very loud | Brutes, spitters, screamers | Anything that reaches it |
| **Flamethrower** | TAB (Lucifer) | 2 | 3 | Short-range cone, sets the ground on fire | *Burn ground*: area denial + removes creep | Loud | Swarms, dense packs | Spitters (range), brutes |
| **Medic** | SC | 2 | 2 | No attack; heals nearby infantry automatically (energy) | *Restoration*: removes acid, slow and burning | Silent | – (support) | Being targeted |
| **Machine gunner** | – (mech) | 3 | 5 | Walker/mech with a gun that has to **spin up** first | *Keep spinning*: toggle, stays spun up without a target | Very loud | Large hordes, holding a line | Brutes, getting surprised |
| **Siege tank** | SC | 3 | 6 | Primitive artillery, can **only shoot when sieged** | *Siege / unsiege* | Very loud | Hordes at long range, nests | Anything up close, flanking |

**In the prototype (2026-10-16)** only soldier and sniper exist, without abilities, with first-guess numbers
(`src\kinds.rs`): soldier HP 100, armour 1, damage 10 every 0.5 s, range 6, speed 3; sniper HP 60, armour 0,
damage 35 every 2 s, range 10, speed 2.6. Walkers have 20 HP and hit for 6 per second (minus armour) once a unit is
closer than 5 tiles.

#### Upkeep instead of supply (like TAB)

- Every unit costs gold once when it's built **and then every minute** (upkeep, first guesses in the table). There is
  **no supply cap**.
- The army's size is limited only by your income. More soldiers means less gold for houses, research and defences, and
  the other way round. That's the choice between economy and army.
- **Dismiss** a unit to stop its upkeep (no refund), e.g. after the final wave is over in endless mode or when the
  economy breaks down.
- When gold runs out, see **Debt** below.

#### Debt: what happens when gold runs out

Gold can go **below zero**. The deeper the debt, the worse it gets, so the player sees the problem growing instead of
being punished all at once:

| Stage | When (first guess) | Effect |
|---|---|---|
| **0. Broke** | Gold reaches 0 | No new units, buildings or research. Upkeep keeps running → debt. |
| **1. Grumbling** | Debt > 0 | Unpaid units fight worse: −15 % damage and attack speed. |
| **2. Unrest** | Debt > 1 minute of upkeep | −30 %, and units move slower. |
| **3. Desertion** | Debt > 2 minutes of upkeep | Every ≈ 10 s one unit **deserts** (leaves the map), picked at random but weighted (see below). |

- **Who deserts:** every unit has a chance, weighted by
  `weight = upkeep × (3 if not a veteran, else 1)` (first guess). So expensive units and non-veterans are the most
  likely to leave. A sniper (upkeep 3, non-veteran → 9) is three times as likely to desert as a veteran sniper (3) and
  nine times as likely as a veteran ranger (1).
- Paying the debt off (back to ≥ 0) removes the penalties right away. Deserted units don't come back.
- Alerts warn before each stage (*"The soldiers haven't been paid – morale is dropping"*), and the HUD shows the gold
  in red with the time until the next stage.
- → Veterans are much less likely to leave, so the units you kept alive with good micro usually stay with you.
- Size to plan for: ≈ 50–120 units (a mixed army probably ends up around 60–80). Control groups and selection have
  to handle that without trouble (StarCraft 2 handles 200 supply).

**Noise per unit** links the army to the noise system (8.3): rangers can go out quietly, while snipers, machine gunners
and tanks give away where they are – like in TAB.

#### Siege tank: weaker and cruder than StarCraft's

- **Unsieged:** drives slowly and **can't shoot at all**. It needs an escort.
- **Sieging up** takes ≈ 4 s (StarCraft: ≈ 3 s), unsieging ≈ 3 s, and it can't defend itself during either.
- **Sieged:** long range, splash damage, slow rate of fire. It has a **minimum range**: zombies that reach it are safe from
  it. Splash also hits your own units and buildings, like all area damage (see below).
- Less damage and a smaller splash radius than in StarCraft, because ammo is crude (makeshift shells) ❓.
- → The classic StarCraft pattern: tanks sieged behind the wall, infantry in front of them.

#### Machine gunner: spin-up

- When it starts firing it shoots at **≈ 25 %** of its full rate and reaches **100 % after ≈ 3 s** of continuous
  fire.
- With no target for ≈ 2 s it spins down again.
- *Keep spinning* keeps it at full speed without a target, at the cost of energy and **constant noise**. That's a
  trade-off: be ready for the wave, or draw it in.
- → Strong when holding a line and weak against sudden attacks. That's where you pair it with soldiers.

#### Area damage hits everyone (2026-10-15)

- **All area damage of the player** (siege tank splash, flamethrower, burning ground, mines, flame tower …) also hurts
  **your own units and buildings**. Where you aim it matters.
- **Area damage of zombies** (swarm explosion, spitter acid, a giant's blow) also hurts **other zombies**. A few
  exceptions only where it fits ❓ (e.g. the acid could leave zombies alone).

#### Look: StarCraft-like skins

The direction is liked (2026-10-15), but depends on the era (open question 21) and on where the pictures come from
(12.3, open question 22). TAB's roles with a StarCraft-*like* look of our own design (inspired by StarCraft, nothing
copied):

| Unit | Look |
|---|---|
| Ranger | Light armour, hood, bow – like a Reaper without a jetpack |
| Soldier | Bulky power armour like a marine, but more makeshift |
| Sniper | Slim suit with a helmet visor, like a Ghost |
| Flamethrower | Heavy armour with tanks on the back, like a Firebat |
| Medic | White armour with a red cross, like StarCraft's medic |
| Machine gunner | Small two-legged walker with a minigun |
| Siege tank | Patched-together tank with a crane-like gun |

As in the cube game, unit pictures could be **swappable PNG files** in an `images` folder, so changing a skin means
replacing a file.

### 7.3 Veterancy

Units gain XP from **killed enemies**. With enough XP a unit becomes a **veteran**, and that's the only level: more HP
and damage, plus a visible mark (e.g. a chevron).

- Every killed zombie gives XP (more for tougher types, e.g. walker 1, brute 10, giant 100 ❓). The XP is **split among
  all your units nearby** (radius ≈ sight range ❓), not just the one that made the kill.
- Veterans count as **half a share**. They still take part of the XP even though it does nothing for them. Otherwise,
  in a group of veterans, the whole horde's XP would go to the last few non-veterans.
- Split: each unit gets `XP × own share / sum of all shares` (share 1 = normal, 0.5 = veteran).
  Example: 10 XP, 3 normal units + 2 veterans → total shares 4 → normal units get 2.5 each, veterans 1.25 each. The
  veterans' part is simply used up.
- → Fresh units fighting next to veterans level up at a sensible pace, without instantly becoming veterans.

Makes losing a squad hurt and saving it with micro rewarding. Veterans are also less likely to desert when you're in
debt (7.2). How much XP is needed ❓, probably per unit type, in the data files.

---

## 8. The horde

### 8.1 Zombie types

| Zombie | Behaviour | Why it exists |
|---|---|---|
| **Walker** | Slow, weak, *lots* of them | The mass. Tests your walls. |
| **Runner** | Fast, weak | Punishes gaps and lone units. |
| **Brute** | Slow, huge HP, smashes walls | Needs focus fire (snipers). |
| **Spitter** | Ranged acid, outranges short-range units | Forces you to push out instead of turtling. |
| **Screamer** | Calls nearby zombies when it sees you | Turns a small fight into a big one – kill it first! |
| **Swarm** | Tiny, fast, many, explodes on contact | Counters clumped armies (spread out!). |
| **Giant** (like TAB's) | Rare, huge, very slow, enormous HP, smashes through walls and buildings, hits units in an area | Appears in late waves and the final wave. A wall alone won't stop it – it needs the whole army. No other bosses. |

**Targeting (prototype, 2026-10-16):**

- A zombie goes for the nearest **unit or building** within 5 tiles (straight line), whichever is closer. A unit
  standing behind a wall means the wall is closer – so the wall gets torn down.
- With nothing nearby it heads for the command centre. Its path runs **through** buildings when the way around is more
  than ~10 tiles longer (each building tile costs 10 extra steps in the flow field). A fully walled base is never
  "unreachable" – the horde breaks in.
- *Update 2026-10-16:* zombies **see** units up to 13 tiles (forests and trees block it, like for units), **hear** them
  within 3 tiles, and a zombie that is **shot** storms for 5 s even without seeing the shooter (a first, local piece of
  noise, 8.3). Since sight went from 10 to 13 (same day) zombies see further than any unit shoots, so the alarm mostly
  matters when a shooter leaves their sight. Having noticed a unit, it storms at it along a second hunt map: around water and mountains, but a
  building tile only costs 2 steps – so it goes almost straight through walls and tears them down instead of looking
  for the gap. Without a unit noticed, the 5-tile rule for buildings and the path to the command centre apply as above.
  Sight is computed from the few units outwards ("from this tile a unit is visible"), not from thousands of zombies.
- About **1 in 4 zombies is a hunter**: it smells units up to 12 steps *on foot* (around walls) and walks through gaps
  to them instead of hitting the wall. Only some – otherwise the whole horde would ignore the walls and squeeze into
  every 1-tile gap. Later this role could move to the **Runner** ("punishes gaps").

### 8.2 Waves

- **Ambient:** single zombies roam all the time.
- **Noise waves:** your noise attracts groups (see 8.3) – you cause these yourself.
- **Scripted waves:** on fixed days, every few days and growing (e.g. every ≈ 4–5 days ❓ ≈ 12–15 min), from a random
  but *announced* direction (warning ≈ a quarter of a day ≈ 45 s ahead ❓). Planned as a share of the round (4.1), so
  an 80-day round has more of them than a 40-day one.
- **Final wave:** on the last day. The countdown is visible all round, the direction is announced ≈ 2 days (≈ 6 min)
  ahead.
  Everything, including giants.

### 8.3 Noise (Factorio's pollution × TAB's noise)

- Every machine, tower shot and explosion makes **noise**. Noise spreads over the map like Factorio's pollution cloud
  (per map chunk, spreads to neighbours, fades over time).
- Zombies and nests "drink" noise: when enough noise reaches them, they start moving towards its source.
- → Expanding production makes you stronger **and** more noticeable. Quiet tech (wind, solar, dampeners) becomes a real
  strategic choice.
- Noise overlay toggle (like the pollution map in Factorio) so the player can see it.

### 8.4 Creep (like StarCraft's, but for the zombies)

- **Nests** spread creep on the ground around them, slowly growing over the round.
- On creep: you can't build, your units are slowed, zombies regenerate.
- Burning it (flamethrowers, cleanser building) removes it. Destroying the nest stops the spread.
- Gives the army an offensive job instead of only defending.

---

## 9. Map generation

A mix of Factorio (noise-based terrain with resource patches) and TAB (hand-feeling regions with points of interest).

### 9.1 Steps

1. **Height + moisture noise** (Perlin/Simplex) → biomes: grassland, forest, rock, swamp, water, ruins.
2. **Start area:** guaranteed safe-ish zone with wood, stone, a small iron + coal patch and farmland nearby.
3. **Resource patches:** Factorio-style blobs, richer and larger the further from the start. Sometimes a rich patch
   right next to a nest (risk/reward).
4. **Chokepoints:** rivers, cliffs and forests create natural lanes – the map should have some places that are
   "obviously" good to wall off.
5. **Nests:** placed with minimum distance to the start, more of them towards the map edges.
6. **Points of interest** (TAB's villages): abandoned towns, crashed supply drops, old bunkers. Guarded by zombies,
   reward = gold, items, a free unit, a blueprint.
7. **Final wave direction** chosen at map creation (and maybe hinted at by where the big nests are).

**World border (prototype, 2026-10-16):** the playing field is surrounded by a ring of 16 tiles outside the border.
Zombies spawn there and walk in; your units can't cross the border, the camera stops at it, and nothing beyond it is
shown in the game view. Building is not allowed with a zombie on the site or within 4 tiles around it.

**Spawners (prototype, 2026-10-16):** zombies only come out of **spawners** – rectangular areas placed, renamed,
moved and resized in the editor (players don't see them). New maps get **3 spawners per side** in the ring outside the
border. Waves, the start horde and respawned zombies all come from them; without spawners no zombie comes. Each
spawner has a mode (editor): **constantly** (killed zombies come back in one of these, if respawn is on),
**periodically** (every 1–120 s, 1–500 zombies) or **off** (only waves). Later the nests (8.4) and the wave directions
(8.2) can simply become spawners.

**Noise terrain (prototype, 2026-10-16):** the map comes from value noise (`src\noise.rs`, several octaves): a height
map makes **lakes** (lowest share) and **mountain ranges** (highest share), a moisture map **forests**, 0–2 thin bands
of further noise make winding **rivers** (step 4: chokepoints), single trees are scattered, denser near forests. The
shares (water 3–12 %, mountains 3–10 %, forest 8–22 %), the feature size and the number of rivers are rolled per map,
so maps differ in character, not just layout. Nothing within 14 tiles of the base. If water or mountains cut a spawner
off from the base, a 3-tile **ford** is cleared in a straight line. Forests and trees block walking and **sight**;
mountains and water only walking.

**Seed (prototype, 2026-10-16):** every start and "New map" draws a truly random seed (the OS random generator via
Rust's `RandomState`, mixed with the clock); `--seed N` on the command line starts a given map (12.6); "Restart" keeps
the seed. In co-op the host draws it and sends it along (12.8).

### 9.2 Map sizes (first guess)

| Size | Tiles | Zombies at peak |
|---|---|---|
| Small | 128 × 128 | ~1,000 |
| Medium | 192 × 192 | ~3,000 |
| Large | 256 × 256 | ~6,000 |

- **Seeds:** every map has a seed; same seed = same map (good for sharing).
- **Fog of war:** unexplored (black) / explored but not visible (grey) / visible. *Prototype (2026-10-16):* units and
  buildings have a sight range (soldier 10, sniper 13, medic 10, command center 11, tent 5, wall and gate 3, tower 6
  tiles; units in a tower +2), blocked by forests and trees. Unexplored is black, explored-but-not-visible is darkened
  (terrain stays, zombies don't show), on the minimap too. Computed in the simulation, so co-op players share it.
  Editor mode shows everything – or, chosen with "View", the player's or the zombies' sight (zombies: 13 tiles, 8.1). Zombies leaving sight fade out over 0.8 s.

---

## 10. Home screen and meta progression

### 10.1 Home screen (TAB-like)

- A **world map** split into **regions** (each a biome with its own threats).
- Pick a region → **round setup screen**:
  - Map size, zombie density, final wave strength, **number of days** (e.g. 40 / 50 / 60 / 80 ≈ 2–4 h – fewer
    but longer days than TAB, 4.1), starting resources.
  - Each setting changes the **score multiplier** (TAB's %). Higher difficulty = higher score; fewer days = harder.
- Buttons: *New round*, *Continue*, *Regions*, *Research*, *Records*, *Settings*.

### 10.2 Meta progression (like TAB, but without a campaign)

In TAB, research and unlocks belong to the campaign. Here they come from the **normal rounds**:

- **Research points** after every round. How many depends on the score (difficulty % × result), and a lost round
  gives a few too.
- They're spent in a persistent **research tree**, as in TAB's campaign: unlocking new units, buildings and upgrades
  for later rounds.
- **Regions unlock** like TAB's survival maps: win the previous region at ≥ a certain difficulty % (each region =
  new biome, new zombie type or mechanic).
- Records per region: best score, fastest win, highest difficulty.

### 10.3 Other modes

- **Endless** (would be nice): after the final wave the round keeps going, waves keep growing, and the score counts
  how long you last.
- **Co-op** (wanted, 2026-10-15): one player hosts the round in the game, the others join – no servers of our own.
  How it works technically: 12.8.
- No campaign and no daily challenge planned.

---

## 11. UI and controls

### 11.1 Screen layout (StarCraft-style)

```
┌───────────────────────────────────────────────────────────────────┐
│ Gold 1250 (+40/min)     Iron 210  Energy +12  │ Day 18 of 50     │  ← top bar
│                                                                   │
│                                                                   │
│                          game view                                │
│                                                                   │
│                                                                   │
├──────────┬───────────────────────────────────────┬────────────────┤
│ minimap  │  selection (portraits / unit info)    │ command card   │
│ + noise  │                                       │ (abilities,    │
│ overlay  │                                       │  build menu)   │
└──────────┴───────────────────────────────────────┴────────────────┘
```

### 11.2 Keys (first draft)

| Key | Action |
|---|---|
| Left click / drag | Select / box select |
| Right click | Smart command (move, attack, enter) |
| T, S, H, P, M | Attack (attack-move), stop, hold, patrol, move (T/P/M wait for a left click; right click or Esc cancels). Attack moved from A to T on 2026-10-16 so that A is free for the camera |
| Shift + command | Queue the command at the end of the unit's chain instead of replacing it (like StarCraft) – works for every command, also attack-move, patrol and hold. A/P/M stay active while Shift is held |
| Q W E R | Abilities (command card grid) |
| B | Build menu |
| Ctrl + 1–0 / Shift + 1–0 / 1–0 | Set / add to / select control group; pressing the number twice jumps the camera there. Clicking a group in the bottom bar works the same, with the same modifier keys |
| Ctrl + Shift + 1–0 | Set the group and take its units out of all other groups ("steal"). StarCraft 2 uses Alt for this, but Alt + key probably beeps on Windows with macroquad (12.2) |
| Ctrl + click, double click | Select all units of that type on screen |
| Backspace | Camera to the base |
| Space | Pause (orders still possible; singleplayer only) |
| Esc | Game menu: save, load, settings, leave (11.5); pauses everything in singleplayer |
| F2 | Select the whole army (like StarCraft 2; in co-op only your own and free units) |
| F5–F8 | Camera bookmarks (like StarCraft 2) |
| Tab | Toggle noise / creep overlay |
| Middle mouse drag | Grab the map and move it |
| **WASD** / arrow keys / screen edge / mouse wheel | Move camera / zoom. **WASD with priority rule** (decided 2026-10-16): a key the current selection uses itself (on its command card) does its command instead of moving the camera – e.g. S = stop with units selected, W = sniper with the command center selected. With Ctrl held, WASD never moves the camera |

**Every key can be changed** in the settings (11.4). The table above is only the default.

**In the prototype (2026-10-16)** everything in this table works except abilities, the build menu, Tab overlay and
the Esc menu: selection (also Ctrl/double click, clicks on the selection icons, Tab subgroups), all move and attack
commands with Shift queues, follow, formations (line/box/circle, drawn with the mouse), control groups (also steal), camera bookmarks, minimap
clicks. Queued commands are drawn as lines for the selected units (green move/follow, red attack, yellow patrol). The
bottom bar (11.1) has minimap, control groups, selection with unit stats and the command card. The developer tools
(12.6) moved to F4 and the Insert/Delete/Home/End/Page block.

### 11.3 Feedback

- Alerts with jump-to-location: *"Building under attack"*, *"House downgraded"*, *"Wave incoming from the west"*.
- Edge-of-screen arrows towards fights.
- Clear sound cues per event (important when hundreds of things happen at once).

### 11.4 Settings in one JSON file

All player settings live in **one JSON file**, so other programs can show and change them too – e.g. a settings
section on the game's page in the windows-toolbox (12.7).

| Setting | Example value |
|---|---|
| Display | `"display": 1` (number of the monitor) |
| Window mode | `"window_mode": "borderless"` (window / borderless / fullscreen) |
| Resolution | `"resolution": [2560, 1440]` |
| FPS limit, vsync | `"fps_limit": 144`, `"vsync": true` |
| Volume | `"volume": {"master": 0.8, "effects": 1.0, "music": 0.5}` |
| UI scale, edge scrolling | `"ui_scale": 1.0`, `"edge_scroll": true` |
| Language | `"language": "de"` – when it's missing: the Windows display language ❓ |
| Key bindings | `"keys": {"attack_move": "A", "stop": "S", "pause": "Space", …}` |

- **Where (open question 16, 2026-10-15):** `%APPDATA%\Last Bastion\settings.json` – a fixed place, so the toolbox
  finds it without asking the game. Saves go next to it (11.5).
  - Pro: survives a new build, a deleted `dist` folder or a fresh clone; the usual place on Windows; every Windows
    user has their own; works even if the game ever sits in `Program Files` (no write access there).
  - Con: hidden folder, harder to find by hand (→ a button *Open settings folder* in the game and on the toolbox page);
    not portable on a USB stick; stays behind when the game is deleted.
- **Key bindings as action → key name** (`"attack_move": "A"`), not as key codes. Readable, editable by hand and by
  the toolbox page.
- **Never crash on a bad file:** missing or broken values fall back to the defaults, unknown keys are ignored. The
  defaults live in the code (or in a `settings.default.json` in the repo), the player's file only holds changes ❓.
- **In the prototype (2026-10-16):** `src\settings.rs` with serde. Values so far: `window_mode` (`"window"` /
  `"borderless"`), `edge_scroll`, `scroll_speed`, `confine_mouse`, `magic_box_move`, `magic_box_attack`,
  `always_health_bars`, `dev_overlay`. Missing values come from the defaults in the code, unknown ones are ignored, a
  broken file gives the defaults. The whole file is written (not only the changes). Key bindings are still missing.
- **Who writes when:** the game writes the file only when the player changes something in its settings menu. If the
  toolbox changes it while the game is running, the game applies it on the next start ❓ (or reloads it when the
  window gets focus).

### 11.5 Save and load (like Factorio, 2026-10-15)

Everything you'd expect from Factorio, plus a bit more (open question 9):

- **Save freely** at any time into as many slots as you like, with a name.
- **Autosave** every few minutes into a few rotating slots ❓ (how often, how many – in the settings).
- **Quick save / quick load** on a key.
- **Load screen** with a list of saves, each showing: name, **date and time** of saving, **time played in the round**,
  **day X of Y**, region and difficulty, and a **screenshot of the last frame** of the game window before saving
  (only the game, not the desktop).
- Saves live in `%APPDATA%\Last Bastion\saves\` (11.4).
- Reachable from the home screen (*Continue*, *Load*) and in the round from the game menu (Esc, 7.1).
- An *Ironman* option (one save only, like TAB) can come later ❓.
- In co-op every player can save, but only the host can load (12.8). A co-op save can also be loaded in singleplayer.

---

## 12. Technical notes

### 12.1 The hard parts

| Problem | Why it's hard | Usual solution |
|---|---|---|
| **Thousands of zombies pathing to the base** | A* per zombie is far too slow | **Flow fields**: one field per target, computed once, every zombie just follows the arrows. |
| **Collisions between thousands of units** | Each vs. each = n² checks | **Spatial hash / grid**: only check neighbours in the same cell. |
| **Drawing thousands of sprites** | Many draw calls per frame | Sprite batching, cache rotated sprites (the cube game's sprite cache is the same idea), only draw what's on screen. |
| **Group movement that feels good** | Units bump into each other, clump, block corridors | Steering behaviours (separation, cohesion), formation offsets. |
| **Path-finding for your own units** | 80 units through one gate, around buildings that appear and disappear – a different problem from the horde | A* on the tile grid per group (or a small flow field per move order) + steering on top. Already needed in prototype step 1 as soon as there are obstacles. |
| **Determinism** (for seeds, replays, co-op) | Floating point, random order, Rust's `HashMap` order | Fixed tick rate, own random generator with seed, rules for the simulation code (12.8). Required for co-op. |

**Path-finding in the prototype (2026-10-16):** one **line-of-sight flow field** per target, for units (one per move
order, shared by the group) and for the horde (to the command centre). Distances by Dijkstra with 8 neighbours
(straight 10, diagonal 14, no corner cutting); instead of an 8-direction arrow every tile keeps an **anchor** – the
furthest point on its way it can still see (the idea of **Theta\***: take over the next tile's anchor if it's visible,
otherwise the next tile becomes the anchor). Walkers go straight at the anchor: any angle, taut around corners. Before,
with 4-neighbour distances and arrows, units went diagonally until one axis fitted and then straight. 2–4 ms per field.

Options for later, if needed:

| Algorithm | What it is | When |
|---|---|---|
| **Theta\* / Lazy Theta\*** | A* that may skip nodes with line of sight – any-angle paths for one walker | single units/heroes, if per-group fields get too many |
| **A\* + string pulling** (funnel algorithm) | shortest path on a grid or navmesh, then pulled taut | the classic for few units, cheap |
| **Fast Marching / Eikonal flow field** | solves the "arrival time" smoothly over the grid; the direction is its gradient | even smoother fields for the horde, varying speeds (roads, creep) |
| **Flow field tiles / HPA\*** | map split into sectors with portals; paths across sectors first, fields only where needed | larger maps (192², 256²) |
| **Continuum Crowds** (Treuille et al. 2006, Supreme Commander 2) | the crowd itself is part of the cost: density and the flow of others slow a path down | the horde spreading around jams, units avoiding the horde's main flow |
| **Density cost field** (simple version of the above) | add cost per zombie on a tile (we already have the spatial grid) | zombies flowing around each other at a breach instead of piling up; units routing around hordes |
| **RVO / ORCA** | local avoidance – walkers predict each other's movement and give way | not path-finding but steering; smoother crowds at gates |

### 12.2 Language: Rust or a C language ❓

No Python: thousands of zombies with collision and path-finding are exactly where Python reaches its limits (see the
cube game's README). It'll be **Rust or one of the C languages**, with my own engine on top of a thin library
(window, input, drawing, sound), like pygame-ce in the cube game. All four are fast enough for the horde.

| Language | Thin library (like pygame-ce) | Pro | Con |
|---|---|---|---|
| **Rust** | macroquad, SDL2 (`sdl2` crate), wgpu | Very fast, no crashes from memory bugs, `cargo` makes libraries and builds easy, ECS fits well (`hecs`, `bevy_ecs`) | Steepest learning curve: the borrow checker fights the usual "objects point at each other" game code, so think in data (ECS) from the start |
| **C++** | SDL2/SDL3, SFML, raylib | Industry standard, most game-dev tutorials and books, full control | Memory bugs crash or corrupt silently, build setup (CMake, libraries) is a pain on Windows |
| **C#** | Raylib-cs, MonoGame, SDL2-CS | Easiest of the four coming from Python, fast enough, great tools (Visual Studio/Rider), garbage collector | Garbage collector can cause small stutters if you create many objects per frame (avoid with pools/structs) |
| **C** | raylib, SDL2 | Simple language, raylib is made for learning | The most manual work: no classes, no generics, managing every allocation yourself |

**Decision (2026-10-15): Rust** – learning Rust is a goal of its own. That means: think in data from day one (objects
refer to each other by ID/index, not by reference) and expect a slower start.

**Which thin library ❓** (open question 17):

| Library | Pro | Con |
|---|---|---|
| **macroquad** | Fastest start: only `cargo`, nothing else to install. Batches 2D sprites by itself, simple shaders, closest to pygame-ce/raylib | Little control over the window (e.g. choosing the monitor, 11.4), global state instead of explicit objects |
| **raylib** (`raylib` crate) | Same API as raylib in C, lots of tutorials, monitor functions, shaders | Builds raylib from C source: needs CMake and a C compiler; the Rust bindings lag behind raylib |
| **SDL3** (`sdl3` crate) | Full control over window, monitors, input, sound | Drawing thousands of sprites quickly is more work (own batching), SDL has to be built or shipped |
| wgpu | Maximum control and speed, learns how GPUs work | A lot of work before the first sprite – slows the game down a lot |

Leaning: **macroquad** for the prototypes, with drawing behind a thin layer of your own (a few functions like
`draw_sprite`), so switching to SDL3 or wgpu later only touches that layer. A good test is a small throwaway prototype:
**10 controllable units plus 2,000 dots following a flow field**. That tests the feel (step 1), the performance (step 2)
and the library at once.

**Throwaway prototype – first results (2026-10-15):** isometric, 10 units (select, box select, right click to move),
zombies walking to the base along a flow field with line of sight, a spatial grid for separation and target search,
fixed ticks (20/s) with interpolated drawing, a seeded random generator and a checksum test.

| Zombies | FPS | Simulation per tick (budget 50 ms) | Drawing per frame (CPU) |
|---|---|---|---|
| 2,000 | 59–61 (vsync) | ≈ 0.4 ms | ≈ 0.3 ms |
| 10,000 | 59 (vsync) | ≈ 2.1 ms | ≈ 1.0 ms |

→ macroquad has plenty of room for the horde, even at 5× the planned peak. Still open before question 17 is
closed: the **feel of the controls** (only the player can judge that) and textured sprites instead of dots.

What macroquad can't do with the window (checked in the miniquad 0.4 source, 2026-10-15):

- **Fullscreen** is always borderless and always on the **main monitor**: no choosing the monitor (11.4), no
  exclusive fullscreen, no changing the resolution.
- **No list of monitors** (names, sizes, refresh rates) – would need own Windows calls (`EnumDisplayMonitors`) and
  then moving the window there with `set_window_position`.
- **VSync** is only a hint to the driver, an **FPS limit** has to be built by hand.
- **Keys are reported by position on a US keyboard** (scancodes): on a German keyboard `+` arrives as `]`, `-` as `/`,
  Z and Y are swapped. Good for StarCraft-style key grids (QWER stays in the same place), but the key bindings
  (11.4) have to show the label of the player's own layout ❓.
- **Alt + key probably beeps** on Windows (read in the miniquad source, not tried): miniquad passes `WM_SYSKEYDOWN`
  on to Windows, which beeps when the window has no menu ❓. So no Alt shortcuts for now (control group "steal" is on Ctrl + Shift instead).
- **No "mouse left the window" event** on Windows (miniquad ignores `WM_MOUSELEAVE`), and no focus event either. The
  last mouse position stays, so screen-edge scrolling kept going on a second monitor and the edge was easy to miss.
  **Solved with own Windows calls** (`src\cursor.rs`, 2026-10-16): `GetForegroundWindow` (do we have the focus?),
  `GetCursorPos` (where is the mouse really?) and `ClipCursor` to keep the mouse inside the window while it has the
  focus, like StarCraft 2's "confine cursor" (on by default, F12; released on Alt+Tab). Not `set_cursor_grab`:
  miniquad doesn't release it when the window loses focus. The setting belongs in the settings file later (11.4).

**Data layout:** no ECS library at first – plain `Vec`s of structs plus IDs (e.g. the `slotmap` crate, so a dead
zombie's ID can't point at a new one). If that gets messy, `hecs` or `bevy_ecs` can come later.

**Toolbox connection:** the windows-toolbox page starts `dist\LastBastion.exe` from this repo (12.7) – with Rust:
`cargo build --release`, then copy `target\release\last_bastion.exe` there (a small build script later).

**Setup on Windows:** `winget install Rustlang.Rustup`. Rust on Windows also needs the linker from the **Visual Studio
Build Tools** ("Desktop development with C++"). If `cargo build` reports that `link.exe` is missing, that's what's
missing. Editor: Zed or VS Code with **rust-analyzer**.

### 12.3 Graphics: the TAB look

TAB is **2D**: units and buildings are pictures drawn from 3D models (pre-rendered sprites), shown from a fixed,
tilted camera on a square grid. **Isometric like TAB is decided** (open question 2, 2026-10-15). Any library that can
draw many pictures quickly can do this, including **macroquad**:

| Part of the look | How |
|---|---|
| Thousands of animated zombies | Sprite sheets in a few large **texture atlases**. macroquad batches quads automatically, and as long as the texture doesn't change it draws tens of thousands per frame in a handful of draw calls. |
| Units facing any direction | Render 8 or 16 directions per animation (mirror left/right to save half). That's how TAB does it. |
| Depth (who's in front) | Sort by the y position at the feet before drawing (painter's algorithm). |
| Fog of war, light, night, fire glow | **Render textures** + **shaders** (GLSL): draw a light or visibility map, blend it over the world. |
| Muted, gritty colours | A colour-grading shader over the finished picture. |
| Particles (fire, blood, muzzle flash) | Own small particle system, drawn in the same batch. |
| UI | macroquad's built-in UI is basic. The StarCraft-style HUD (11.1) will mostly be custom-made. |

**The actual hard part is the art, not the code:** every unit needs its animations (walk, attack, die) × directions as
pictures. The usual way is to model and animate in **Blender**, then render all directions automatically into sprite
sheets with a script. That's a lot of pictures, but it's done once per unit, and placeholder art (coloured shapes, as in
the cube game) is enough for all prototypes.

**Where the pictures come from ❓ (open question 22):** model them yourself in Blender (learnable, but a project of its
own), buy or download ready-made packs (e.g. itch.io, Kenney – free), have them made by an artist, or generate them
with AI tools and touch them up. Not needed before the mini round (step 4).

**Performance:** 120 units + a few hundred colonists are negligible next to thousands of zombies. The horde decides
the performance, see 12.1.

### 12.4 Data-driven from the start

Like the cube game's `data/*.json`: units, zombies, buildings, recipes, waves and map settings live in JSON files, not in
code. Balancing then means editing numbers, not code. Even better: **reload the JSON files while the game is
running** (key or file watcher), so balancing doesn't need a restart.

**Decided (open question 14, 2026-10-15): JSON files, no database.**

- One file per kind of thing in `data\` (`buildings.json`, `units.json`, `zombies.json`, `waves.json` …), e.g. every
  building with HP, wood cost, stone cost, build time, upkeep.
- At start (and on every reload) the files are read **once** into plain Rust structs in memory. While playing, the game
  only reads these structs – that's as fast as numbers written into the code. The file format doesn't matter for speed
  at all, so the most readable one wins.
- Why not a database (e.g. SQLite): no speed gain (the data is a few hundred entries, read once), but harder to edit by
  hand, worse to compare in git, and one more library. A database pays off for millions of rows or many queries, not
  for game data.
- **Change without restart:** saving a JSON file in the editor (VS Code) reloads it in the running game. Later an
  in-game panel in the developer tools (12.6) can change values directly and write them back to the file.
- **In co-op**, changed data would make the PCs drift apart (12.8). Hot reload is therefore singleplayer only, or the
  host sends the new values to everyone as an order.
- **Does the economy work over 40–80 days?** Instead of a separate spreadsheet: a small calculation (a developer tool
  or a test) that reads the same JSON files and plays the economy through day by day. Then there's only one place
  for the numbers.

### 12.5 Simulation and drawing apart

- The game logic runs in **fixed ticks** (e.g. 20–30 per second, StarCraft 2: 22.4). Drawing runs as often as it can
  and interpolates between two ticks.
- That gives **active pause, slow-mo and fast-forward almost for free** (tick 0×, 0.5×, 4×) – real active pause in singleplayer, none in co-op (open question 7).
- Together with an own random generator with a seed, the same seed + the same orders give the same result. That makes
  bugs reproducible, and it's the base for co-op (12.8) and replays.
- The logic always works on a plain **2D grid** (tiles + positions in tile units). **Isometric** (open question 2) is
  only a question of drawing: tile position → screen position for drawing, and the reverse for the mouse (which tile
  did I click?). The throwaway prototype may stay top-down; from step 1 on the game is drawn isometrically, so the
  feel is right early.
- **Saving (open question 9, decided: yes, like Factorio, 11.5):** the whole game state (zombies, infections, noise
  map, timers) has to be writable to a file. Easy if the state is plain data from the start, painful to add later.
  Flow fields don't need saving, they can be recalculated.

### 12.6 Developer tools (build them early)

A round of 2–4 hours is far too slow to test by just playing. From the first prototype on:

- **Cheats:** fast-forward the clock / jump to day X, game speed ×4 (and more), give gold/items, spawn zombies, start a
  wave, reveal the map.
- **Short test rounds:** a setting with very few and very short days (e.g. 5 days of 30 s), so a full round can be
  played through in a few minutes.
- **Debug view:** flow-field arrows, spatial-hash grid, noise and creep values per chunk, FPS, number of objects,
  time per system (path-finding, collisions, drawing).
- **Fixed seed** on demand, so a bug can be repeated. *Prototype:* `--seed N` on the command line; otherwise random.
- **Hot reload** of the JSON files (12.4).

In the prototype so far (2026-10-16): game speed ×1–×8, FPS, ms per tick, checksum (F3), restart with the same or a
new seed (Esc menu). Two buttons in the top right switch between **Game View** (what a player sees: only the playing
field inside the world border, no dev tools) and **Editor Mode** (F10; the whole map incl. the ring outside the border,
plus all dev tools as buttons, so the hotkeys stay free for the game): **View** – whose eyes the editor shows (All:
no fog / Player: the player's fog of war, shared by both co-op players / Zombies: dark where no zombie looks), paint
terrain live with a brush (mountain,
forest, tree, water, erase – indestructible, for testing behaviour and path-finding; whoever stands there is pushed
out), place spawners (click a spawner to select it: its edge glows and can be dragged to resize, the inside moves it,
the name can be typed, Del removes it), a **zombie brush** (1–20 zombies per tick on the brush area while held) and **drop** (1–1,000 at once), each with breed (mix / walker / hunter) and behaviour (storm the command centre / **wander** around until they notice a unit), buildings placed anywhere (not on other buildings), foldable panel sections, 1,000 zombies from all spawners, waves (small: one
spawner / the spawners of one side / of two random sides), remove all zombies, respawn on/off, a soldier, sniper or
medic per click. All in-game text and
code comments are in English. Every cheat that changes the game is an
order like any other, so it stays deterministic (12.8).

### 12.7 Repository and toolbox

The game has **its own git repository**, completely separate from the windows-toolbox (decided 2026-10-15). It doesn't
know the toolbox and can be built and started on its own.

- **The toolbox only has a small page** (`programs\last_bastion\` in the toolbox repo): pick the game folder, open
  this document, start the game. Without a chosen folder it looks for `last-bastion` next to the toolbox folder.
- **Agreement between the two – keep it stable:**
  - the finished game is at **`dist\LastBastion.exe`** in this repo (built there, not in git), whatever the language
    (C#: `dotnet publish -o dist`, Rust: copy from `target\release\`),
  - the design document is `DESIGN.md` in the main folder,
  - the player's settings are in a fixed place (11.4).
- **On a fresh PC** (2026-10-15): clone the repo, `winget install Rustlang.Rustup` (MSVC toolchain, the default),
  Visual Studio Community or the Build Tools with "Desktop development with C++" (Rust needs its linker `link.exe`
  and the Windows SDK), then `.\build.ps1` → `dist\LastBastion.exe`. Later maybe a ready-built `.exe` as a GitHub
  release.

### 12.8 Co-op: one player hosts

Co-op is wanted (2026-10-15), but without servers of our own: a player starts a co-op round in the game and their
game **is** the server. The others join them.

#### Lockstep (like StarCraft and Age of Empires)

- Sending the state of thousands of zombies over the network every tick is far too much data. Instead, only the
  **orders** are sent (*"units 4, 7, 9: attack-move to x, y"*). Every PC runs the same simulation with the same
  orders and gets the same result (12.5).
- The host collects the orders of all players for each tick and sends them to everybody (a star, not everyone with
  everyone). An order is carried out a few ticks later (**input delay ≈ 100–200 ms** ❓), so it has reached every PC
  in time.
- Unit control is the top priority (7.1), so the delay must stay small and mustn't be felt: the click marker and the
  unit's answer come at once, only the movement waits for the tick.
- The delay adapts to the ping: on a LAN or with a friend nearby ≈ 2–3 ticks (≈ 100 ms), more only on a bad
  connection. The delay is per order, so multitasking (kiting with a ranger while the army fights) doesn't add up.
  StarCraft 2 players micro against humans with the same kind of delay. **Singleplayer has no delay.**
- If one player's orders for a tick arrive late (lag spike), every PC waits for them: the game stutters briefly for
  everyone instead of drifting apart.
- Very little data – normal home internet is enough, even for the host.

#### The price: strict determinism

Same seed + same orders → the **same state, bit for bit**, on every PC. If not, the games drift apart (*desync*).
Rules for the simulation code from step 1 on (drawing, sound and UI may do what they like):

- Randomness only from the seeded random generator of the simulation – never `rand::thread_rng()`, the clock or
  anything else from outside. Same seed + same orders = the same "random" numbers in the same order on every PC, so
  wave directions, hits and desertions come out the same everywhere.
- Randomness that's only for the eye (particles, blood splatter) uses a second generator of its own, so it can't shift
  the simulation's numbers.
- Only the players' orders go over the network and wait for the delay. Everything units decide by themselves
  (target choice, smart targeting, when to fire, zombie movement) is part of the simulation: every PC works it out
  itself on the same tick from the same state, with no delay and nothing sent.
- Fixed order and fixed tie-breaks: units are processed in a fixed order (e.g. by ID), and when two choices are equally
  good the lower ID wins. Otherwise one PC could hand out the targets differently.
- Animations are only drawing. The simulation decides on which tick a shot hits, and the animation is drawn to match.
- Never iterate over a `HashMap`/`HashSet` in the simulation: Rust shuffles their order on purpose. Use `Vec`,
  `BTreeMap` or `IndexMap` instead.
- Floats are fine as long as everyone plays the same `.exe` on x86-64 ❓. `sin`, `cos` & co. can differ between
  systems → lookup tables or a pure-Rust implementation (the `libm` crate) ❓. Fixed-point numbers would be the safe
  way, but more work.
- Orders from the player only reach the simulation through the order list for a tick – also in singleplayer. Then
  singleplayer and co-op run the same code.
- **Desync check:** every few seconds each PC sends a checksum of its game state to the host. Different checksum =
  desync: show it and write the state to a file for debugging. The same check works without a network: same seed +
  recorded orders, run twice → same checksum (a test for the developer tools, 12.6).

#### Connecting without servers of our own

Finding each other through home routers (NAT) isn't reliably possible without *some* server in between. The
options that don't need our own:

| Way | Pro | Con |
|---|---|---|
| **Direct IP** (LAN, or a forwarded port on the host's router) | Nothing needed, perfect for development | Forwarding a port is too much to ask from most players |
| **Virtual LAN** (Tailscale, ZeroTier) | Free, works through any router, no extra code | Every player has to install it |
| **Steam networking** (Steamworks: lobbies, invites, relays run by Valve) | Friends click *join*, routers are no problem, free once on Steam | Only if the game is on Steam (Steam Direct fee), `steamworks` crate |
| **Epic Online Services** (P2P with relays) | Free, no store needed | Epic account and more setup |

Leaning: direct IP / Tailscale during development, Steam networking if the game is ever released (open question 20).
The network code sits behind a thin layer of its own (*send orders*, *receive orders*), like the drawing layer
(12.2), so the way of connecting can be switched later.

#### What co-op changes in the game (2026-10-15)

- **Number of players: 2.** The network code doesn't care how many players there are, so it's written for any number
  – but more than 2 on the same units wouldn't be fun.
- **One colony, both control everything** (open question 19, mode C). Later maybe a second mode: each player has
  their own colony on the same map and they help each other (mode B).
- **Hands off what your ally is using:** once a player gives an order to a unit or a building (not the command
  center), the other player can't give it orders. Units in a player's control group are locked for the other player
  as well – so selecting the whole army (F2) never pulls the ally's units along.
- **When it's free again** (open question 25, 2026-10-15): a unit belongs to a player as long as it's in one of their
  control groups or carrying out their order. In no control group and idle → free again. New units belong to the
  player who ordered them.
- **No pause in co-op** (open question 7) – two players already have twice the APM. Speed stays at 1×.
- **Game menu** (Esc): every player can open it; it doesn't stop the game.
- **Saving:** in lockstep every PC has the whole game state, so **every player** can save a copy – and keep playing
  it alone in singleplayer if they like.
- **Loading** mid-round: **only the host**. It ends the running round for everyone and starts the loaded one.
- A saved co-op round can be **continued with other players** too (someone else takes over the free slot).
- **Dropping out:** a player who loses the connection can rejoin – the host sends them the current state (the same
  data as a save). If the **host** leaves, the round stops for everyone (no host migration – too expensive).

---

## 13. Prototype roadmap

Riskiest and most fun-critical first. Each step should end in something playable.

### Before step 1

These lock everything after them in. The rest of the ❓ in this document can wait for playtesting.

- [x] **Language** (open question 1) – Rust (2026-10-15).
- [x] **Library** (open question 17) – macroquad 0.4 is in use since 2026-10-15 (first window opens); still to
  test with the throwaway prototype (12.2).
- [x] **Learn the basics** – learned along the way while building the game, every piece of code is explained
  (README, rules for Claude). The Rust book and Rustlings stay as a reference (2026-10-15).
- [x] **Co-op yes or no** (open question 3) – yes, one player hosts (2026-10-15). That makes strict determinism and
  orders as a list per tick (12.8) a must from step 1.
- [x] **Saving mid-round** (open question 9) – yes, like Factorio (11.5, 2026-10-15). Plan the game state as
  saveable (12.5).
- [x] **Who builds and gathers** (open question 5) – TAB-style, buildings build themselves (6, 2026-10-15).
- [ ] **Fixed tick + seeded random generator** (12.5) and **developer tools** (12.6) planned from the start – built
  into the first real code, the throwaway prototype (2026-10-15).
- [x] **Repository** (12.7) – own repository, separate from the toolbox (2026-10-15).
- [x] **Build on a fresh PC** (12.7) – rustup (MSVC) + Visual Studio with "Desktop development with C++", then
  `build.ps1` (2026-10-15).
- [ ] **Economy check** (5.3, open question 14) – do the numbers work for 40 days as well as for 80? Done as a small
  calculation over the JSON data files (12.4), not a spreadsheet – can wait until before step 6.

### Steps

| Step | Goal | Done when |
|---|---|---|
| **1. Unit control** | Select, control groups, move, attack-move, one ability, against a few dummy zombies. *2026-10-16: all StarCraft 2 control commands and the bottom bar are in, with soldier and sniper and zombie melee – only the ability is missing* | Moving 10 units *feels good* |
| **2. Horde stress test** | 2,000+ zombies flowing towards a point with flow fields + spatial hash | Stable 60 FPS |
| **3. Walls, towers, infection** | Build walls/towers on a grid, zombies attack them, infection spreads | A breach feels dangerous |
| **4. Mini round** | 10 minutes: tents pay gold directly (no production chains), 2–3 unit types, 3 zombie types, walls, 2 waves + a final wave | **Is it fun?** If not, the factory won't save it – fix the core first |
| **5. Co-op mini round** | The mini round for 2 players over the network (direct IP / LAN): host and join, orders per tick, desync check (12.8) | Two PCs play the mini round to the end without a desync |
| **6. Machines and production chains** | Drill → colony stock → smelter → workshop, with power | A small chain runs by itself |
| **7. Houses, gold, upkeep** | Houses consume and pay gold, units cost upkeep, downgrade | Economy and army are linked |
| **8. Map generation** | Noise terrain, patches, nests, POIs, fog of war | Two seeds feel different |
| **9. Noise and creep** | Noise attracts waves, nests spread creep | Expanding is a real decision |
| **10. Full round** | Timeline from section 4, days to choose, final wave, win/lose, save and load (11.5) | A 40-day round (≈ 2 h) can be played start to finish |
| **11. Home screen and meta** | Regions, round setup, score %, research | The "one more round" feeling |
| **12. Polish** | Sounds, effects, alerts, balancing, tutorial, settings menu (11.4) | Someone else can play it without help |

---

## 14. Open questions

| # | Question | Options | Decision |
|---|---|---|---|
| 1 | Language? | Rust, C++, C# or C – own engine, no Python (12.2) | Rust – learning it is a goal of its own (2026-10-15) |
| 2 | Perspective? | 2D top-down (Factorio) or isometric (TAB) | Isometric like TAB (2026-10-15). The logic stays a square grid (12.5) |
| 3 | Singleplayer only, or co-op later? | Co-op needs determinism / networking from the start | Co-op: one player hosts in the game, no servers of our own, lockstep (12.8, 2026-10-15) |
| 4 | Supply houses by belt or by radius? | Belts (deep) vs. warehouse radius (simple) | Radius around a market – no belts at all (2026-10-15) |
| 5 | Who builds and gathers? | StarCraft-style worker units (then right click = *gather*, command center makes workers) vs. TAB-style: buildings build themselves with colonists, gathering buildings work on their own | TAB-style: buildings build themselves, no worker units. Colonists are only decoration (5.6, 6, 2026-10-15) |
| 6 | Ammo as an item? | Adds logistics depth vs. more micromanagement | Free for now (2026-10-15), ammo as an item is parked |
| 7 | Pause or slow-mo? | Full active pause (TAB) vs. 0.5× slow-mo | Real active pause, singleplayer only. No pause in co-op (2026-10-15) |
| 8 | Hero unit? | Yes (one per round, levels up) / no | Parking lot (2026-10-15) |
| 9 | Save rule? | Free saving / one autosave (TAB) / both as options / no saving mid-round | Like Factorio: free saving, autosave, quick save, load screen with date, time played and screenshot (11.5, 2026-10-15) |
| 10 | Meta progression: power or sidegrades? | | Like TAB: research tree with unlocks and upgrades (2026-10-15) |
| 11 | Final name? | | **Last Bastion** (2026-10-15) – treated as final until there's a better one |
| 12 | Gold runs out? | Units fight worse (morale) / units desert / only no new purchases | Debt in stages: fight worse, then desert one by one at random, weighted towards expensive and non-veteran units (7.2, 2026-10-15) |
| 13 | If only one pillar could stay, which one? | Factory-base / army control / one-run story | The TAB side: horde against a grid-built base, isometric (1, 2026-10-15) |
| 14 | Does the economy work over 40–80 days? | Spreadsheet: income per house tier, build times, upkeep of a 60–80 unit army (5.3) | Numbers in JSON files, no database; checked by a small calculation over those files (12.4, 2026-10-15). The check itself is still to do |
| 15 | Own repository for the game? | One repo with the toolbox / own repo in a sub-folder / completely separate repo (12.7) | Completely separate repo, the toolbox only has a page that starts it (2026-10-15) |
| 16 | Where does the settings file live? | In the game folder (portable) / `%APPDATA%\Last Bastion` (11.4) | `%APPDATA%\Last Bastion`, saves too (2026-10-15) |
| 17 | Which Rust library for window, input, drawing, sound? | macroquad / raylib / SDL3 / wgpu (12.2) | Before step 1 – leaning: macroquad behind an own thin drawing layer. In use since 2026-10-15, to be confirmed by the throwaway prototype |
| 18 | How long is one day? | ≈ 2 / 3 / 4 minutes of game time (4.1) | Rounds should last 2–4 hours, with longer days than TAB and so fewer of them (2026-10-15); exact value by playtesting, first guess 3 min |
| 19 | Co-op: shared colony or one each? | One colony and one pot of gold, each player commands their own units / each player has their own colony on the same map and they help each other / one colony, everyone controls everything | First only "one colony, everyone controls everything", with locks on what the ally uses; "one colony each" maybe later (12.8, 2026-10-15) |
| 20 | Co-op: how do players find each other? | Direct IP / virtual LAN (Tailscale) / Steam networking / Epic Online Services (12.8) | Direct IP / Tailscale for development, Steam if released (2026-10-15) |
| 21 | Which era, how advanced is the technology? | Steampunk / today / near future / sci-fi like StarCraft | Open. Decides the look of units (7.2), towers (tesla?) and later air units |
| 22 | Where do the pictures come from? | Blender yourself / bought packs / an artist / AI + touch-up (12.3) | Open – not needed before the mini round |
| 23 | Early power source for tents? | Wood-fired generator / windmill / the command center gives a little | A little from the command center + small, inefficient wood generators (5.3, 2026-10-15) |
| 24 | Do buildings need workers? | A number like TAB (houses give workers) / no workers at all | Colonists are a resource: buildings need them (switched off without), units take them (5.6, 2026-10-15) |
| 25 | Co-op: when is a locked unit free again? | Never (until given away) / when it's in no control group and idle / after some time | When it's in none of the player's control groups and idle (12.8, 2026-10-15) |

---

## 15. Idea parking lot

Ideas that don't serve a pillar yet, or are too big for now. Not deleted – parked.

- **Day/night cycle:** more zombies at night, lights cost energy but reduce danger. Would fit the days the round is
  counted in (4.1).
- **Weather:** fog reduces sight, storms knock out solar.
- **Trains** for late-game logistics between distant ore fields.
- **Drones** that deliver items (late tech).
- **Refugees** arriving mid-round: accept them (more houses needed) or turn them away.
- **Infected refugees** – one of them might be a carrier. 😈
- **Mutations:** each round the horde gets a random modifier ("runners are faster", "brutes explode").
- **Map editor** like the one planned for the cube game (StarCraft 2 editor as a model).
- **Mod support** via the JSON data files.
- **Replays** (needs determinism, see 12.1). Almost free with lockstep (12.8): a replay is the seed plus the list
  of orders.
- **Belts** (Factorio-style item transport): taken out on 2026-10-15, replaced by the colony stock (5.4).
- **Ammo as an item** with an armory (iron + coal): parked 2026-10-15, ammo is free for now (open question 6).
- **Air units**: parked 2026-10-15 until the era is clear (open question 21). Something simple like a hot-air
  balloon would fit a low-tech era.
- **Rally point into a control group**: new units join control group N by themselves. Parked 2026-10-15.
- **Crawler** (zombie that burrows under walls): taken out 2026-10-15.
- **Units from the first draft (taken out of the roster on 2026-10-15):**
  - *Scout*: fast, long sight, *Flare* reveals an area. Partly covered by the ranger now.
  - *Engineer*: repairs and builds in the field, deploys a temporary turret.
  - *Grenadier*: area burst with *Cluster grenade*.
  - *Hero*: one per round, levels up, 2–3 abilities (see open question 8).
