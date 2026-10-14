---
design_status: proposal
date: 2026-10-11
decision_refs: []
---

# Behaviour AI: family defence and pack tactics (2026-10-11)

How Cubarium does coordinated, multi-animal behaviour. The first cases are the loftstrider
family defending itself and its calves, and the Veilhound pack hunting it. The same
machinery carries into the trained brains (§10). This is a proposal: Wrysk's calls are
recorded in §0 with his words, and nothing here is canon.

Evidence:
- Code survey: `design/7_Research/behaviour-ai-code-survey-2026-10-11.md`, cited below as
  *survey §n*.
- Ethology: `design/7_Research/behaviour-ai-ethology-2026-10-11.md`, cited as *eth §n*.
- Handoff: `design/handoffs/behaviour-ai-2026-10-11.md`.

## 0. Wrysk's words and calls

**The ask (2026-10-11):**
> giraffe or elephant defense. it takes a pack coordinated effort to take down an adult.
> multiple adults together can generally defend themselves. lone juveniles are the most
> defenseless. they should generally run to the adults (but are probably slower than adult
> veilhounds) and adults alert and run to defend the babies. should balance defense with,
> sometimes veilhounds do win and inflict enough wounds to take down an adult, or are able
> to overcome the juvenile, or are able to isolate one individual from the pack or corner it
> or intercept it from joining the others.

**Calls**, in the order they're taken, one at a time:

| # | Topic | State |
|---|---|---|
| 1 | Coordination: signals (sight and calls with range) or a shared group mind | **signals** (Wrysk, 2026-10-11: "suggestion B seems right"); the pack record stays as bookkeeping only (§2) |
| 2 | Would a brain learn to protect a calf (group reward) | **kin-weighted** (Wrysk, 2026-10-11: "i think lean more towards C but B would be okay probably but idk abotu the trainginn noise"). Each body keeps its own variant; its score adds a share of its relatives' scores. Shares are knobs (own calf, mother, sibling, packmate; 0.5 to start). The mother is recorded at birth. A frozen-brain run measures the noise first; a lineage whose signal stays at noise falls back to one shared family score |
| 3 | Where the deciding brain ends and the moving brain starts | **two learned layers with named intents** (Wrysk, 2026-10-12: "i guess B makes more sense to start with, and we get output and feedback faster from the training to see if its working"). A slow top layer picks and holds a named intent; a fast bottom layer reads the senses plus that intent and steers. The rules are teachers only. Training is staged: the top first, with the bottom frozen as its copy, then both. A plain brain is a one-item menu. Self-invented modes are a later study: "only after training B and showing better fitness than baseline, then maybe we try C and see if we can get any improvement over B? otherwise it wont be worth using either". This replaces plan item 23's "one net, two heads, scripted skills; layered later" (§2.2) |
| 4 | Which animals get a menu (lanternjaw, frondgrazer) | **menus from the start for seedporter, loftstrider, Veilhound and lanternjaw; frondgrazer trains plain now, then a menu trial** (Wrysk, 2026-10-12: "yeah C sounds fine"). The other four prey stay plain (one-item menus). Once the two-layer brain lands, the frondgrazer trains both ways (plain vs graze / bunch / hunker / shelter / rest); with 40–90 per world it is the quickest test of whether the top layer pays. If the menu wins, the other prey get menus (§5) |
| 5 | How a family avoids being caught asleep | **body fixes plus "sleeps lighter when it sees no one awake"** (Wrysk, 2026-10-12: "would you recommend B? it sounds more interesting to watch"; recommended, taken). Mostly light rest at reduced range; deep sleep in 1–2 min bouts, only deep sleep blind; a rush's footfalls wake sleepers; rest spots away from hound scent; deep bouts rare while the animal sees no relative awake (one knob, also for the Veilhounds at the den). Deep versus light is the body's; the brain picks when and where to rest (§3.4) |
| 6 | How dangerous a kick is | **kicks wound** (Wrysk, 2026-10-12: "yeah A sounds reasonable"): about a third per landed kick on a grown hound, three in one fight kill; strength scales by body size and is a knob. His worry, kept as a watch item (§7): "hopefully the brains dont just decide that loftstriders are too challenging and they should eat like all the seedporters and bellwings instead and ecosystem crashes" |
| 7 | Does a calf treat its own mother differently (the record exists: `Animal::mother`, SP-NURSE) | **bond both ways** (Wrysk, 2026-10-12: "lets do A"), plus his addition: "young loftstriders and non-infant seedporters should have some level of curiosity to stray from their parent and explore though, increasing the older they get" (§3.5) |
| 8 | How often the pack should win | **a rare feast** (Wrysk, 2026-10-12: "(a) makes sense"): about one loftstrider a world-year, mostly strayed calves; an adult once in several years (lone, lame or old); a standoff every 1–2 h that usually ends with the pack giving up; a hound killed by kicks about once in a few years. A target the knobs are turned to bracket, not a gate (§6). **Accepted for now** (Wrysk, 2026-10-13: "accept loftstrider hunt results for now (option 1)"): 16 eight-hour wide worlds with the defence on had no loftstrider wounded or killed and one hound kicked dead; no knob changes; a 20-world-hour run on 8 seeds reads the first calves and strays |
| 9 | Look of the new poses (kick, guard stance, calf tucked in, hound crouch) | when there is something to show |
| 10 | How training compares animals, so the family benefit survives scoring (Astra review #1–3) | **matched copies of the staged encounters, kept open** (Wrysk, 2026-10-12: "i guess (a) but hard to say and we might want to keep the options open or revise once we start training"). A staged scene runs twice from one start, one animal on `+ε` and `−ε`, all else identical; live windows keep today's per-body comparison for everyday skills. The comparison is a switch, not a commitment: the shared family variant and matched copies of whole live worlds stay available, and the first signal read decides whether to revise (§11.7). **Revised 2026-10-13:** live training uses whole-lineage twin worlds (Wrysk: "accept twin world training"), kin `comparison = "twins"` (§11.7) |

## 1. What happens today

Facts from *survey §2–5*; derived numbers are hand calculations, not measured.

**Why resting loftstriders die** (10 of 11 probe kills were Resting or Asleep animals on
open ground, 10–31 m from the den):
- An asleep body neither senses nor runs a controller (`perception.rs:718`, `step.rs:1557`).
- Sleepers wake only to a *fleeing prey* within hearing, a hit, emergency hunger or the end
  of their bout. A rushing Veilhound isn't "running" in that sense, so a rush wakes no one
  (`rhythm.rs:371-408`).
- Each family member sleeps on its own clock: nobody keeps watch.
- Rest sites are wherever the animal stands. The loftstrider reads no hunter scent.

**Awake, the family barely defends itself:**
- An adult facing one pressing Veilhound within 6 m stands still facing it, with no time
  limit. That has no mechanical effect beyond halving the pack's prospect.
  - It flees from two or more (`group_min` 2).
  - This is the reverse of the field record: moose that stood at bay all survived (0 of
    36), while 5 of 41 that ran were killed (*eth §1.3*).
- A calf flees one Veilhound **alone**, away from its adults (its `need` is 1). No adult
  ever comes to it.
- There is no kick, no counter-strike, armour 0 and agility 0. The `Counter` seam exists but
  only the lanternjaw uses it.
- The loftstrider reads alarms only while asleep. It has one kin slot, and nothing in its
  behaviour ties a calf to its mother. (At the survey, no parent id existed; `Animal::mother`
  has since landed with SP-NURSE, §3.5.)

**The pack is crude and partly telepathic:**
- `pack::known` reads every awake packmate's prey tracks within 10 m straight from their
  records.
- Target choice reads every prey's true position.
- The driver walks at the target; flankers go to fixed points 1.5 m either side, on open
  ground only. There is no encircling, no cutting off and no intercepting.
- A strike hits the nearest prey in the jaw's reach, whatever the target.
- `calling_ticks` is saved and never read.

**Brains:**
- Flee, herd and pack reflexes run only for heuristic bodies (`step.rs:1776-1781`).
- The hybrid option head is 0 wide, and no option menu exists. `herding::stay_with_group`
  is the only skill written.
- The Veilhound's Social block reads the target's true position.

**Speeds** (*survey §4–5*):

| | |
|---|---|
| Adult loftstrider | 2.2 m/s, no sprint |
| Loftstrider newborn | about 1.0 m/s |
| Veilhound cruise | 1.4 m/s |
| Veilhound rush | 3.5 m/s for 20 s, then 120 s to recover |

A Veilhound hit wounds a full-grown loftstrider 0.36 (three hits kill) and kills a newborn
in one. One Veilhound's size window already reaches a young adult loftstrider.

## 2. Architecture (topic 1: signals, Wrysk's call)

Three layers per social animal, with body constraints wrapped around them as today (sleep,
DROP, stamina, wading, physics).

### 2.1 Senses, including calls

Everything one animal knows about another arrives through a sense.

**Sight tracks gain what a watcher can see:**
- the tracked body's facing relative to the observer (from the track's `heading`);
- a **stance**:
  - for the loftstrider, `Guard` (braced, facing a threat);
  - for the Veilhound, `Rush` (fast run, derivable from speed);
