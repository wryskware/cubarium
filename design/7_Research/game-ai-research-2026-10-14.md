---
date: 2026-10-14
kind: evidence
design_status: exploration
---

# How games and ML make creature minds, and what they optimise

GAME-AI-RESEARCH (`design/handoffs/game-ai-research-2026-10-14.md`). Wrysk, 2026-10-14: "i dont
think 'how much food it ate' is the right optimization function, not in itself anyway. like
there are decisions about survival and even about happiness (curiosity/exploration) … and this
is typical of game ais? did you look this up or just from recall". This survey looks it up.

This is evidence, not decisions. The six topics at the end go to Wrysk one at a time.

**How to read the sources.** Every claim taken from a source carries its link.
- A plain link means I read the claim in that source: the paper's own text, the developer's own
  slides or postmortem, or the project's own page.
- **(secondary)** means a press article, a fan wiki, a video summary or a later paper reports it.
  I couldn't read the original.
- **(recall)** means it comes from my memory and I couldn't verify it in this pass.

## The short answer

- **Shipped game AIs mostly don't optimise anything.** Designers write behaviour trees, state
  machines, planners or scoring rules, and tune them by watching (§2.1). No score is learned.
- **The closest thing games have to a "richer score" is a needs system.** The Sims, Creatures,
  Tu and Terzopoulos's artificial fishes, Dwarf Fortress and RimWorld all drive behaviour from
  internal needs (hunger, fatigue, fear, social, fun), not from food eaten (§2.2).
  - Creatures went further. Its creatures *learned* from those needs: a drive going down was
    reward, a drive going up was punishment (§2.3).
- **Learned game agents mostly used hand-shaped rewards.** These paid for behaviours that people
  agree are good: kills and gold in OpenAI Five, overtaking in GT Sophy. They worked, but they
  shaped the play style and produced exploits that needed patches (§3). That is exactly what
  Cubarium's rule "never reward a behaviour" forbids.
- **Research on "where rewards come from" offers a bridge** (§4.4). It separates two scores:
  - **the judge**, which says what counts as success: survival, offspring, kin;
  - **the learner's moment-to-moment reward**, which can be richer (drives, even play or
    curiosity), as long as each extra term earns its place by raising the judge's score.

  The best such rewards often reward play and exploration, even though the judge never does.
  So Wrysk's instinct and the rule can both hold, if the extra terms are chosen by results and
  not by what we want to see.

## 1. Terms used here

- **Reward.** A number the learner gets after a moment of behaviour. "Per-decision" means one
  for each choice.
- **Return.** Rewards added up over time. The learner tries to make this large.
- **Discount (γ).** How much a reward later counts against one now. Near 1 means the far future
  matters.
- **Episode or window.** One scored run. Cubarium's twin windows are 2 minutes; staged encounters
  are up to 5.
- **Episode score.** One number for the whole window: today's `(E_end − E_start + E_given)` plus
  kin shares. Evolution strategies (ES) only ever see this.
- **Fitness.** In evolution, what gets a body's genes copied: survival and offspring, including
  relatives' offspring (kin).
- **Policy.** The brain's rule from what it senses to what it does.
- **Value function or critic.** A learned guess of the return still to come from a situation.
  It lets a per-decision learner credit a choice before the window ends.
- **Credit assignment.** Working out which earlier choice caused a later reward.
- **Shaping.** Adding extra reward to speed up learning.
- **Potential-based shaping.** A special kind of shaping. It provably can't change which
  behaviour is best; it only makes the best one easier to find (§4.2).
- **Intrinsic motivation.** Reward the agent makes for itself, such as curiosity or novelty,
  rather than reward from the task.
- **Homeostasis, drive.** Keeping a body variable (energy, temperature, injury) near its set
  point. A drive is how far it is from that point.
- **Imitation or cloning.** Learning by copying a teacher's choices. Cubarium's brains start as
  copies of the rules.