- `Lame`: wound above `lame_wound`, a visible limp;
- size, which the track already carries and from which age follows.

**Kin slots.** Loftstrider and Veilhound `kin_tracks` go from 1 to 4. A family or pack
member can then see its group.

**Calls** are a new substrate:
- **The record.** A call is an event `{caller, lineage, kind, x, y, z, tick, subject?}`
  written in the serial write side into a short buffer. The buffer holds about 2 s and is
  not saved.
- **Hearing.** Calls are heard at the hearer's next remote sensing, within
  `range_m × loudness`, through the pair's terrain ray (×0.25 occluded, as hearing today).
  The latency is a fraction of a second, as for today's bolt alarm.
- **Knobs per lineage and kind:** range, cooldown, and who heeds it.

| Lineage | Calls |
|---|---|
| Loftstrider | **Alarm snort.** An adult or juvenile that sees a pressing threat. |
|  | **Distress bleat.** A calf chased, struck or alone with a threat. |
| Veilhound | **Hunt cue.** On committing to or rushing a target. The record keeps the target for diagnostics, but a hearer gets only the caller's bearing; it adopts the quarry only when it has its own track on it and sees the caller heading at it (§11.2). Field studies report no hunting signals, only positioning (*eth §3.2*). |
|  | **Regroup howl.** After a hunt or at the den. |
| Frondgrazer | Today's bolt alarm moves onto the substrate unchanged (§5). |

**Removed privileged reads.** `pack::known`, true-position stragglers, the Social target's
true pose, "is the prey aware of any pack member" (read from the prey's record) and
`window_body`. Each becomes the reading hound's own tracks plus calls it heard.

**What a brain gets.** Per call kind it can hear: valid, sin, cos, proximity, age. Also the
stance and lame bits on its slots, and its kin slots.

### 2.2 Intent: a per-species option menu

At each controller sample (4 Hz), the animal scores every option on its menu from its own
percept and the calls it has heard. It takes the best one, with hysteresis:
- `hold_s`, the minimum hold, about 1 s;
- `switch_margin`, how much better a new option must score.

Rules:
- Every weight is a knob.
- Every term is a plausible response to what the animal senses: heuristics are teachers.
- The chosen intent and its subject are recorded apart from the executed steering source
  (`Steer`), which a physical override can replace (§11.3).

For the brains (topic 3, Wrysk 2026-10-12), **this menu is the top layer's vocabulary**:
- the top layer runs at the intent rate (4 Hz) with the same hold and switch margin, and picks
  a named intent (plus a couple of spare, unnamed ones the rules never use);
- the bottom layer runs at every controller sample, reads the senses plus the held intent,
  and steers. It is
  learned, not scripted, so how an intent is carried out can improve past the rules;
- cloning: the top copies which intent the rules held (cross-entropy); the bottom copies the
  rules' actions under that intent;
- training is staged: the top trains first with the bottom frozen at its copy, then both;
- a plain brain is a one-item menu, so every lineage runs the same two-layer brain.

**Intents carry a subject** where they need one (which calf, which target, which place): the
top layer picks the intent and a slot from what it senses, and the bottom reads that slot
marked. This is how target choice (§4.3) becomes the brain's own.

**Actions** (Wrysk 2026-10-12 left the fidelity of the action interface to this thread). The
bottom layer gets exactly the actions that change something in the world:
- new in this pass: the aimed kick and the calls (which kind), both needed for defence and
  tactics;
- later, after the first brain results: **look** (head direction within the neck's reach, head
  up or down). Sight is a cone from the head and grazing blinds it (`perception.rs:426-438`),
  so looking up costs feeding time. That is the real vigilance trade-off, and the "many eyes"
  effect (less looking per animal in bigger groups) could emerge from it. It needs head turns
  on the rigs too;
- never: display-only outputs (poses, ear or tail moves). They come from motion and intent;
  as learned outputs they would be pure noise to training.

**Player commands (later, when the game package comes; Wrysk asked to keep it in view).** The
two-layer brain gives the hook: a command sets the top layer's intent and subject ("go there",
"guard that calf"), and the bottom carries it out as the animal itself would move. Whether an
animal may refuse (a command as a weight on that intent rather than a hard set, so fear can win)
is a game-design call for then. Training credits or blames the top layer only for stretches it
chose itself; the bottom layer's experience under a command still counts.

How the brain is built, rewarded and trained is §10. This replaces plan item 23's starting
form (one net with two heads, option 0 = own steering, scripted skills underneath). Self-invented intents (no names, nothing to copy) are a later
study, only once the named form beats the heuristic baseline.

### 2.3 Skills