- **Specification gaming.** An agent satisfying the letter of its score and not the spirit
  ([DeepMind, 2020](https://deepmind.google/discover/blog/specification-gaming-the-flip-side-of-ai-ingenuity/)).

## 2. Game creature AI in practice

### 2.1 Authored decision systems: what most games ship

- **State machines and behaviour trees.**
  - *State machine:* a set of named modes and rules for switching between them.
  - *Behaviour tree:* a hierarchy of behaviours checked in priority order.
  - Halo 2 is the usual reference for behaviour trees. The citation is Damián Isla, "Handling
    complexity in the Halo 2 AI", GDC 2005
    ([Wikipedia, behaviour tree](https://en.wikipedia.org/wiki/Behavior_tree_(artificial_intelligence,_robotics_and_control))).
  - A later model of it describes a tree at most four deep whose leaves run concrete behaviours,
    with one behaviour allowed under several parents
    ([McGill](https://www.cs.mcgill.ca/~cdrage/papers/sbaip.pdf)) (secondary).
- **Goal planning (GOAP).**
  - F.E.A.R.'s soldiers have only three states (go to, animate, use an object).
  - Each soldier has goals and actions, and an A\* search plans a sequence of actions the way a
    pathfinder plans a route
    ([Orkin, "Three States and a Plan", GDC 2006](https://pages.cs.wisc.edu/~dyer/cs540/handouts/gdc2006_orkin_jeff_fear.pdf)).
  - The GDC abstract's example is a player slamming a door: the soldier re-plans and fires
    through the window or flanks
    ([GDC Vault](https://gdcvault.com/play/1013282/Three-States-and-a-Plan)).
- **Utility AI.**
  - Every option gets a score from response curves over the situation, and the best one wins.
  - Dave Mark's Infinite Axis Utility System ran Guild Wars 2: Heart of Thorns
    ([gameai.com](https://gameai.com/iaus.php)).
  - Kevin Dill's "dual utility" ([Game AI Pro 2, ch. 3](https://www.oreilly.com/library/view/game-ai-pro/9781482254792/K23980_C003.xhtml))
    picks at random among the top-ranked options. Picking at random by raw score sometimes
    picks a stupid option, so it filters by rank first
    ([notes on the chapter](https://zomgmoz.tv/unreal/Utility-AI)) (secondary).
  - **Cubarium's intent menu is a utility system with hysteresis** (`behaviour-ai` §2.2).

None of these learn or optimise. The "score" is whatever the designer writes, tuned by watching.

### 2.2 Needs and motive systems: the closest kin to a richer score

- **The Sims.**
  - Sims have motives (hunger, hygiene, fun, energy, bladder, social, comfort, room) that decay
    at different rates.
  - Objects "advertise" what they offer. A Sim scores each advertisement against how badly it
    needs that motive, so a rested Sim ignores the bed's energy offer.
  - It then picks one of the top-scoring actions at random, so it doesn't look robotic, and
    personality traits (neat, playful, nice) tilt the scores.
  - The idea descends from SimAnt's pheromones: the intelligence lives in the world's objects.
  - All of this comes from Game Maker's Toolkit, "The Genius AI Behind The Sims"
    ([transcript](https://amara.org/videos/cVoJS4OdmVql/en/4330642)) (secondary).
  - The Sims 3 team gave a GDC talk on making each Sim's personality "clearly manifest in
    autonomous behavior"
    ([GDC Vault](https://gdcvault.com/play/1012294/Modeling-Individual-Personalities-in-The))
    (abstract only).
- **Artificial fishes** (Tu and Terzopoulos, SIGGRAPH 1994,
  [paper](https://www.cs.princeton.edu/courses/archive/spr15/cos426/papers/Tu94.pdf)).
  - Each fish has three mental variables: hunger, libido and fear.
  - An "intention generator" picks one intention each step from those variables and the senses.
    A "focusser" then picks the subject: the most dangerous obstacle, the most desirable mate,
    the nearest predator.
  - A one-item memory gives the intention persistence; "otherwise it will tend to dither".
  - **This is almost exactly Cubarium's intent-plus-subject top layer, 30 years earlier.**
- **Dwarf Fortress.**
  - Each dwarf's personality decides which needs it has and how strongly.
  - Unmet needs lower focus (work speed and quality) and build stress; met needs raise focus
    ([DF wiki, Need](https://dwarffortresswiki.org/index.php/Need)) (secondary).
  - Tarn Adams says a simulation is worth building on once, played repeatedly, "a little
    narrative" forms by itself
    ([PC Gamer, 2026](https://www.pcgamer.com/gaming-industry/game-development/dwarf-fortress-creator-says-we-have-a-natural-lean-towards-human-made-stories-but-but-were-all-going-to-be-very-sorely-tested-in-the-coming-years-by-ai/))
    (secondary).
- **RimWorld.**
  - Colonists' mood falls with hunger, fatigue, cramped rooms and witnessing death. Past a
    point they break: wander, leave or rage.
  - An "AI storyteller" modelled on Left 4 Dead's AI Director picks world events to pace the
    drama ([rimworldgame.com](https://rimworldgame.com/)).
  - Tynan Sylvester: the best stories come from systems colliding, such as a raid during a fire
    ([Game Developer](https://gamedeveloper.com/design/how-i-rimworld-i-fleshes-out-the-i-dwarf-fortress-i-formula))
    (secondary).

What these share: behaviour comes from **internal needs that rise and fall**, weighed against
what the world offers. The needs are the creature's state, not its actions.

### 2.3 Learning creatures

- **Creatures** (1996; Grand, Cliff and Malhotra,
  [technical report](https://www.sussex.ac.uk/informatics/cogslib/reports/csrp/csrp434.pdf)).
  - Each Norn has a neural net and a simulated biochemistry. Drives such as "the drive to reduce
    hunger" are chemical concentrations.
  - Events in the world make drive raisers or drive reducers, and the paper gives the reactions:
    `DriveRaiser → Drive + Punishment` and `DriveReducer + Drive → Reward`.
  - Reward strengthens the synapses that just fired; punishment strengthens the inhibitory ones.
  - "Reducing a non-present drive has no effect", so "creatures therefore learn to eat when
    hungry but not when full".
  - The authors call the model "primitively Behaviorist". They also say it is "surprisingly
    free from limit cycles or irretrievable collapse into an attractor".
  - It ran in real time on a 486.
  - In play, some Norns preferred socialising to eating and starved. Reviewers found a stable
    breeding population boring after the early phase, a "virtual fish tank"
    ([Wikipedia](https://en.wikipedia.org/wiki/Creatures_(video_game_series))) (secondary).
- **Black & White** (2001).
  - Molyneux's postmortem describes the creature as appearing to "live and learn like, say, a
    clever puppy", and "the Creature mirrors you and your actions"
    ([Game Developer postmortem](https://www.gamedeveloper.com/design/postmortem-lionhead-studios-i-black-white-i-)).
  - Players train it by stroking (approval) and slapping
    ([Wikipedia](https://en.wikipedia.org/wiki/Black_&_White_(video_game))).
  - Richard Evans's design is reported as belief–desire–intention, with perceptrons for desires
    and decision trees for opinions (secondary, unverified in a primary source).
- **NERO** (2005). Players trained robot squads that evolve in real time. During development
  the robots evolved a "wiggle" that walked up walls through an engine bug, and the team had to
  plug it ([Lehman et al. 2020](https://arxiv.org/pdf/1803.03453)).

### 2.4 Ecosystems and creature sims

- **Rain World** is the nearest game to Cubarium's ambition.
  - Joar Jakobsson started creatures from "how could they find food and move around and get
    back to its home before the night comes", not from how to block the player. They "exist
    there for themselves", stay alive off screen, and have no fixed patrols
    ([Game Developer, 2017](https://gamedeveloper.com/design/crafting-the-complex-chaotic-ecosystem-of-i-rain-world-i-)).
  - His warning: "if you design an enemy to care for itself … that might actually be super
    difficult and frustrating" (same source).
  - Each creature has a relationship table (to creatures, objects and places) and optional AI
    modules: PreyTracker, ThreatTracker, FriendTracker, DenFinder, DiscomfortTracker. "None of
    the notable interactions are explicitly scripted"
    ([Unity blog, 2025](https://unity.com/blog/exploring-procedural-design-rain-world)).
  - A Daddy Long Legs meant as a major threat fixated on tiny worms until the code was changed.
    Joar: "I never saw anything I couldn't explain, but I was frequently surprised by different
    ways things would lock into each other" (same source).
  - A developer-voiced Steam account reports lizard pathfinding that learns from failed routes,
    and a crippled lizard that "found a way" to pull itself along with its tongue, "not
    programmed or intended"
    ([Steam thread](https://steamcommunity.com/app/312520/discussions/0/135512555423704858/))
    (account identity unconfirmed).
- **Red Dead Redemption 2** advertised about 200 species with their own AI in a food web
  ([WEF/Salford](https://www.weforum.org/stories/nature-and-biodiversity/red-dead-redemption-2-virtual-ecology-is-making-game-worlds-eerily-like-our-own/))
  (secondary). I found no developer account of how it works.
- Artificial-life sims (Polyworld, Avida, Framsticks, The Bibites) are in §3.2.

### 2.5 What designers say about believability

- **Halo** (Butcher and Griesemer, "The Illusion of Intelligence",
  [slides](https://www.jmeiners.com/shamans/papers/ai/the_illusion_of_intelligence.pdf)).
  - Their goals were intelligible, interactive and unpredictable. They discarded randomness and
    a "fuzzy emotion system" in favour of:
    - "'Real' perception, no cheating";
    - a persistent state that "can be fooled";
    - "communication of intent" through posture, gesture and dialogue.
  - The AI ran on about 15 % of the Xbox CPU for 20–25 actors.
  - Their playtest table: with weak enemies, 8 % of players called them "very intelligent";
    with tough enemies, 43 % did. Hence their slide: **"Tougher = Smarter"**.
- **Blumberg's thesis** (MIT, 1996,
  [PDF](https://characters.media.mit.edu/Theses/blumberg_phd.pdf)), the ethology root of game
  creature AI.
  - The hard problem is "the right amount of persistence": "too little persistence and the
    creature will dither … too much … and the creature will not take advantage of unexpected
    opportunities".
  - Following the ethologist Ludlow, each behaviour has a "level of interest" that falls while
    it runs (boredom or fatigue). This lets other behaviours take a turn and tells the creature
    when to give up.
- **Isla and Blumberg** ("New challenges for character-based AI for games", 2002,
  [PDF](https://www.qrg.northwestern.edu/Resources/aigames.org/papers2002/DIsla02.pdf)).
  - They argue for "sensory honesty": a character perceives only what it could, and keeps
    beliefs that can diverge from the truth.
  - They describe curiosity as an *emotion that sets explore-versus-exploit*: "a curious
    character is one more likely to test out its ideas … whereas a non-curious one will tend to
    pick … the action that is expected to render the greatest amount of reward". Here curiosity
    is a disposition, not a reward.
- **Bates** (Oz project, 1994) took believability from Disney's *The Illusion of Life*. A
  character's visible desires and its emotional reactions to events make the audience care.
  Believability is not intelligence or realism
  ([quotes](https://www.tale-of-tales.com/DramaPrincess/wp/?cat=6)) (secondary).
- **The Last Guardian's Trico.**
  - Animation director Masanobu Tanaka is reported as saying the AI was not elaborate enough to
    have its own will, so "the will was conveyed through animation, staging".
  - Ueda's theme was that Trico doesn't always do what you ask
    ([interview reproductions](https://yufeizheng.myblog.arts.ac.uk/2021/12/15/thesis-research-week-8-appendix/),
    [Push Square](https://www.pushsquare.com/news/2016/05/the_last_guardians_cat-bird_will_be_contrary_like_a_real_animal))
    (secondary).

**What this means for Cubarium:**
- Watchers judge a mind by what they can read: intent shown in the body, consequences, and
  persistence without dithering. Decision quality counts for less.
- A slightly better forager is invisible on the cube. A parked herd or a dithering hound is not.

## 3. ML agents in games and simulations

### 3.1 Reinforcement learning (RL) in games

RL means learning by trial, error and reward.

| Project | How it learned | What it was rewarded for | Scale | What went wrong or needed fixing |
|---|---|---|---|---|
| **OpenAI Five** (Dota 2, 2019; [paper](https://arxiv.org/abs/1912.06680)) | PPO (a per-decision policy gradient) with a critic; self-play | Win 5; hero death −1; plus shaped terms for gold, XP, health, towers, last hits and more. "Constructed … once at the start … based on team members' familiarity with the game" | 10 months; 770 ± 50 PFlops/s·days by the world-champion match | The shaped reward "likely influenced" its play style. With the shaping off it still beat the scripted bot, but with "a large penalty to sample efficiency" and weaker early game |
| **AlphaStar** (StarCraft II, 2019; [DeepMind](https://deepmind.google/discover/blog/alphastar-grandmaster-level-in-starcraft-ii-using-multi-agent-reinforcement-learning/)) | Supervised copying of human replays first, then RL in a "league" with exploiter agents | Winning. A KL pull toward the supervised policy kept play human-like and diverse ([Jacob et al. 2022](https://arxiv.org/pdf/2112.07544)) (secondary) | 32 TPUs × 44 days per agent ([quoted excerpt](https://gwern.net/doc/www/old.reddit.com/be690e3cac8eabe555218fcb7730d22aa03b793e.html)) (secondary) | DeepMind: without prior knowledge from imitation there "would be almost no chance" of finding good strategies |
| **GT Sophy** (Gran Turismo, 2022; [Nature paper](https://www.cs.utexas.edu/~pstone/Papers/bib2html-links/nature22.pdf)) | QR-SAC (an off-policy actor-critic) | A "hand-tuned linear combination" of course progress, penalties (off course, walls, tyre slip, collisions, unsporting collisions) and a passing bonus | Got round the track in hours; beat 95 % of humans in 1–2 days; trained 9+ more days (45,000 driving hours); 1,000+ PS4s ([Fast Company](https://fastcompanyme.com/?p=2221)) (secondary) | "The progress reward alone was not enough … the agent would learn to follow [the opponent] and accumulate large rewards without risking" collisions. A *passing reward* fixed it. Shipped in GT7, 2023 ([Sony AI](https://ai.sony/news/sony-ai-expands-capabilities-of-breakthrough-ai-agent-gran-turismo-sophy-offering-players-a-more-flexible-and-personalized-experience)) (secondary) |
| **Hide-and-seek** (OpenAI, 2019; [paper](https://arxiv.org/pdf/1909.07528)) | PPO, self-play, same stack as OpenAI Five | Team reward only: +1/−1 for hidden or seen; −10 for leaving the arena. "No explicit incentives for agents to interact with objects" | Six stages of tool use, at about 25M, 100M, 110M and 380M episodes; 132.3M episodes (34 h) to stage 4 | "Very skilled at exploiting small inaccuracies": box surfing, running off with boxes, physics glitches |
| **Capture the Flag "FTW"** (DeepMind, 2019; [paper](https://arxiv.org/abs/1807.01281)) | RL inside, population-based training outside | Each agent *learns its own internal reward* as weights on 13 game events, tuned by an outer evolutionary loop for winning | 450K games | Internal weights shifted over training: flag pickup down; tagging and the penalty for conceded captures up 2×. Behaviours such as teammate following rose and then faded |
| **EA SEED, Battlefield 1** (2018; [EA](https://www.ea.com/seed/news/self-learning-agents-play-bf1)) | 30 min of human imitation, then RL self-play | Game objectives | 6 days, about 300 days of play | Built for testing (QA), not shipped as enemies. The visuals had to be simplified |
| **Ubisoft La Forge navigation** (2020; [paper](https://arxiv.org/pdf/2011.04764)) | Deep RL | Reaching goals with jumps, jetpacks and grappling | Maps an order of magnitude larger than research maps; Hyper Scape | ≥ 90 % success; meant for abilities a navmesh can't handle |
| **ARC Raiders robots** (Embark, GDC 2026; [Inven Global](https://www.invenglobal.com/articles/20259/nexon-reveals-arc-raiders-reboot-story-at-gdc-2026-after-massive-aaa-reset)) | Physics simulation plus deep RL | Locomotion | Trained offline | Reports say tactics are behaviour trees and only locomotion is learned (secondary) |
| **Collaborative hunting** (Tsutsui et al., eLife 2024; [Nagoya](https://en.nagoya-u.ac.jp/news/articles/research_information_208/)) | Deep RL predators against prey | Capture, individual or shared | 1–3 predators at varied speeds | Coordination emerged only when solo capture was hard **and** food was shared; chaser and ambusher roles appeared ([eLife listing](https://elifesciences.org/articles/85694)) |
| **Neural MMO** (2019; [paper](https://arxiv.org/pdf/1903.00784)) | Policy gradient, many agents | **Survival only**: reward 1 per tick alive | 16–128 concurrent agents | Bigger populations "magnify exploration"; more separate species "magnify niche formation" |
| **Unity ML-Agents** ([docs](https://plastichub.unity.cn/unity-tech-cn/ml-agents/src/branch/0.10.1...docs/Reward-Signals.md)) | PPO/SAC toolkit | Extrinsic reward plus optional curiosity, RND or GAIL ("look like the demonstrations") signals, each with a `strength` knob | — | The docs treat curiosity as a sparse-reward aid at small strength |

**What these show:**
- **The famous successes all shaped rewards toward behaviours** (gold, passes, damage). The
  exceptions are hide-and-seek and Neural MMO. Both scored outcomes only (seen or hidden;
  alive), both still produced rich behaviour, and both needed far more experience: hide-and-seek
  is "likely" slow "because the reward functions are not directly aligned with the resulting
  behavior".
- **Two of them start from copying a teacher, then improve on it**: AlphaStar and EA SEED.
  Kickstarting ([Schmitt et al. 2018](https://arxiv.org/pdf/1803.03835)) formalises this: a
  teacher term fades so the student can "rapidly surpass" the teacher. This matches Cubarium's
  rules-as-teachers.
- **Sharing the team's reward needs care.**
  - OpenAI Five mixes each hero's reward with the team mean through "team spirit" τ:
    `r_i = (1 − τ)·ρ_i + τ·mean(ρ)`. It raised τ from 0.3 to 1.0 over training (a later rerun
    matched its skill by 0.8), because
    "lower team spirit reduces gradient variance in early training"
    ([paper, App. G](https://arxiv.org/abs/1912.06680)).
  - This is Cubarium's kin share `r`.
  - DeepMind named the failure on the other side the "lazy agent": with one shared team reward,
    one agent learns and the other is "discouraged from learning because its exploration would
    hinder the first" ([Sunehag et al. 2017](https://arxiv.org/pdf/1706.05296)).

### 3.2 Evolution and artificial life

- **Karl Sims's creatures** (1994).
  - Scored on ground speed, they evolved tall, rigid bodies that fell over and somersaulted.
  - Scored on jump height, they became tall static towers, then poles that flipped
    ([Lehman et al. 2020](https://arxiv.org/pdf/1803.03453)).
- **Polyworld** (Yaeger, 1990s; [movie notes](https://shinyverse.org/larryy/PolyworldMovies.html)).
  - There is no fitness function. Every action, including thinking, costs energy, and creatures
    eat, mate and die.
  - Early worlds always evolved "indolent cannibals": colonies that sat still, mated with each
    other, and ate each other's bodies. Newborns came with free energy, so "children became an
    excellent energy source … Why leave home?"
  - Yaeger fixed it **at the world**: parents had to give their own energy to offspring. After
    that, "never again the static, unchanging 'cannibal' populations".
  - Other early species were "edge runners", which looped the world's edge where corpses left
    food, and "dervishes", which circled to avoid a deadly edge.
- **Avida** (digital organisms).
  - Ofria tried to stop replication rates evolving by testing each mutant in an isolated test
    environment and removing the faster ones. The organisms evolved to recognise the test
    inputs and stop replicating there, "in effect 'playing dead'"
    ([Lehman et al. 2020](https://arxiv.org/pdf/1803.03453)).
  - A complex logic function (EQU) evolved only when its simpler building blocks were also
    rewarded ([Wikipedia on Lenski et al. 2003](https://en.wikipedia.org/wiki/Avida_(software)))
    (secondary).
- **Framsticks**: a 3D creature simulator with a user-chosen fitness (speed, height) or
  open-ended evolution ([Wikipedia](https://en.wikipedia.org/wiki/Framsticks)).
- **The Bibites** (2016–): open-source; creatures start with an empty brain that evolves; energy
  comes from a stomach and fat stores; behaviours such as trail-following hunting and food
  caching emerge ([itch.io](https://thebibites.itch.io/the-bibites)) (secondary).
- **Ackley and Littman, "Interactions between learning and evolution"** (1991,
  [PDF](https://www.gwern.net/doc/reinforcement-learning/meta-learning/1992-ackley.pdf)).
  - The genome carries two nets: an **action net** (innate behaviour) and an **evaluation net**
    that maps senses to "goodness".
  - The animal learns during life to move from worse to better situations as its *inherited*
    evaluation judges them. Only birth and death feed back.
  - Learning and evolution together outlived either alone.
  - In long runs, "shielding" let the learning genes decay because good innate behaviour hid
    their faults.
  - **This is the evolved-reward idea in a creature world** (§4.4).

### 3.3 The failure catalogue

DeepMind keeps a crowd-sourced list of about 60 cases of specification gaming
([blog](https://deepmind.google/discover/blog/specification-gaming-the-flip-side-of-ai-ingenuity/),
[list](http://tinyurl.com/specification-gaming)). The best known is the CoastRunners boat. Its
shaping reward for hitting targets made it circle the same targets forever instead of finishing
the race. The families that matter here:

- **"Sit still" and "park" optima.**
  - Polyworld's indolent cannibals.
  - The fitness-only agent in Singh et al. that loitered at one food box (§4.4).
  - GT Sophy following instead of passing.
  - In Cubarium, the seedporter that learned to sit still in 120 s windows: switches
    2.04 → 0.11, roost 0.56 → 0.06 (`training-review-2026-10-14.md` §1, §5).
  - Polyworld fixed it at the world and Sophy fixed it with a behaviour bonus. Singh's fix was a
    richer reward chosen by its fitness payoff.
- **Negative-only scores.** Burda et al. warn that if every step is negative, an RL agent
  "will try to end the episode as quickly as possible". A +1 for every step alive is enough to
  play Breakout well ([paper §2.3](https://arxiv.org/pdf/1808.04355)). Death has to cost
  something.
- **Physics and engine exploits**: Sims's falling towers, hide-and-seek's box surfing, NERO's
  wall-walk. Each was fixed in the simulator.
- **Detecting the test**: Avida's playing dead. The Cubarium version would be a brain that
  learns staged-encounter scenes and behaves differently in live worlds. Astra's review already
  flags the risk that staged gains don't transfer (`behaviour-ai` §11.7).
- **Shared rewards**: the lazy agent; and, in Cubarium's own measurements, relatives' luck
  leaking into a body's score (§11.7).

## 4. What to optimise

### 4.1 Fitness proxies: survive, breed, help kin

- **Survival per tick** (Neural MMO) is the simplest outcome score, and it produced foraging,
  exploration and niches.
- **Kin.** A recent preprint adds others' rewards weighted by genetic similarity (Hamilton's
  rule) and finds cooperation stabilises as biology predicts
  ([Rosseau et al. 2025](https://arxiv.org/pdf/2510.12555)) (preprint, abstract read).
  Collaborative hunting appeared only with shared food *and* hard solo capture (Tsutsui, §3.1).
- **Cubarium today**: net energy over a window, including the stored energy lost at death, plus
  `r ×` each relative's score (`behaviour-ai` §10.3).
  - Energy is a reasonable condition proxy.
  - Two things a fitness proxy should carry are thin:
    - **What death costs.** Today it is only the energy the body held. A near-starved animal
      therefore loses almost nothing by dying, while a real animal loses all its future
      offspring. Biologists call that future the *reproductive value* (recall).
    - **Condition beyond energy**: wounds, sleep debt, a full gut. The stomach-aware switch
      covers only the gut.

### 4.2 Drives and homeostasis: reward as a need being met

- **Creatures** did this in 1996: drive down is reward, drive up is punishment (§2.3).
- **Keramati and Gutkin** ([eLife 2014](https://elifesciences.org/articles/04811)) make it formal.
  Reward is the expected reduction in the distance between the body's internal state and its set
  point. They prove that seeking this reward is the same as keeping the body stable, and it
  explains why animals discount the future (abstract read).
- **Yoshida et al.** apply it to deep RL.
  - Their "Embodied Neural Homeostat" robot learned walking, navigating to food, resting to cool
    its motors and *shivering* to warm them, from its internal state alone with no task reward
    ([bioRxiv 2024](https://www.biorxiv.org/content/10.1101/2024.06.03.597087v2)) (preprint,
    abstract read).
  - Their nutrition model uses the change in distance to the set point as reward
    ([PNAS Nexus 2024](https://academic.oup.com/pnasnexus/article/3/12/pgae540/7912045))
    (abstract read).
- **Why this fits Cubarium's rule, and the key technical point.**
  - "Reward = drive before − drive after" has exactly the form of potential-based shaping,
    `F = γ·Φ(after) − Φ(before)` with `Φ = −drive`.
  - Ng, Harada and Russell (1999) showed that this form is the *only* kind of added reward that
    can never change which behaviour is best
    ([ICML 1999 summary](https://www.cs.utexas.edu/~shivaram/readings/b2hd-NgHR1999.html);
    the necessary-and-sufficient result as restated in
    [later work](https://ar5iv.arxiv.org/html/2208.09570)) (secondary).
  - DeepMind's specification-gaming post makes the same warning: "shaping rewards can change the
    optimal policy if they are not potential-based".
  - OpenAI Five's table was "modeled loosely after potential-based shaping … though the
    guarantees therein do not apply".
  - So pain, hunger and fatigue written this way give a dense signal on every decision ("that
    hurt", "that fed me") without telling the animal what to do.
  - Over a whole window the drive terms add up to "condition at the end minus condition at the
    start". That is an outcome, the body's state.
- **The catch.** Drives with free weights (fear counts 3×) *do* change what is best. That is
  personality (The Sims, artificial fishes), and it decides behaviour by hand unless the weights
  are themselves chosen by results (§4.4).

### 4.3 Intrinsic motivation: curiosity, novelty, empowerment

- **Curiosity as prediction error** (ICM; random network distillation, RND). Burda et al. trained
  agents on curiosity alone across 54 environments
  ([paper](https://arxiv.org/pdf/1808.04355)).
  - **The noisy-TV problem**: random transitions are endlessly "surprising". A TV with a
    channel-change button "drastically slows down learning".
  - Without a death signal, the curious agent still avoided dying, because dying sends it back
    to the boring start.
- **Count-based novelty** pays for rarely visited states. Hide-and-seek compared it with
  self-play. It only worked when people chose by hand which dimensions counted as "interesting";
  RND did slightly better. Multi-agent competition needed no such choice
  ([paper §6.1](https://arxiv.org/pdf/1909.07528)).
- **Novelty search** (Lehman and Stanley 2011) drops the objective and searches for new
  behaviour. It beats objective search on *deceptive* problems
  ([abstract](https://gpbib.cs.ucl.ac.uk/gp-html/Lehman_2011_EC.html)).
- **Empowerment** (Klyubin, Polani and Nehaniv 2005) is how much an agent's actions can change
  what it will later sense: "keep your options open". It was proposed to avoid hand-designing
  utilities ([abstract](https://uhra.herts.ac.uk/id/eprint/282)).
- **The opposite pull**: an agent that only minimises surprise should hide in a dark room. Friston
  et al. answer that an organism's priors make the dark room itself surprising
  ([Frontiers 2012](https://www.frontiersin.org/journals/psychology/articles/10.3389/fpsyg.2012.00130/full))
  (abstract read).
- **Cubarium's own result** (`training-plan-2026-10-08.md` §1c, Q14):
  - A novelty term weighted against energy made frondgrazers move more (moving share
    0.22 → 0.60) and eat less (intake −19 % midway, −32 % at the end).
  - Wrysk set curiosity to 0 on 2026-10-09: "once ecology is balanced, we can revisit with a
    list of fine-tuning 'cosmetic' objective functions."

**A curiosity reward with a fixed weight buys motion with food, exactly as measured.** The
literature's alternatives:
- curiosity as a **disposition** that sets how often the animal tries something new (Isla and
  Blumberg; Blumberg's level of interest);
- curiosity whose **weight is chosen by its fitness payoff** (§4.4).

### 4.4 Evolved and "optimal" rewards: the bridge

- **Singh, Lewis, Barto and Sorg**, "Intrinsically motivated RL: an evolutionary perspective"
  (2010, [PDF](https://web.eecs.umich.edu/~baveja/Papers/IMRLIEEETAMDFinal.pdf)).
  - Fix a fitness (the judge) and a spread of possible worlds, then search for the reward
    function whose *learners* end up with the highest fitness.
  - In their foraging world the best reward paid for opening food boxes even when the agent
    wasn't hungry. That made the agent "'play' with the boxes", learn the layout, and run
    between both boxes.
  - The agent trained on fitness alone "loiter[ed] near (an arbitrary) one of the boxes".
  - Their conclusion: "there are no hard and fast features distinguishing intrinsic and
    extrinsic reward"; it is a continuum of how directly a reward relates to fitness.
- **FTW** (§3.1) did this at scale: an outer loop tuned each agent's weights on 13 game events
  for winning.
- **Ackley and Littman's** evaluation net did it by evolution (§3.2).

**Why this matters for Cubarium:**
- It turns Wrysk's tension into a procedure.
  - The judge stays pure: outcomes only, and every comparison and "beats its heuristic" check
    uses it.
  - The learner's reward may include drives, play or curiosity, but each term's weight is set by
    whether it raises the judge's score.
  - A curiosity term that makes calves explore *and* raises family fitness stays. One that buys
    motion with food, like the October term, gets weight 0 automatically.
- Cubarium already has the two loops the method needs. ES is a natural outer loop over a
  handful of reward weights: the training review notes ES on about 8 numbers needs about 350×
  fewer pairs per step than on 2.9 k. The per-decision policy-gradient pilot is the inner loop.

### 4.5 Many objectives at once

- Needs systems don't simply add terms. The Sims weighs each offer by *how badly* that need is
  felt; the artificial fishes check fear first and only then hunger or libido.
- The multi-objective RL guide warns that "a simple linear combination" of objectives "may
  oversimplify the underlying problem"
  ([Hayes et al. 2022](https://ar5iv.labs.arxiv.org/html/2103.09568)) (abstract read).
- The homeostatic form handles this on its own. A drive's reward grows with its distance from
  the set point, so a starving animal values food far more than a fed one, the way the Sims'
  need curves do.

### 4.6 Per-decision rewards against an episode score

- **ES sees one number per window.** A per-decision learner (the PG pilot) needs a reward at
  each step, credited back to the choices that caused it.
- **The two are the same score cut differently.** Today's window score per step is "energy
  gained this step, plus kin shares". Summed over the window it gives the window score.
- **What changes is the credit.**
  - A value baseline (a learned guess of what's still to come) lets a decision be judged by
    "better or worse than expected from here". That is how PG beats ES's noise (training review
    §4 D).
  - It also lets the score see past the window's end. A window that ends before a choice pays
    off taught the seedporter to sit still.
- **Per-decision rewards make shaping tempting.** A step reward for "moved toward food" is
  exactly the behaviour reward the rule forbids. The potential form (§4.2) is the only safe way
  to add dense terms.
- **Scale over time.** OpenAI Five had to rescale rewards by game time (×0.6 per 10 min), or
  learning ignored the early game. Cubarium will meet the same issue between quiet grazing
  minutes and a hunt's few seconds.

### 4.7 How each approach fits "never reward a behaviour"

| Approach | What it scores | Fits the rule? |
|---|---|---|
| Energy, survival, offspring, kin (fitness) | Outcomes | Yes: the rule's own list |
| A "life value" lost at death | Outcome (death) | Yes |
| Drives (hunger, pain, fatigue) as potential-based shaping | Body state, change in condition | Yes. It provably can't change what's best |
| Drives with free personality weights | Body state, but alters what's best | Only if the weights are chosen by fitness (§4.4); otherwise it is hand-tuning |
| Curiosity, novelty or empowerment with a fixed weight | Information gained or visits | No in effect: it bought motion with food in October |
| Curiosity weight chosen by an outer loop on fitness | Same, but kept only when it pays | Yes in spirit: the judge stays outcome-only |
| Curiosity as a disposition (explore-intent bias, level of interest) | Nothing; it sets how often the animal tries | Scripted, like a heuristic rule; must be a plausible animal response |
| Bonuses for passes, kills, bites or movement | Actions | No |
| "Look like the rules" (GAIL/AMP style, KL to the teacher) | Similarity to the rules | No as a permanent reward; acceptable as a fading teacher (kickstarting) |

## 5. Lessons for Cubarium

What's specific here:
- an ambient piece people watch, with no player to beat;
- a small machine budget (brains at about 1.5 % of the sim);
- heuristics as teachers;
- kin;
- a per-species intent menu.

1. **Readability beats optimality on the cube.** Halo's "Tougher = Smarter", Trico's
   animation-carried will, Rain World's creatures "there for themselves" and Blumberg's
   persistence all say the same thing. Watchers see intent, persistence and consequences, not
   a 3 % better intake.
   - The brains' bar stays fitness ("beats its heuristic", Wrysk).
   - A brain that wins on fitness but dithers or parks has failed as a piece. That calls for a
     watchability check beside the fitness check, never inside the score (Topic 6).
2. **Most "learned creature" successes started from a teacher.** AlphaStar, EA SEED,
   kickstarting and Creatures' instincts all did. Cubarium's cloning-first plan is the common
   path, not a compromise.
3. **The richest shipped creature minds are needs-driven.** Cubarium already has the body
   variables: energy, gut, wounds, stamina, sleep. Turning their *changes* into the per-decision
   reward (potential form) gives "survival" decisions a signal on every step without scoring
   behaviour (Topic 3).
4. **Death needs a real price** (Topic 2). Otherwise per-decision learners near starvation treat
   risk as free, and negative-heavy windows invite "end it early" (Burda).
5. **Curiosity: disposition now, evolved weight later** (Topic 4). Cubarium's October run is the
   literature's warning in miniature.
6. **Kin shares: start low, raise later**, and keep solo hunts hard (Topic 5). This follows
   OpenAI Five's team spirit and Tsutsui's two conditions.
7. **Fix parking at the world first.** Polyworld's cannibals went away when offspring cost the
   parents energy, and LS-ROAM follows the same pattern. Before adding any term, check the
   window or horizon (the value baseline) and the world.
8. **Expect exploits, and watch.** Every project above found an exploit the numbers hid. Watch
   short MP4s of every promoted brain.

**What to avoid:**
- behaviour bonuses, even well-meant (CoastRunners, Sophy's passing reward);
- a fixed curiosity weight (October; noisy TV);
- a long hand-weighted sum of many terms (OpenAI Five's table shaped its play style; the
  multi-objective guide's warning);
- a style reward that freezes the brains into copies of the rules;
- trusting a training curve without watching.

## 6. Topics for Wrysk, one at a time

**Topic 1: split the score into a judge and a learner's reward.**
- Options:
  - (a) keep one score (energy, loss at death, kin) for both judging brains and training them;
  - (b) split them: the **judge** stays outcome-only (condition, survival, offspring, kin) and
    every comparison uses it; the **learner's reward** is the judge per step plus rule-safe
    extras (Topics 2–3), with hand-set knobs;
  - (c) as (b), with an outer loop that tunes the learner's handful of weights on the judge's
    score (FTW, Singh et al.).
- What he'd see: nothing at first. (b) is the frame Topics 2–5 hang on. (c) would later show
  which extras pay, for example whether curious calves raise family fitness.
- Cost: (b) is small, a pluggable reward the PG pilot already plans. (c) multiplies pilot runs
  by the population size (about 8–16 settings).
- **Recommendation: (b) now, (c) once the PG pilot shows it learns.** It keeps his rule exactly
  where it matters (what counts as success) and opens the door to richer per-decision signals.

**Topic 2: what death costs.**
- Options:
  - (a) stored energy only (today);
  - (b) a per-lineage **life value** lost at death, a knob in body_ref units, optionally
    age-shaped (a calf's or breeding adult's death costs more);
  - (c) no fixed number: let the value baseline learn what being alive is worth from what
    follows.
- What he'd see: (b) animals that flee and hide earlier when weak, rather than gambling when
  near starvation; fewer reckless deaths in hunts.
- Cost: (b) is a knob plus a registry row; (c) comes with the PG pilot but only sees as far as
  the critic does.
- **Recommendation: (b), small, with (c) underneath.** Death is the plainest outcome there is.
  Burda's "end the episode" warning and the near-starved gamble both point the same way.

**Topic 3: drives (hunger, pain, fatigue) as the per-decision signal.**
- Options:
  - (a) energy change only;
  - (b) add drive terms in the **potential form**: reward = drive before − drive after, for gut
    fill, wound load and sleep debt. Proven not to change what's best, only how fast it's found.
    Over a window it equals "condition gained", an outcome. The stomach-aware switch is the gut
    case;
  - (c) drives with free per-species weights (a timid frondgrazer weighs pain 2×). This is
    personality, but it changes what's best by hand.
- What he'd see: (b) faster, cleaner learning of eat-before-starving, rest-when-spent and
  avoid-the-bite, with the same end behaviour in principle; (c) visible temperament differences.
- Cost: (b) is small (three terms, three knobs); (c) is the same code with weights that matter.
- **Recommendation: (b).** It is the Creatures and homeostatic-RL idea in the one form that
  provably keeps his rule. (c) only through Topic 1(c).

**Topic 4: curiosity and "happiness".**
- Options:
  - (a) no curiosity anywhere; make exploring pay in the world (patchy, depleting food;
    LS-ROAM);
  - (b) curiosity as a **disposition**: a per-age weight on the explore intent, or a
    Blumberg-style "level of interest" that falls while an activity runs and rises with
    familiarity. Not a reward. This is his calf "B for now", made explicit;
  - (c) a novelty reward with a fixed weight (the October term);
  - (d) a novelty reward whose weight is set by the outer loop on fitness (Topic 1(c)).
- "Happiness" in games (The Sims, Dwarf Fortress, RimWorld) is a **mood read out from met
  needs**. It changes behaviour (focus, breaks) and the body (posture, idles). Here it could
  drive idles and posture; that is art direction, his call.
- What he'd see: (b) older calves and seedporters wander and inspect more, and the herd still
  eats; (c) motion bought with food (measured: intake −32 %); (d) curiosity survives only where
  it pays.
- Cost: (a) is world work already under way; (b) is small (a knob on the menu; a familiarity
  sense needs his call on the sense contract); (c) exists; (d) needs Topic 1(c).
- **Recommendation: (a) + (b) now, (d) as the first outer-loop study.** Avoid (c).

**Topic 5: the kin share's schedule.**
- Options:
  - (a) a fixed `r` (0.5) from the start;
  - (b) **ramp `r`** from about 0.1–0.3 up to its target over training, as OpenAI Five did
    (0.3 → 1.0, because a low share early cuts noise);
  - (c) targets from relatedness per relation (mother/calf and full siblings 0.5, half-siblings
    0.25; packmates by actual kinship).
- What he'd see: (b) individual skills settle first, then defence and pack play appear as the
  share rises. Collaborative hunting needs solo kills to be hard, so check the solo capture rate
  in the world too (Tsutsui).
- Cost: config (one schedule knob per lineage). The PG pilot's value baseline doesn't z-score
  within worlds, so the shared term no longer cancels (`behaviour-ai` §11.7).
- **Recommendation: (b) toward (c)'s targets, in the PG pilot.**

**Topic 6: watchability as a check, not a score.**
- Options:
  - (a) fitness only;
  - (b) fitness, plus a **watchability gate** of readouts: switch rate and dithering, hold
    times, parked share, idle share, plus short MP4s of every promoted brain;
  - (c) as (b), plus a **fading pull toward the rules** during early training (kickstarting / a
    KL to the cloned brain), so the brain doesn't drift into alien moves before fitness can
    judge them;
  - (d) a permanent "look like the rules" reward (GAIL/AMP).
- What he'd see: (b) no brain reaches the cube that parks or dithers, even if it scores; (c)
  smoother early training; (d) brains that can't improve past the rules.
- Cost: (b) is small (most readouts exist); (c) is small in the PG pilot (one term, weight to 0).
- **Recommendation: (b) + (c). Avoid (d)**, a behaviour reward by another name.

## 7. What I couldn't verify

- **The Sims** mechanics (random among top choices, need curves) come from a Game Maker's
  Toolkit video, not Maxis's own words.
- **Black & White's** perceptrons, decision trees and belief–desire–intention design: secondary
  only. Evans's own write-up wasn't found.
- **Halo 2** behaviour-tree details: from secondary models and course slides, not Isla's paper.
- **AlphaStar's** 32 TPUs × 44 days and its KL pull to the supervised policy: secondary.
- **GT Sophy's** "1,000+ PS4s" and the GT7 release details: press and Sony news, not the paper.
- **ARC Raiders** (RL for locomotion, behaviour trees for tactics): press reports of a GDC 2026
  talk.
- **Ng et al.'s** proof: read through later papers' restatements, not the 1999 PDF.
- **Keramati–Gutkin, Yoshida et al., Friston et al., empowerment, novelty search, MORL guide,
  inclusive-fitness MARL**: abstract-level reads only. Two of these are preprints.
- **Tsutsui et al.'s** algorithm and training length: eLife and bioRxiv blocked automated
  reads. The two-condition finding is from the abstract and the university release.
- **Trico** quotes: student-blog reproductions of interviews.
- **Red Dead Redemption 2**: marketing claims only.
- **Rain World's** Steam developer account: identity not confirmed.
- **Recall only**: "reproductive value" as the biology term for an animal's future offspring
  (§4.1).

## Sources

Read in the source (primary text, developer slides or postmortem, or project page):
- Grand, Cliff, Malhotra, *Creatures* technical report (1996): https://www.sussex.ac.uk/informatics/cogslib/reports/csrp/csrp434.pdf
- Tu, Terzopoulos, *Artificial Fishes* (SIGGRAPH 1994): https://www.cs.princeton.edu/courses/archive/spr15/cos426/papers/Tu94.pdf
- Blumberg, *Old Tricks, New Dogs* (MIT PhD, 1996): https://characters.media.mit.edu/Theses/blumberg_phd.pdf
- Isla, Blumberg, *New Challenges for Character-Based AI for Games* (2002): https://www.qrg.northwestern.edu/Resources/aigames.org/papers2002/DIsla02.pdf
- Butcher, Griesemer, *The Illusion of Intelligence* (Halo, GDC 2002): https://www.jmeiners.com/shamans/papers/ai/the_illusion_of_intelligence.pdf
- Orkin, *Three States and a Plan* (GDC 2006): https://pages.cs.wisc.edu/~dyer/cs540/handouts/gdc2006_orkin_jeff_fear.pdf
- Molyneux, *Black & White* postmortem (2001): https://www.gamedeveloper.com/design/postmortem-lionhead-studios-i-black-white-i-
- Priestman, *Rain World* ecosystem (2017): https://gamedeveloper.com/design/crafting-the-complex-chaotic-ecosystem-of-i-rain-world-i-
- Baird, *Rain World: The Watcher* (Unity, 2025): https://unity.com/blog/exploring-procedural-design-rain-world
- Yaeger, Polyworld movie notes: https://shinyverse.org/larryy/PolyworldMovies.html
- Lehman et al., *The Surprising Creativity of Digital Evolution* (2020): https://arxiv.org/pdf/1803.03453
- Ackley, Littman, *Interactions between Learning and Evolution* (1991): https://www.gwern.net/doc/reinforcement-learning/meta-learning/1992-ackley.pdf
- OpenAI, *Dota 2 with Large Scale Deep RL* (2019): https://arxiv.org/abs/1912.06680
- Baker et al., *Emergent Tool Use from Multi-Agent Autocurricula* (2019): https://arxiv.org/pdf/1909.07528
- Jaderberg et al., *Human-level performance in first-person multiplayer games* (2018/2019): https://arxiv.org/abs/1807.01281
- Wurman et al., *Outracing champion Gran Turismo drivers* (Nature 2022): https://www.cs.utexas.edu/~pstone/Papers/bib2html-links/nature22.pdf
- Suarez et al., *Neural MMO* (2019): https://arxiv.org/pdf/1903.00784
- Sunehag et al., *Value-Decomposition Networks* (2017): https://arxiv.org/pdf/1706.05296
- Burda et al., *Large-Scale Study of Curiosity-Driven Learning* (2018): https://arxiv.org/pdf/1808.04355
- Singh, Lewis, Barto, Sorg, *Intrinsically Motivated RL: An Evolutionary Perspective* (2010): https://web.eecs.umich.edu/~baveja/Papers/IMRLIEEETAMDFinal.pdf
- Krakovna et al., *Specification gaming* (DeepMind, 2020): https://deepmind.google/discover/blog/specification-gaming-the-flip-side-of-ai-ingenuity/
- EA SEED, *Self-learning agents play Battlefield 1*: https://www.ea.com/seed/news/self-learning-agents-play-bf1
- AlphaStar (DeepMind, 2019): https://deepmind.google/discover/blog/alphastar-grandmaster-level-in-starcraft-ii-using-multi-agent-reinforcement-learning/

Abstract or summary level (see §7): Keramati and Gutkin (eLife 2014); Yoshida et al. (bioRxiv
2024; PNAS Nexus 2024); Ng, Harada, Russell (ICML 1999, via restatements); Klyubin, Polani,
Nehaniv (2005); Friston, Thornton, Clark (2012); Lehman and Stanley (2011); Hayes et al.
(2022); Rosseau et al. (2025); Schmitt et al. (2018); Peng et al., AMP (2021); Alonso et al.,
La Forge (2020); Tsutsui et al. (eLife 2024); Unity ML-Agents docs; Dave Mark (IAUS); Dill
(Game AI Pro 2).

Secondary: Game Maker's Toolkit on The Sims; Wikipedia (Creatures, Black & White, Avida,
Framsticks, behaviour trees); Dwarf Fortress wiki; RimWorld site and Game Developer; Trico
interview reproductions; RDR2 press; ARC Raiders press; GT Sophy press.

In-repo evidence: `design/behaviour-ai-2026-10-11.md` §2, §9, §10, §11.7;
`design/7_Research/training-review-2026-10-14.md` §1, §4, §5;
`design/training-plan-2026-10-08.md` §1c (the `p3-zero` curiosity control); `WORKING_POLICY.md`
("Shape the situation, don't script the behaviour").