In a heuristic animal each intent runs a skill: a local steering rule that writes forward,
turn, feed, sprint and `kick`. In a brain animal the skills are the bottom layer's teachers
and the source of its intent labels; they never steer it. For
the loftstrider and Veilhound they replace three reflexes in the serial chain:
- vigil/flee (#8);
- the herd pull (#5);
- the pack tactic (#14).

The other reflexes keep their places: shallows, DROP, wade-out, fed rest and the den walk.

### 2.4 Why signals and not a group mind

- **Field evidence.** Pack encirclement needs only positions: approach to a safe distance,
  then spread from packmates near it (Muro 2011; Escobedo 2014, *eth §5*). Lions in 486
  observed hunts coordinated by position alone (Stander 1992).
- **The listed tactics only exist if knowledge is local.** Isolating, cornering and
  intercepting a calf mean anything only when information has range and can be cut. A
  shared record makes every family member know at once that a calf is in danger, through
  rock.
- **Failures look like animals.** A calf out of earshot behind a ridge gets help late, and
  a hound that loses the pack hunts alone.
- **Brains learn only from what they sense.** Today's Social block shows the brain a target
  chosen with true positions.
- **The pack record stays as bookkeeping only:** den, breeder, season, litters, feast. None
  of it is tactical.

The alternative (topic 1, option A) keeps one record per family and pack that members read
and write. It is simpler and exact, and it rules out the above.

## 3. Loftstrider: family defence

### 3.1 The menu

Real large herbivores defend in ways that are documented:
- elephant calves bunch inside the family;
- bison calves run to the cow, the herd or the nearest bull;
- cows and bulls charge the wolves;
- a herd that panics and runs loses its newborn (*eth §1.2–1.3*).

| Option | Who | Skill | Scored up by |
|---|---|---|---|
| graze | all | today's forager | hunger, no threat |
| stay with family | all (today calves only) | `stay_with_group`, comfort `comfort_m` | distance to sensed kin |
| watch | all | stop eating, face the threat; give an **alarm snort** once it presses | a threat seen, not calm by FEAR |
| go to adults | calf, juvenile | run to the nearest adult (its mother first, topic 7), to the point on the far side from the threat | threat sensed or alarm or distress heard × youth; falls once tucked in |
| go to calf | adult | run to the guard point between a threatened calf and the threat | a distress call heard, or a calf nearer the threat than any adult; mother weight (topic 7) |
| guard | adult | hold the guard point facing the nearest hound; **kick** a hound inside reach; short charge-and-return | hounds within `guard_m`, calves behind it, its condition |
| move off | all | walk the family away from a threat seen at range, at cruise | threat far, no calf in reach of it |
| rest | all | lie down near kin, away from hunter scent (§3.4) | tiredness, phase, kin near, no threat |

**Flight changes meaning.** Today's "adults flee from two or more" goes. A loftstrider at
cruise (2.2 m/s) can't outrun a rush (3.5 m/s for 20 s), and its calves are slower still.
So, as for moose and bison:
- standing in a group with calves inside is what works;
- running is what gets an animal singled out.

The world makes that true through kicks, the rush and the target choice in §4, not through
a rule that forbids running. **Move off** is the early, unhurried retreat; it is open to a
family that sees the pack at range.

### 3.2 What the menu looks like on the cube

- **A pack seen at range.** Heads come up, the family drifts closer together and walks away.
- **A pack that presses.** An alarm snort. Calves run to the adults and tuck in behind them.
  The adults turn out to face the hounds.
- **A calf caught away from the family.** It bleats and runs toward the adults. The nearest
  adult (its mother first) comes at a run to stand over it.
- **Hounds that close in.** They meet kicks, and they circle looking for a side no adult is
  facing.

### 3.3 The kick (topic 6)

The kick is an aimed action of an adult in **guard**:
- **Windup** `kick.windup_s` (about 0.4 s), then a hit test against the targeted hound's
  body within `kick.reach_m` in the front arc.
- **Cooldown** `kick.cooldown_s` (about 2.5 s).
- The hound dodges at its agility × awareness, as prey dodges a strike today.
- **Damage** uses the counter formula: `damage × size_self/size_hound`.

**One kick, one budget** (Astra review #6). The aimed kick and a reactive kick-back are the
same physical kick: one state machine with one windup, front arc and recovery, whichever
asked for it. A bite that lands while the kick is ready and the biter is in the arc draws a
kick-back that spends the recovery; a bite during recovery or from behind draws none. So an
adult can't answer four hounds on four sides at once. Today's `Counter` (`step.rs:3867`,
`counter_answer` `step.rs:3964`, no facing, no recovery) is not reused for the loftstrider;
the lanternjaw keeps its own. A dodged bite draws no kick-back.

Geometry is stated in the bites' own convention (jaw offset, sweep, both footprints;
`step.rs:3528-3571`): kick origin, front arc, height band, tracking during windup and terrain
obstruction (§11.4). Placeholder: reach about 1.3 m against the hound's 0.84 m bite, measured
the same way, so a hound must step inside the kick zone to bite.

**Deterrence then comes from cooldowns and facing, not from a count rule.**
- One adult can hold one or two hounds off. Four overwhelm it: it can face and kick only one
  at a time, and the others bite its flanks.
- Two or three adults side by side cover each other's flanks.

Field data fit this: lion claw marks on surviving giraffe are mostly on the hindquarters,
and wolves "avoided the charges" of bison bulls (*eth §1.1, §1.3*).

Wrysk's call (topic 6, 2026-10-12): **kicks wound.** `kick.damage` is set so a landed kick
takes about a third off a grown hound (the counter formula's size ratio is about 1.9, so
about 0.18; the lanternjaw's counter value 0.7 would kill outright). Two kicks leave a hound
at about 65 % speed; three in one fight kill it. A hound dies only by pressing on after being
hit. Field accounts say adult giraffe kicks can kill lions; no measured rate exists.

### 3.4 Rest and sleep (topic 5)

Wild giraffe at night (*eth §2.1*):
- lie down 50–68 % of the 2 h after sunset;
- deep-sleep bouts last about a minute (median 55 s, mean 2 min);
- at least one, often three, stand guard;
- at most three are in deep sleep at once;
- they rest in the open with sight lines, the older ones on the edge.

Elephants sleep about 2 h a day.

Wrysk's call (topic 5, 2026-10-12): the physiology below plus the "sleeps lighter" rule,
generalised from a lone animal to any animal that sees no relative awake. No explicit guard
rota: Wrysk picked the local rule, and a rota would need more machinery than it buys. The
guard is probabilistic, not guaranteed: the readout is the fraction of rest time the whole
family is in deep sleep at once (Astra review #7).
- **Rest is mostly light.**
  - A resting loftstrider lies down and senses at reduced range (`rest_sense`).
  - Deep sleep comes in short bouts (`deep_bout_s`, about 1–2 min) separated by light rest.
  - Only deep sleep is blind.
  - Bouts are independent per animal, so a family of three is rarely all deep at once.
- **Sleepers hear loud things.** A rushing hound's footfall counts like a fleeing prey's.
  This fixes the "a rush wakes no one" gap.
  - A stalking hound, slow and quiet, still doesn't wake them. Stalking pays, as it does for
    lions.
- **An animal sleeps lighter when it sees no relative awake**: its deep-bout chance falls by a
  knob (`rest.deep_alone_scale`) while it senses no kin that is up or in light rest. A guard
  emerges from what each one sees. Ducks at the exposed end of a row sleep with one eye open
  more often (Rattenborg et al. 1999).
- **Rest sites read hunter scent.** A resting loftstrider picks low `Scent(hunter)` within a
  short walk.
  - The den area is soaked in it, so families stop bedding down 10–31 m from the den.
  - The field already exists, and the v3 brain block already reads it.

### 3.5 Calves and mothers (topic 7)

The parent id now exists: `Animal::mother` (SP-NURSE, main fd1a5c90) is set at birth for every
live-bearer's young, and seedporter young already trail their mother (`nurse::mother_pull`).
Topic 7 asks whether the loftstrider uses it in behaviour. Two choices:
- **Bond.**
  - The calf trails its mother and goes to her first, when she is reachable without crossing
    a hunter; otherwise the nearest safe adult (Astra review #10). `nurse::mother_pull` only
    serves nursing seedporters, so a sensed mother-follow primitive is extracted from it for
    any age and lineage (§11.6).
  - The mother's *go to calf* weighs her own calf above others. Other adults still defend
    any family calf: elephant allomothers do, and bison bulls do.
- **No bond.** Calves go to the nearest adult, and every adult weighs every calf the same.

Wrysk's call (topic 7, 2026-10-12): **bond.** Brains get a "mine" mark on mother and calf in
their kin slots, so the kin-weighted reward has a cue to act on.

**Straying grows with age** (his addition):
- **The leash loosens.** The follow's comfort distance grows with age, from tight at birth to
  loose at independence (`young.follow_m` at birth, `young.roam_m` at independence). A
  seedporter infant (before `weaning_age_s`) doesn't stray; after weaning its roam grows.
- **Excursions.** A young animal sometimes wanders off toward something it senses (a food
  patch, another animal, an unvisited place), at a chance per hour that rises with age
  (`young.excursion_per_h` at birth and at independence).
- **Coming back.** An excursion ends when the young is alarmed, hears its mother's call, or
  loses sight of its family; then it bleats (the distress call) and its mother comes.
- **In brains.** The young's menu gets an *explore* intent, copied from the excursions. The
  reward stays energy only (curiosity 0, 2026-10-09), so training keeps exploring only where
  it pays (food found, places learnt).
- **Predation.** Strayed calves are the lone juveniles a pack can isolate (7 of 8 Hwange
  elephant kills were lone calves, *eth §1.2*). Straying is where most calf kills should come
  from, and the excursion knobs are balance knobs (§6).

Giraffe mother–calf bonds last 12–16 months, and mothers carry more claw marks
(*eth §1.1*).

## 4. Veilhound: pack tactics

### 4.1 How real packs hunt

- **Wolves** (*eth §4.1*):
  - approach → watch → attack the group → select one by pursuit → capture;
  - a group that stands firm is usually abandoned;
  - success plateaus at about 4 wolves on elk;
  - bison attacks last minutes to 11 h, with the wolves resting 20–100 m off between rushes.
- **Lions** (*eth §4.2*):
  - wings circle wide while centres wait;
  - 73 % of captures come from the ambush side;
  - alone 2 %, cooperative 27 %;
  - adult elephants and giraffe are rarely taken: kills are lone calves, a lagging juvenile,
    or a weak adult.

### 4.2 The menu

| Option | Skill | Scored up by |
|---|---|---|
| rest, den, provision | today's | fed, phase, pups |
| search | today's scent and roam | hunger, nothing sensed |
| stalk | approach a sensed group under cover; freeze when watched (today's stalk) | hunger, prey sensed, cover |
| test | approach openly to `test_m`, hold and watch, then a short rush at the group | a group sensed, nobody selected yet |
| drive | chase the selected animal | it runs, is young, lame or isolated |
| cut off | take the point between the target and its nearest defender or family, outside kick reach | target young or running to adults; a packmate already driving |
| intercept | pursue with lead (aim at the predicted point) | target faster than straight pursuit allows; crossing path |
| encircle | Escobedo's two rules: approach to safe distance `dc` (outside kick reach), spread from packmates near `da` = 1.5 `dc`; `dc` falls as the target tires or is wounded, rises when it kicks | target stands at bay |
| harry | dart in to bite when the target faces another hound or its kick is cooling, dart out | target at bay, encircled, sound adult |
| give up | break off, regroup howl, rest | expected value below zero (§4.3) |
| feed | today's | carcass |

**Roles emerge from local scoring.** A role's score drops when a hound sees a packmate
already doing it: a packmate already between the target and the family lowers *cut off*.

Individual role preference (Stander's lions repeat roles, and success rises when they do)
is a later trait, not in this pass.

**Strikes go to the target.** Feed is held only when the target, or any juvenile with high
`p_kill`, is in the capsule. A hound slipping between adults no longer bites a bystander
adult and eats a kick.

### 4.3 Target choice and giving up

Each hound scores each sensed prey from its own tracks:

> value = meat(size) × p_kill − chase cost − risk

- **`p_kill` rises with:** youth (size), lame, isolation (distance to the nearest sensed
  adult kin), asleep or lying, running away, and no adult facing the pack within
  `guard_m`.
- **`p_kill` falls with:** adults in guard stance near it, and adults facing the hound.
- **Risk:** expected kicks (adults in guard within reach of the approach) × the cost of a
  wound × (1 + own wound). Hunger lowers the risk weight.
- **Give up:** value below zero, or own wound, stamina or time without progress past a knob.
  Then the target is shunned for `shun_s`.

**The test is where the choice gets made.** A family that stands together in guard scores
low, and the pack drifts off to easier prey. That is MacNulty's bison against elk.

A calf that bolts, a lame adult, or a juvenile that strays raises the score, and the hunt
starts on it. Nothing scripts "avoid groups": the risk and `p_kill` terms read what the
hounds see.

## 5. Which species get what

| Lineage | This pass |
|---|---|
| Loftstrider | the full menu, kick, rest physiology, calls, kin 4, mother bond and straying (topic 7) |
| Veilhound | the full menu, calls, privileged reads removed, kin 4 |
| Frondgrazer | HERD and HUNKER stay its defence (armour, not kicks). Its bolt alarm moves onto the call substrate with the same behaviour and the same observation input, so its brain's inputs keep their shape and its plain training can start now. Then the menu trial (topic 4) |
| Seedporter | retreat to the trees stays its defence. An alarm call later, primate-style, on the substrate. Its menu gets "stay with troop" now and alarm later; its post-weaning young stray more with age (topic 7) |
| Lanternjaw | a menu from the start (topic 4): wait in ambush with the lure lit, patrol, feed at a carcass, give way to a Veilhound, flee, rest. Labels from its existing modes (`Steer::Patrol`, `Yield`, the ambush) |
| Others | plain (one-item menus) until the frondgrazer trial says otherwise |

Later and not in this pass:
- cross-species eavesdropping (grazers heeding a loftstrider snort);
- individual hunting roles;
- pack territory fights (recorded 2026-10-09).

## 6. Balance (topic 8)

**Targets:**
- one Veilhound pack and one loftstrider family of 2–6 each on the wide world;
- Veilhound hunt success 15–25 % per chase (predation design, D5);
- "sometimes veilhounds do win".

Wrysk's call (topic 8, 2026-10-12): **a rare feast**, option A below. About one loftstrider a
world-year, mostly strayed calves; an adult once in several years; a standoff every 1–2 h,
usually ending with the pack giving up; a hound killed by kicks about once in a few years.

**Budget** (one world-year = 10 h; [hyp] = not measured):
- **Family births.**
  - One calf per adult per year (season gate, gestation 14,400 s), so about 2–3 calves a
    year from 2–3 adults.
  - Old age takes about 0.3 per animal per year, about 1 a year.
  - Dispersal removes young above 3 adults.
- **Predation allowance.** Births less old age is about 1–2 a year. A family holds at 2–6
  if the pack takes **about one loftstrider a world-year, mostly calves**. An adult should
  die about once in several years: lone, lame or old.
- **Today.** 11 kills in 48 seed-hours, about 0.23/h or 2.3 a year, from founders of 2.
  That is above any allowance: a pair is gone within hours.
- **Engagements.** One every 1–2 h is watchable, 5–10 a year. At 10–20 % success per
  engagement on the family, that gives 0.5–2 kills a year, inside the allowance and the D5
  band [hyp].
- **The pack's food.** A loftstrider kill is a rare feast. Bellwings are 68 % of pack kills
  and grazers most of the rest, so the pack doesn't depend on loftstriders. That matches
  lions on giraffe and wolves avoiding bison where elk exist (MacNulty 2014).

**Knobs that reach it**, by leverage:
1. **Kick** damage, reach and cooldown: the adult kill rate and the length of engagements.
2. **Calf rescue and straying:** alarm and distress range, the adult's *go to calf* weight,
   and the young's roam and excursion knobs. These set the calf kill rate.
3. **Sleep:** deep-bout length, rest sensing, waking threshold. These set kills of resting
   animals.
4. **The hounds' risk weight and give-up thresholds:** the number of engagements and how
   long they last.
5. **Rest-site scent avoidance:** the encounter rate.

**Prediction for the first full build** [hyp]:
- kills of resting loftstriders fall from 10 of 11 to rare;
- engagements per world-hour stay about the same;
- per-engagement success is 15–30 % when a calf is present and under 5 % on grouped adults
  without calves.

The engagement harness (§7) measures the per-engagement numbers in minutes. The wide check
measures the encounter rate.

## 7. Verification

**Law tests.** Mechanism tests on hand-built fixtures, reading values from the config, with
no pinned numbers:
- **Calls.** A call is heard within range, muffled behind rock, unheard beyond it.
- **Sleep.**
  - A deep sleeper misses a stalking hound and wakes to a rushing one within hearing.
  - A light rester senses at the reduced range.
- **Loftstrider.**
  - A calf with a pressing threat moves toward the nearest adult (its mother if bonded) and
    settles on the far side.
  - An adult hearing a distress call moves to the guard point between the calf and the
    hound.
  - A kick lands only after the windup, on a hound in reach and arc, then cools down.
  - A dodged strike draws no counter.
- **Veilhound.**
  - N hounds around a standing target settle outside kick reach and don't overlap; the test
    checks the two forces (approach to `dc`, repulsion from packmates within `da` of the
    target) and bounded approach, not a gap equal to `da` (Astra review #10: in Escobedo's
    model `da` is the distance from the prey at which repulsion switches on).
  - A cut-off point lies between the target and its nearest defender.
  - With config weights, an isolated calf outscores a guarded one and a lame adult outscores
    a sound one.
  - A hound gives up when its wound passes the configured threshold.
  - The pack never targets prey no member senses.
- **Arbitration.** An option holds at least `hold_s` and switches only past `switch_margin`.

**The engagement harness** (package 1):
- **Setup.** A flat scenario map (`train-flat` or a hand-built flat world) with a family (2
  adults and a calf; variants with no calf and a lone calf) against a pack of 2, 3 or 4, at
  random placements.
- **Runs.** 50–100 episodes of at most 5 sim-minutes each, through labd.
- **Outcomes:** calf killed, adult killed, pack gave up, hound wounds and deaths, and
  duration.
- **Arms:** kick on or off, calls on or off, pack size.
- **Cost:** minutes of compute.
- **Video.** One chosen episode rendered headless to MP4 (x264 yuv444p crf 12, today | new).

**The wide check.** 8 seeds × 6 h through labd, staged: an early liveness and mechanism
checkpoint, then the rest only for a stated event question. At the topic-8 target (one
loftstrider per 10 h) 48 world-hours expect about 5 kills, so the check can catch a large
failure and give a rough kill rate against today's 11; it can't settle the rarer targets
(adult and hound deaths). Fewer kills after a family dies out is not success: exposure (hours
with both family and pack alive) is reported beside every rate (Astra review #12). Its
readouts:
- loftstrider deaths by posture and age;
- engagements and their outcomes;
- hound wounds;
- the pack's diet share by kills and by energy eaten (a few large carcasses can dominate);
- family size, births, dispersals and calf availability over time;
- encounters, tests, chases, captures and kills counted apart.

**Prey switching (Wrysk's worry on topic 6, a watch item for every brain training check).** The
Veilhound's list holds the frondgrazer, the lanternjaw, a grounded seedporter and a resting
bellwing as well as the loftstrider (`body.rs:2790-2791`). A brain scored on energy over
10-minute windows cannot see a slow crash coming, so it could overeat easy prey before the
scarcity reaches its meals. Every training check reads, brains beside the heuristic:
- the Veilhound's diet share per prey lineage;
- each prey lineage's numbers over time.

If brains crash a prey, the fix goes to the world (that prey's escape, refuge or abundance),
never to the reward (*shape situations, not behaviour*).

## 8. Packages

Each one ships something visible and testable. Order:

| # | Package | Ships | Lane |
|---|---|---|---|
| 1 | **ENGAGE.** Engagement harness: scenario, outcome readout, MP4; matched starts (§11.7) | today's behaviour on video, the baseline numbers | Codex high (harness on existing study patterns) |
| 2 | **CONTRACTS.** The shared interfaces of §11 in code before lineage work splits: slot and subject encoding, call records and hearing (awake and asleep), intent/subject/override records, the kick state machine and rest request in the action interface, save semantics. No behaviour change yet beyond what the records need | one interface packages 3–7 build on | Opus high (new architecture) |
| 3 | **CALLS-REST.** The call substrate on that contract (loftstrider snort and bleat; frondgrazer bolt ported with its observation input unchanged and its alarm behaviour checked); rest physiology with "sleeps lighter"; a rushing hound and calls wake sleepers; rest sites read hunter scent; kin 4 with the "mine" mark | a family that wakes on a rush or a call | Opus high (new substrate) |
| 4 | **LS-DEFEND.** Loftstrider intent layer and skills; the one-budget kick; `group_min` flight retired; the bond (mother-follow primitive, her weight on her own); defender sector spread | calves run to their mothers, adults bunch, face out and kick | Opus high |
| 5 | **VH-TACTICS.** Veilhound intent layer and skills; tactical memory (§11.5); risk-aware target choice; privileged reads and feast travel removed (§11.2); strike identity; pry and attack accounting ported off the pack target | test, cut off, encircle, harry, give up | Opus high |
| 6 | **YOUNG-ROAM.** Straying with age for loftstrider calves and post-weaning seedporters: the loosening leash, excursions, the distress bleat on losing sight (the seedporter's included) and the mother's response | calves that wander off and get called back | Opus medium |
| 7 | **TWO-LAYER.** The brain of §10.1 with §11.3's timing; cloning with teacher forcing, alternative-mode examples and a closed-loop correction pass; per-mode competence checks; labels for the seedporter and lanternjaw first, then the loftstrider and Veilhound from 4–5, *explore* from 6 | brains that copy their rules mode by mode | Opus high (new architecture) |
| 8 | **KIN-TRAIN.** The reward roster and estimator (§11.7), the comparison switch (matched copies over any saved start, shared family variant, per-body), stage 1 perturbing only the top, staged-encounter episodes in the trainer, the noise and signal checks | training that can value a calf, and the signal read | Opus high (the estimator is new) |
| 9 | **BALANCE.** Engagement sweeps over §6's top knobs; the staged wide check; defaults chosen; report to Wrysk | his topic-8 target bracketed or a stated gap | Opus medium |

Order:
- 1 first: it records today's baseline.
- 2 next. The lineage packages share the controller write path, actions, observation encoding,
  labels and saves (`step.rs:1783-1789`, `2171-2194`), so their common contract lands once,
  before they split (Astra review #13).
- 3 after 2. Then 4, 5 and 7 in parallel: 7 builds against the contract with provisional
  menus, and clones the seedporter and lanternjaw first.
- 6 after 4 (it uses the calf's bleat and the mother's response).
- 8 after 7. 9 after 4–6, and again once trained brains exist.
- The frondgrazer's plain training (ecology-training thread) is blocked by none of these, as
  long as plain-lineage manifests stay unchanged. Its menu trial (topic 4) waits for 7 and 8;
  it tests the menu machinery, not pack credit.

**Cost per tick.**
- The intent layer reads the body's own record (≤ 32 tracks) at 4 Hz, which is negligible.
- Calls are a few per minute. Hearing them is O(calls × candidates) inside the sense pass.
- Removing `pack::known` and the true-position scans removes O(N) loops.
- Expected well under 0.1 ms per tick for the rules [hyp]. The full mixed-world cost (four kin
  slots, call queries, both brain layers) is measured after packages 3 and 7, not bounded by
  package 3 alone.

**Saving.**
- `Animal::mother` exists (schema 79). A kick cooldown or any intent state that must survive a
  save bumps the fauna schema.
- Calls are session state: an old call vanishing on load is harmless. A kick's recovery, a
  brain's held intent, subject, hold age and recurrent state are saved (schema bump, now 80),
  because matched training starts and mid-engagement saves must continue, not cold-start
  (§11.3).

## 9. Rules this design keeps

- **No fitness or brain reward pays for defending, coordinating or kills.** Defence pays
  because calves survive; tactics pay because packs eat.
- **Every new number is a serde knob** with a registry row and a real-world note.
- **Tests check mechanisms,** never tuned values.
- **Heuristic rules read only what the animal senses.**
- **Kin-weighted reward replaces the open pack-shared question** (plan Q4b). A hound that cuts
  off and doesn't eat first still gains through its packmates' meals (topic 2, §10.3).
- **A brain is worth using only if it beats its heuristic** on fitness (Wrysk, topic 3). Every
  training step is set up to show that comparison early.

## 10. Brains: build, reward, training

The brain-design questions moved into this thread on 2026-10-11 (Wrysk: "i think we should
pick up the design of the new brains here since it may be blocking or depend on its
implementation"). Topics 2–4 settled them; this section is the spec they add up to.

### 10.1 Two layers (topic 3)

- **Bottom layer.** Today's per-lineage GRU (P3, merged 71b5d8ef). Its input is the v3
  observation plus:
  - the held intent, one-hot over the lineage's menu (named intents, plus two spare ones kept
    masked until the self-invented-intent study);
  - a mark on the subject's slot, when the intent has a subject;
  - time in the current intent, squashed.

  Its output is today's actions plus the kick and the call kind, where the lineage has them
  (§2.2). It runs at every controller sample.
- **Top layer.** A small head over the bottom layer's hidden state, the held intent and the
  time held. At the intent rate (4 Hz) it gives:
  - logits over the menu;
  - a pointer over the kin and threat slots for the subject.

  `hold_s` and `switch_margin` apply to its probabilities at a fixed temperature, not raw
  logits (§11.3). They are per-lineage knobs, not trained weights.
- **A plain brain** is a one-item menu: the top head is absent and the intent input constant.
  Frondgrazer and littershredder training on today's brain is this case, unchanged.
- **Mixed worlds work.** Brain and heuristic animals coordinate only through sight and calls
  (topic 1), so a brain hound reads a heuristic packmate's hunt cue as it would a brain's.
- **Cost.** The top head is a few thousand weights at 4 Hz, negligible beside the GRU.

### 10.2 Copying the rules

- **The top** copies which intent the rules held, and its subject, by cross-entropy.
  Imitation already fits an option head (`imitate.rs:822`).
- **The bottom** copies the rules' actions under the rules' own intent and subject (teacher
  forcing), so each mode is learnt cleanly. Timing, alternative-mode examples and the
  closed-loop correction pass are in §11.3.
- **Label sources:**
  - loftstrider and Veilhound: their new intent layers (§3.1, §4.2);
  - seedporter: its existing modes and the `Steer` readout (forage, stay with troop, roost,
    groom, follow its mother, flee to the trees);
  - lanternjaw: `Steer::Patrol`, `Steer::Yield` and the ambush and lure states (wait in
    ambush, patrol, feed at a carcass, give way, flee, rest);
  - young loftstriders and seedporters: *explore* from the excursions (§3.5).

### 10.3 Reward (topic 2)

Each body keeps its own antithetic variant. Its score is its own
`(E_end − E_start + E_given)/body_ref`, as today, plus `r ×` each relative's own score over
the same span. Who counts and how the scores are compared are fixed in §11.7: today's
within-world z-scoring cancels the shared family term exactly (Astra review #1), so the
comparison changes (topic 10: matched copies of the staged encounters, kept open).
- **Who counts as a relative.** Relatives come from `Animal::mother` (its calf, its mother,
  siblings sharing a mother) and from the pack record for packmates. The reward reads
  bookkeeping the animal never senses. That is allowed: only its behaviour must come from its
  senses.
- **The share.** `r` is a knob per relation (calf, mother, sibling, packmate), 0.5 to start.
- **The leak.** A relative on another variant leaks its luck into the score, and a neighbour
  on the opposite sign of the same perturbation is an interacting opponent, not noise (Astra
  review #3). §11.7's matched copies remove both.

**Noise check first.** A run per lineage with the brains frozen (no perturbation), as in the
2026-10-08 pilot, shows how much of a window score is luck. A lineage whose training signal
stays at that noise level switches to one shared variant per family or pack (topic 2's B).

### 10.4 Training

- **Stage 1: the top only.** The bottom stays frozen at its copy (the variant builder
  perturbs only the top's weights), so choices improve without wrecking movement. A quick
  signal is a hypothesis, not a promise: loftstrider and Veilhound encounters are rare in
  live windows (eight 10-minute windows hold about one engagement), so the staged encounters
  are the primary early evidence for them (Astra review #3).
- **Stage 2: both layers.**
- **Staged encounters.** Live 10-minute windows hold too few hunts, so the loftstrider and
  Veilhound brains also train on the engagement harness's episodes (family against pack,
  random placements, at most 5 minutes each, package 1). The scores are the same energy terms
  over the episode.
- **Readouts every check,** brains beside their heuristic on matched starts:
  - own energy return and kin-weighted return, raw (the "beats its heuristic" bar);
  - the topic-8 numbers;
  - the Veilhound's diet share by kills and by energy, and each prey's numbers (§7, prey
    switching);
  - hold durations, switch rates and why holds ended.
- **The frondgrazer trial** (topic 4): plain against a graze / bunch / hunker / shelter / rest
  menu. If the menu wins, the other prey get menus.
- **Later studies:**
  - self-invented intents, only once named intents beat the heuristic baseline (topic 3);
  - *look* as an action (§2.2);
  - player commands setting the top layer's intent (§2.2).

## 11. Contracts (from the Astra review, 2026-10-12)

Astra's review (`design/7_Research/astra-behaviour-ai-review-2026-10-11.md`) found the
direction sound and none of Wrysk's calls broken, but the interfaces underspecified. This
section fixes them; package 2 builds it before the lineage packages split. Review finding
numbers are in brackets.

### 11.1 Slots and subjects [#4]

- **Slots.** Bounded lists of kin, threats and, for hunters, prey. A recognised mother or calf
  keeps a reserved kin slot while sensed. Each slot carries what the teacher reads: lineage
  and size; bearing, distance and elevation; heading or a velocity estimate from the track;
  posture with resting and asleep apart; stance (guard, rush); lame; modality and freshness;
  relation (mother, calf, kin, other).
- **Subjects** are typed: none, a body (sensed or remembered), or a place (sensed or
  remembered: a rest spot, an excursion goal, a guard or cut-off point). The selected body is
  held by its id internally and its current slot re-marked at each sample, so slot order
  changing doesn't change the subject. Raw ids never enter the network.
- **Expiry and masks.** A subject unseen for `subject_lost_s` becomes none. Invalid
  intent–subject pairs are masked, and the subject head is conditioned on the chosen intent.
- **Checks:** slot permutation, two targets crossing, a subject dying or vanishing, two
  calves bleating at once.

### 11.2 Calls and shared records [#7, #9]

- **What a hearer gets:** kind, bearing, proximity, age, and the caller's relation if it
  recognises the voice (mother and calf, a per-lineage knob). Never an exact subject id.
- **Hunt cue.** It draws attention to the caller. A hound adopts the caller's quarry only
  when it has its own track on that animal and can see the caller heading at it.
- **Bounded records.** Each hearer keeps a few call records with deduplication and a
  refractory time. Calls are collected by their own range, not through the nearest-animal
  candidate cap (`perception.rs:601-628`).
- **Asleep hearing.** Deep sleepers hear calls and a rush's footfalls above a loudness
  threshold, with the same occlusion as waking hearing. The current wake path
  (`rhythm.rs:371-408`) has no terrain ray; this one does.
- **Shared records stay accounting.** The pack's feast and kill records remain for the census.
  A hound goes to a carcass it senses, remembers or hears a regroup howl from, not to
  `pack.feast` (`pack.rs:1880-1894`). The remaining global reads (membership, hunger, target
  progress, shunning, roles) are audited and either localised or confined to bookkeeping.
- **Focal-invariance check:** moving bodies a hound can't sense, with its percept and call
  history fixed, doesn't change its decision. This replaces "the pack never targets prey no
  member senses".

### 11.3 Intent timing, holding and cloning [#5, #11, #13]

- **Records.** Chosen intent, subject, executed actions and override reason are recorded
  apart. `Steer` stays the executed steering source, which a physical override can replace.
- **Order at a decision sample.** The GRU updates once with the observation and the
  previously held intent. The top reads that state and proposes. Arbitration settles the
  held intent and subject. The bottom's action readout then reads the new hidden state with
  the newly settled intent and subject, in the same sample; the recurrent update reads them
  from the next sample. Cloning uses exactly this order, so the top's label never leaks in
  through its input.
  - *Amended 2026-10-12 after the TWO-LAYER core (cb6f2fd2).* Conditioning the whole bottom
    only from the next sample put every mode change one sample late. On a synthetic teacher
    whose modes act at once, a plain GRU then out-copied the two-layer clone (mode 0.993,
    action error 0.0023 against 0.0146). A same-sample readout removes the lag without
    leaking the label: the top still reads only state built from the previous hold.
- **What the top imitates:** the teacher's proposal before its hold (its best-scoring
  intent). The brain then runs the same hold and margin, so hysteresis isn't applied twice.
- **Hold semantics.** The margin is on probabilities at a fixed temperature, a knob per
  lineage, so a change in logit scale can't change switching on its own. A hold ends early
  when its subject dies or is lost, when its intent becomes masked, or on a physical
  emergency (DROP, wade-out, a hit). The top then chooses again.
- **Cloning data.**
  - Besides the teacher's own choices, staged scenes invoke teacher skills in other modes
    and at transitions, so the bottom learns how each mode moves where the teacher wouldn't
    have chosen it.
  - Then a short closed-loop pass: the learner picks intents, and the teacher labels the
    states it actually reaches.
  - The strategic loss trains the shared representation during cloning.
- **Competence before freezing.** Rescue, guard, retreat and intercept are each checked
  closed-loop on staged scenes. Average action error over mostly grazing samples doesn't
  count.
- **Saves.** Held intent, subject, hold age and recurrent state are saved for brains, so a
  load or a matched training start continues rather than cold-starts.

### 11.4 Actions and the body [#6, #7, #8]

- **The kick** is one state machine with one budget for aimed kicks and kick-backs (§3.3).
  Its geometry is in the bites' convention. It tracks its target during windup within the
  turn rate; whether the body may step during windup is a knob.
- **Rest is an action.**
  - A brain or heuristic requests rest through the shared action interface. The body checks
    eligibility and runs light and deep sleep.
  - A body past its sleep-need ceiling still falls asleep (today's rhythm as the backstop).
  - The same path serves brains and heuristics.
- **"Sleeps lighter" reads sight.** It uses recent visual evidence that a relative is up, not
  a refreshed track timestamp or an unseen animal's true state. A minimum light-rest interval
  separates deep bouts.
- **Strikes keep their target.** The chosen target's id passes into strike creation and holds
  through windup and contact. Taking a juvenile that comes into reach instead is an explicit
  substitution of a sensed animal; a brain does it by changing its subject.
- **Bodies block.** A lunge contacts the first body along its path. An adult standing between
  hound and calf takes or blocks the lunge; there is no invulnerability rule.
- **`window_body` splits.** Whether a bite can take an animal of that size stays a physical
  mechanism on true state (`step.rs:3679-3688`). The hound's sensed estimate of the pack at
  hand goes only into its risk and target choice.
- **Checks:** two prey in one capsule, a calf behind an adult, opposite sides of a ledge,
  ring-seam geometry, a rear approach, two simultaneous bites, a bite during recovery, a
  target moving during windup.

### 11.5 Tactical memory [#10]

The Escobedo rules justify an encirclement skill, not the whole repertoire (they start from
an already isolated prey). The heuristics add local memory:
- target commitment with expiry, and a per-hound approach-side preference as a stable
  tie-break, so four hounds don't all pick *cut off* and then all drop it;
- occupied-sector avoidance from packmates and adults it sees; adults spread to face hounds
  no other adult faces;
- cut-off, guard and calf points checked as reachable and safe; lead prediction bounded by
  terrain and replanned when blocked;
- a packmate's role read from its recent movement relative to the target. An adult's kick
  recovery is assumed only from a kick the hound saw, with its time; otherwise unknown;
- scenarios with asymmetry, occlusion and confinement, not only open-ground circles; feasible
  and failed interceptions tested apart.

### 11.6 Couplings the packages must carry [#13]

- Frondgrazer prying (`vigil.rs:1486-1489`) and attack accounting read physical events or
  local attack state, not the pack target.
- Dispersal eligibility stops depending on `herding.young_only` (`step.rs:1408-1412`), so
  all-age loftstrider herding doesn't switch dispersal off.
- A sensed mother-follow primitive comes out of `nurse::mother_pull` for any lineage and age;
  nursing keeps using it.
- Plain-lineage manifests stay unchanged, so frondgrazer and littershredder training stays
  compatible.
- Founders of one family are recorded as sisters, and of one pack as littermates (a shared
  founding mother id), so kin reward and recognition hold from the first generation, as in
  real elephant families. Sibling relations need a shared real mother id; founders with none
  are not siblings by accident.

### 11.7 Reward roster and comparison [#1, #2, #3] (topic 10)

- **Roster.**
  - Fixed at the start of the window or episode from true relations; a newborn joins its
    mother's roster at birth.
  - Relatives' energy meters persist after death, so a killed calf's loss counts against its
    mother.
  - Each relative contributes its own base score, not its kin-augmented one.
  - One relation per pair, the closest (mother/calf, then sibling, then packmate).
  - Heuristic relatives are metered too (today `window.rs:670-672` drops bodies without a
    candidate).
- **The cancellation.** Today scores are z-scored within each world and lineage
  (`trainer.rs:136-161`). With one pack per world and equal shares, a hound's score is
  `(1 − r)·own + r·pack total`. The shared total is the same for every member, so
  normalising removes it, and the score becomes exactly own energy again. Antithetic twins
  are placed as ring neighbours (`window.rs:228-245`), so a mother and her calf often carry
  opposite signs of one perturbation.
- **Comparison (Wrysk's call, topic 10):** matched copies. A scene is run twice from the
  same start; one animal carries `+ε` in one copy and `−ε` in the other, and everything else
  is identical. The difference in its kin-weighted score is then caused by its own variant,
  relatives included. Focal animal and sign rotate. It is cheap on the staged encounters
  (small scenes, at most 5 min). Live windows keep today's per-body comparison for everyday
  skills, where kin terms add little.
- **Kept open** (his words: "we might want to keep the options open or revise once we start
  training"). The comparison is a serde switch per lineage with three modes:
  - `matched` (the default for staged encounters);
  - `family` (one shared variant per family or pack, compared across worlds; topic 2's
    fallback);
  - `per_body`, z-scored as today (the default for live windows).
  Matched copies are built over any saved start state, so matched copies of a whole live
  window are the same machinery on a bigger scene: a cost choice, not new code.
- **Determinism, measured 2026-10-12 (KIN-TRAIN, `kin::matched::null_pair`).** Two copies
  from one start with identical brains were bit-identical in 32 of 32 staged-encounter pairs
  (300 s, on the single-thread schedule the harness already uses) and in 8 of 8 small live
  windows on one thread. The null-pair noise is exactly zero, so any matched difference is
  the perturbation's effect. A run can keep checking it every generation (`NoiseReadout`).
- **When to revisit**, read at KIN-TRAIN's first signal check and reported to Wrysk:
  - matched differences for a lineage stay at the frozen-brain noise level: that lineage
    switches to `family` (topic 2's fallback, already his call);
  - staged-scene gains don't show in live worlds (calf and hound losses on matched live
    seeds no better than the cloned brain's): a short trial of matched live windows, priced
    and taken to him first (EC2 only via `lab approve`).
- **Revised (Wrysk, 2026-10-13: "accept twin world training").** The first signal read
  (lanternjaw) found per-body live windows blind: the variant's share of a pair difference
  was about 0 at every σ up to 0.1. Twin copies showed a variant moves behaviour consistently
  (ambush/patrol), but a body's energy is mostly which twin caught prey; focal twins sit at
  about 0 because one body's flip reshuffles its neighbours' luck. **All-sign twins** (every
  body of the lineage on `+ε` in one copy, `−ε` in the other, from one start) carry about
  0.1 of the difference. Live training now runs `comparison = "twins"`
  (`config/training/kin-twins-2026-10-13.toml`: 1024 pairs × 2-minute windows a generation).
  The other modes stay as switches.
- **Flat after 3 h (2026-10-13/14).** Lanternjaw (44–60 generations) and seedporter (33) twin
  runs end where they start (brain ≈ rules, a small lead that never grows; seedporter roosting
  falls .56 → .06); Adam steps ~0.001 rms, so each step is mostly luck. **Wrysk, 2026-10-14:**
  "i guess id say go ahead with your suggestion" — measure the gradient's noise first (one
  generation's step estimated twice from fresh luck), then raise the signal per step to fit,
  not a blind long run. And: "i wanna also revisit like how we are training though. i mean
  theres the algorithm we use and then also optimizations for the runs. both could be subject
  to improvements." A review of both comes back to him one topic at a time.
- **Checks.** An algebraic check that a perturbation improving only a relative's energy
  survives the whole scorer. Readouts of raw own, relative and combined scores. Missing signs
  are never scored as zero returns.
- **Signal check, beside the frozen-brain noise check:** repeated matched differences in a
  consistent direction, how often decisions change, held-out energy improvement, and
  candidate coverage.

