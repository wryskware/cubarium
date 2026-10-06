---
design_status: exploration
last_reviewed: 2026-10-06
decision_refs: []
---
# Astra: a senses pass for every animal (2026-10-06)

Astra (gpt-6-astra) answered `astra-senses-prompt-2026-10-06.md`. Evidence, not decisions. Wrysk's decisions are at the end of `senses-review-2026-10-06.md`.

**I recommend distinct sensory niches for all nine animals, built on one shared perception system.**
The Veilhound should find prey across open habitat; grazers should notice danger broadly; small decomposers should read their substrate; bellwings should follow scent and conspicuous light.
The critical distinction is between detecting a cue, identifying its source, and reaching it.
Those should remain separate ecological events.

This is a read-only review of `main` at `6c7ea7ea` and the uncommitted `veilhound-search` worktree observed during this review.
The branch was actively changing; its findings below describe the inspected source, not a finished implementation.
I wrote no files and ran no builds, tests, or ecological studies.
**Fact** below means inspected implementation; **recommendation** means my proposal; predictions are unmeasured.
Wrysk’s directions in this request take precedence over older proposals.

**The scale supports substantial sensing distances, but not universal awareness.**
The terrarium uses 0.125 m voxels: its configured dimensions correspond to 32 m around, 16 m vertically, and 6 m deep.
Its 120-minute day has a daylight fraction of 0.67—about 80 minutes daylight and 40 minutes night. [terrarium.toml:17](/home/wrysk/wryskware/cubarium/config/desktop/terrarium.toml:17)
A 16 m detection distance already covers half the circumference along an unobstructed horizontal sightline.
The reported 13 m route to prey is therefore a navigation distance, not a required visual range.

These are the adult simulation envelopes, in length × width × height; they are not detailed anatomical meshes:

| Animal | Metres | Voxels at this preset | Code fact |
|---|---:|---:|---|
| Littershredder | 0.375 × 0.125 × 0.125 | 3 × 1 × 1 | [body.rs:1047](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:1047) |
| Capgnawer | 0.25 × 0.21 × 0.19 | 2 × 1.68 × 1.52 | [body.rs:1279](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:1279) |
| Ripple-snail | 0.19 × 0.19 × 0.063 | 1.52 × 1.52 × 0.50 | [body.rs:1551](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:1551) |
| Frondgrazer | 0.75 × 0.375 × 0.375 | 6 × 3 × 3 | [body.rs:1127](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:1127) |
| Loftstrider | 2.2 × 2.2 × 3.4 | 17.6 × 17.6 × 27.2 | [body.rs:1338](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:1338) |
| Bellwing | 0.125 × 0.125 × 0.0625 | 1 × 1 × 0.5 | [body.rs:1396](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:1396) |
| Seedporter | 0.5 × 0.25 × 0.2 | 4 × 2 × 1.6 | [body.rs:1447](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:1447) |
| Lanternjaw | 1 × 0.25 × 0.45 | 8 × 2 × 3.6 | [body.rs:1212](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:1212) |
| Veilhound | 1.4 × 0.35 × 0.6 | 11.2 × 2.8 × 4.8 | [body.rs:1514](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:1514) |

The manifest’s `anchors` obtains these dimensions and receptor fractions from `FounderPhysiology::frozen`. [manifest.rs:797](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/manifest.rs:797)
Growing bodies scale their geometry by the cube root of structural stock relative to adult stock. [body.rs:1656](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:1656)
Recommendations should use these physical anchors, including juvenile eye height, rather than older dossier dimensions.
The dossiers explicitly remain proposals and contain unreconciled earlier anatomy. [species-dossiers-2026-09-21.md:9](/home/wrysk/wryskware/cubarium/design/art-direction/species-dossiers-2026-09-21.md:9)

**The current problem is broader than a shared short cone, and some useful machinery already exists.**

- **Fact:** the browser has a 2 m, 45-ray cone; lanternjaw copies that layout, and Veilhound copies lanternjaw without its lure action. [manifest.rs:911](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/manifest.rs:911), [manifest.rs:965](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/manifest.rs:965), [manifest.rs:1218](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/manifest.rs:1218)
- **Fact:** bellwing has its own 4 m downward fan and bloom channel; seedporter inherits the browser range but substitutes fruit sensing. [manifest.rs:1132](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/manifest.rs:1132), [manifest.rs:1179](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/manifest.rs:1179)
- **Correction:** littershredder, capgnawer and ripple-snail already have distinct detritus, fungal and film chemical fields.
  They are not restricted to mouth taste; the detritus field includes litter and carrion. [senses.rs:269](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/senses.rs:269), [body.rs:3284](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:3284)
- **Correction:** tremor already drives an external heuristic flee path, although it is absent from the blind neural manifest.
  It sums other bodies’ mass × speed / distance² into left/right readings. [lib.rs:2934](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/lib.rs:2934), [step.rs:1338](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/step.rs:1338), [manifest.rs:539](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/manifest.rs:539)
- **Fact:** lanternjaw patrol separately searches with `hunt.sight_m` and tremor; it also scans for reachable carcasses.
  Thus “no remote carrion chemical observation” is accurate; “cannot find remote carrion” is not. [hunt.rs:846](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/hunt.rs:846), [hunt.rs:592](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/hunt.rs:592)
- **Fact:** main’s pack acquisition uses the manifest range and forward half-plane, followed by a route-reachability filter.
  That acquisition loop itself does not perform a visibility ray. [pack.rs:700](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/pack.rs:700)
- **Fact:** the observation cone reports generic bodies; the `Light` receptor has no glowcap emission contribution.
  Its implementation samples sky visibility, not a directional luminous-source signal. [body.rs:3319](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:3319)
- **Fact:** Brief P2 proposed classified bodies, glow, wound, daylight, blind tremor and carrion chemistry.
  Those ideas should be folded into this pass rather than implemented as a separate intermediate contract. [voxel-predation-2026-09-28.md:157](/home/wrysk/wryskware/cubarium/design/handoffs/voxel-predation-2026-09-28.md:157)

**The Veilhound branch is a useful foundation, with several limitations to address before generalising it.**
In the following citations, the path is the inspected `veilhound-search` worktree.

- **Fact:** `SenseTable` is serde-visible and holds sight range, sight half-angle, hearing range, movement threshold and smell range.
  Corresponding registry rows exist. [body.rs:4814](/home/wrysk/wryskware/cubarium/.claude/worktrees/veilhound-search/crates/cubarium-voxel-fauna/src/body.rs:4814), [registry.rs:235](/home/wrysk/wryskware/cubarium/.claude/worktrees/veilhound-search/crates/cubarium-voxel-fauna/src/registry.rs:235)
- **Fact:** Veilhound values are 16 m sight, 120° half-angle, 10 m hearing, 0.05 m/s hearing threshold and 6 m smell.
  These are plausible starting knobs, not measurements derived from wolves. [body.rs:4856](/home/wrysk/wryskware/cubarium/.claude/worktrees/veilhound-search/crates/cubarium-voxel-fauna/src/body.rs:4856)
- **Fact:** sight currently ignores foliage and other bodies; hearing and smell use distance without terrain attenuation.
  Smell succeeds first, then hearing, then sight, returning only one modality. [perception.rs:20](/home/wrysk/wryskware/cubarium/.claude/worktrees/veilhound-search/crates/cubarium-voxel-fauna/src/perception.rs:20), [perception.rs:39](/home/wrysk/wryskware/cubarium/.claude/worktrees/veilhound-search/crates/cubarium-voxel-fauna/src/perception.rs:39)
- **Fact:** the pack pools awake adults’ detections once per tick, retaining target IDs.
  The approach behaviour can then route toward the current coordinates of detected prey. [pack.rs:424](/home/wrysk/wryskware/cubarium/.claude/worktrees/veilhound-search/crates/cubarium-voxel-fauna/src/pack.rs:424), [pack.rs:501](/home/wrysk/wryskware/cubarium/.claude/worktrees/veilhound-search/crates/cubarium-voxel-fauna/src/pack.rs:501)
- **Recommendation:** retain the lineage table, cached observations, and separation between approaching distant evidence and committing to a chase.
  Add emission strength, terrain transmission, illumination, target conspicuity, confidence and observation age.
- **Recommendation:** use actual three-dimensional receptor-to-target distance for range.
  The inspected branch gates on horizontal `d` before computing the three-dimensional sightline. [perception.rs:54](/home/wrysk/wryskware/cubarium/.claude/worktrees/veilhound-search/crates/cubarium-voxel-fauna/src/perception.rs:54)
- **Recommendation:** report all contributing modalities, not whichever was cheapest to evaluate.
  Otherwise the nearest visible prey will be logged as “smelled,” making sensory ablations hard to interpret.
- **Recommendation:** a smell or rustle should initially identify a direction or patch, not an exact moving individual.
  Pack sharing should require a nearby receiver or an audible call, with retained information becoming stale.
- **Recommendation:** replace the branch test that pins other lineages to 90°, no hearing and no smell.
  That test would fail the intended senses pass even when its mechanisms work correctly. [pack_search_tests.rs:331](/home/wrysk/wryskware/cubarium/.claude/worktrees/veilhound-search/crates/cubarium-voxel-fauna/src/pack_search_tests.rs:331)

**All ranges below are recommended starting values at terrarium scale.**
They describe useful detection of a representative source in favourable conditions, not guaranteed identification.
A small, still bellwing should be harder to resolve than a walking loftstrider at the same distance.
All angles are full horizontal fields of view; hearing and chemistry are omnidirectional unless stated.
“Night range” assumes residual ambient illumination; an unlit non-emissive target should not remain visible in complete darkness.
Every distance, sensitivity, response threshold, memory time, emission strength and attraction weight is a configurable ecological knob.
Different presets should use the same ecological values in metres, with voxel conversion occurring only at geometry queries.

**For the littershredder, I recommend a substrate reader with almost no distant world model.**
Its selected body is explicitly eyeless, with downward feelers and an elongated plated crawler form. [species-dossiers-2026-09-21.md:335](/home/wrysk/wryskware/cubarium/design/art-direction/species-dossiers-2026-09-21.md:335)

- **Sight:** none; no body silhouettes, colour discrimination or UV channel.
- **Light:** a nondirectional exposed-versus-sheltered reading; weak avoidance of strong illumination.
  This is photoreception, not an eye and not attraction to bright objects.
- **Chemical:** strong local litter/decomposition chemistry, useful over roughly 1–2 m of connected substrate.
  Use the existing field and temporal improvement during movement; retain mouth taste for whether food is actually present.
- **Hearing:** no airborne hearing in the first pass.
- **Touch/tremor:** contact feelers plus substrate vibration over about 1 m, extending toward 2 m for heavy footfalls.
  The signal means “disturbance here,” not “Veilhound of known identity.”
- **Conspecifics:** contact and weak local chemical crowding within roughly 0.5 m; no distant social attraction.
- **Behaviour:** continue feeding through weak background disturbance; freeze briefly or withdraw from strong approaching vibration.
  Constant flight from every passing grazer would interrupt the very decomposition role this animal supplies.
- **Ecological prediction:** better patch following should improve litter consumption and reduce starvation between neighbouring patches.
  Sensitive tremor should reduce some lanternjaw captures, while a stationary hunter remains difficult to detect.

**For the capgnawer, I recommend fungal chemistry first, with coarse directional light sensing.**
Its selected face has four sensor patches and a broad mantle suited to gripping bark; the dossier explicitly contemplates sensing fungal light. [animal-body-reimagining-2026-09-24.md:329](/home/wrysk/wryskware/cubarium/design/animal-body-reimagining-2026-09-24.md:329)

- **Sight:** coarse silhouettes at about 1 m by day and 0.5 m in dim conditions, across roughly 240°.
  Give motion more weight than static shape; no detailed species recognition at that distance.
- **Colour:** one blue/cyan-sensitive brightness channel is sufficient; UV adds little to its niche.
- **Chemical:** strong fungal-cap cue over 2–3 m along connected feeding surfaces.
  Weaker conditioned-litter chemistry can guide fallback feeding within about 1 m.
- **Glow attraction:** recognise a luminous fungal patch at approximately 2–3 m in darkness.
  Attraction should strengthen when fungal scent agrees, rather than treating every cyan light as edible.
- **Hearing/tremor:** no separate airborne hearing; useful substrate vibration over 1–1.5 m.
- **Touch:** short-range mandible and foot contact confirms caps, bark edges, obstacles and feeding position.
- **Conspecifics/threats:** local crowding chemistry around 0.5 m; coarse looming and vibration trigger withdrawal.
  Do not give the animal perfect predator classification through its mantle.
- **Ecological prediction:** capgnawers should connect neighbouring fungal patches and answer expanding glowcap stocks more reliably.
  Excessive light attraction could instead concentrate them at lanternjaw lures, indirectly releasing fungi from consumption.

**For the ripple-snail, I recommend water-connected chemistry and a strong local disturbance response.**
The selected animal has an open-front shell and feeding palps; the implementation confines it near standing water and gives it a film diet. [animal-body-reimagining-2026-09-24.md:411](/home/wrysk/wryskware/cubarium/design/animal-body-reimagining-2026-09-24.md:411), [body.rs:1555](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:1555)

- **Sight:** only broad shadow and looming detection, approximately 0.5 m by day and 0.25 m in dim light.
  Use roughly 180° around the exposed front, with no fine static shape or colour recognition.
- **UV/brightness:** no UV channel and no generic luminous-object attraction.
  Weak light preference may help find productive film, but wetness and actual film chemistry must dominate.
- **Chemical:** film-associated cue over 0.5–1 m along connected wet surfaces.
  Do not make film beneath a dry ridge detectable as if it were in the same water patch.
- **Hearing:** no airborne hearing.
- **Touch/tremor:** palps and foot contact; local water or substrate disturbance at about 0.5 m.
  A large approaching walker may be detectable at roughly 1 m.
- **Threat response:** retract or stop scraping before attempting relocation; a snail should not behave like a tiny running grazer.
- **Conspecifics:** mucus/contact chemistry within about 0.25–0.5 m can discourage repeatedly using depleted tracks.
- **Ecological prediction:** more coherent film grazing, short feeding interruptions, and continued vulnerability to lanternjaws at accessible margins.
  Overpowered vibration avoidance could leave wet ground ungrazed despite many living snails.

**For the frondgrazer, I recommend broad vigilance with moderate identification range.**
Its selected anatomy now includes two eye-like sensors on corner stalks, replacing earlier plain palp tips. [species-dossiers-2026-09-21.md:227](/home/wrysk/wryskware/cubarium/design/art-direction/species-dossiers-2026-09-21.md:227)

- **Sight:** moving large bodies at 8 m by day and 4 m at night; static bodies at about 5 m and 2 m.
  Use roughly 300° horizontal coverage, with a small rear blind region and reduced clarity while cropping.
- **Food sight:** foliage patches at 4–6 m; colour/contrast separates vegetation from bare substrate.
  Detailed food suitability is confirmed at the mouth; UV is unnecessary.
- **Hearing:** walking predators at about 6 m, approaching running bodies at 8 m.
  Conspecific alarm or escape movement should create an orienting cue, without instant herd-wide knowledge.
- **Smell:** nearby predator odour at about 3–4 m; foliage and conspecific chemistry at 1–2 m.
  Scent should prompt vigilance or relocation, not perpetual full-speed flight.
- **Touch/tremor:** foot contact and approximately 1 m of local disturbance sensing.
- **Brightness:** notice bright objects as landmarks; no feeding attraction to lanternjaw glow.
- **Behaviour:** alternate cropping with brief scans; orient to uncertain sounds, flee a confirmed approaching threat, then reassess.
  Recovery from alarm needs hysteresis so the animal neither chatters between actions nor runs indefinitely.
- **Ecological prediction:** Veilhounds should still spot exposed grazers first at long distance, but grazers should often initiate escape before striking range.
  Better vigilance reduces capture probability per encounter while increasing lost feeding time.

**For the loftstrider, I recommend panoramic awareness from its actual eye ring.**
Its current envelope is radial, with the eye anchor at 0.44 of 3.4 m—about 1.5 m above its support, not at the top. [body.rs:1338](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:1338)
The selected body description gives it six eyes, making panoramic coverage a stronger anatomical fit than a forward browser cone. [animal-body-reimagining-2026-09-24.md:220](/home/wrysk/wryskware/cubarium/design/animal-body-reimagining-2026-09-24.md:220)

- **Sight:** 12 m for moving large bodies by day, 6 m at night; static recognition around 8 m and 4 m.
  Give it 360° coverage with directional sectors and a separate upward feeding view.
- **Food sight:** foliage structure at 6–8 m, evaluated against its feeding height rather than ground greenery.
- **Colour/UV:** modest vegetation colour contrast; no UV requirement.
- **Hearing:** useful large-body movement at 8 m; louder running or calls at 10 m.
- **Smell:** vegetation and nearby animal presence at 2–3 m, secondary to sight and sound.
- **Touch/tremor:** substantial foot contact and about 2 m of useful ground disturbance.
  Its own steps should mask weak tremor, not make it an exceptionally precise vibration hunter.
- **Brightness:** recognise luminous objects without approaching them for food.
- **Behaviour:** react to a converging group more strongly than an isolated distant hunter; move toward usable space before encirclement.
  A head-height foliage view must not suppress lower-body threat awareness.
- **Ecological prediction:** fewer avoidable pack encounters and more displacement of canopy feeding.
  Panoramic sensing must not make every juvenile effectively invulnerable.

**For the bellwing, I recommend weak form vision, strong floral smell, and a separable brightness receptor.**
This follows Wrysk’s animal, rather than importing a hummingbird’s excellent eyesight.
Real hummingbirds can discriminate colours involving UV; their visual ability does not justify calling poor vision “hummingbird-like.” [Stoddard et al., 2020](https://doi.org/10.1073/pnas.1919377117)
Experiments with hawkmoths support combining floral scent and visual display, which is the more useful functional analogy here. [Riffell and Alarcón, 2013](https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0072809)

- **Form vision:** obstacles and looming bodies at approximately 1.5 m by day and 0.5 m in dim conditions.
  Use about 300° coarse coverage, including above and below; motion is much more legible than a still predator.
- **Brightness/colour:** broad UV-plus-blue sensitivity, resolving conspicuous patches rather than detailed objects.
  Detect exposed flower patches around 4 m by day; detect an emissive lure around 2–3 m against a dark background.
- **Smell:** floral odour is the principal search sense, useful over about 4–6 m.
  It provides a coarse direction and increasing/decreasing signal, not an exact nectar coordinate through rock.
- **Hearing:** no general airborne hearing initially; avoid adding an unsupported mammalian ear.
- **Touch:** airflow/looming escape at close range, contact at the siphon, and vibration through a perch.
  Perch disturbance over roughly 0.5 m can trigger takeoff when the animal is awake and able to launch.
- **Conspecifics:** near-field movement and crowding within about 1 m; no distant flock channel.
- **Attraction:** scent plus bright patch strongly favours approach; brightness alone permits a shorter exploratory visit.
  A still lanternjaw can therefore deceive it, while a moving hunter supplies conflicting looming and motion evidence.
- **Ecological prediction:** fewer nectar-search failures, but more approaches to lures and some additional predation.
  Failed visits also consume flight energy; bellwing losses must include lure-associated starvation, not just kills.

**For the seedporter, I recommend the strongest detailed daylight vision in the community.**
Its selected body has large eyes, grasping limbs, gliding membranes and a prehensile tail. [animal-body-reimagining-2026-09-24.md:488](/home/wrysk/wryskware/cubarium/design/animal-body-reimagining-2026-09-24.md:488)
Primate colour vision offers a useful analogy for distinguishing fruit against foliage, although primate visual systems are not all identical. [Regan et al., 2001](https://pmc.ncbi.nlm.nih.gov/articles/PMC1088428/)

- **Sight:** conspicuous fruit patches, landing structures and large animals at 12 m by day; roughly 3 m at night.
  Use about 220° broad coverage with a sharper central region around 90°.
- **Static versus motion:** strong static discrimination is essential for fruit and landing sites; motion additionally attracts attention.
  Ripeness discrimination should become reliable closer in, around 4–6 m.
- **Colour:** strong multi-channel colour contrast for fruit; no UV requirement.
  Keep sensory colour properties independent of the display’s chosen palette.
- **Hearing:** useful predator movement and conspecific calls around 6 m, with louder disturbance farther away.
- **Smell:** ripe fruit around 2–3 m and conspecific/predator odour around 2 m.
  Smell confirms a patch and helps under foliage, while vision leads long-distance selection.
- **Touch/tremor:** excellent grip/contact feedback; branch movement within the occupied support can reveal nearby disturbance.
- **Brightness:** investigate only when colour, shape or odour suggests fruit; no generic lure attraction.
- **Ecological prediction:** less fruit-search starvation, more successful grove switching, and fewer unnecessary ground crossings.
  Better detection may increase fruit removal and seed transport, so measure recruitment and where seeds are delivered.

**For the lanternjaw, I recommend a close-range multisensory hunter that alternates patrol and still ambush.**
The selected direction is a mantis-scorpid with twin tail lures. [animal-body-reimagining-2026-09-24.md:63](/home/wrysk/wryskware/cubarium/design/animal-body-reimagining-2026-09-24.md:63)
Scorpion sensory work supports substrate/contact chemistry as an analogy; it does not establish kilometre-scale airborne tracking. [Gaffin and Brownell, 1997](https://pubmed.ncbi.nlm.nih.gov/9309869/)

- **Sight:** moving small prey around 4 m by day and 3 m at night; still prey around 2 m and 1.5 m.
  Use approximately 220° coarse coverage, with better localisation ahead of the jaws.
- **Colour:** luminance and blue/cyan contrast suffice; accurate fruit-like colour discrimination is unnecessary.
- **Hearing:** local rustling or wingbeat cues around 1–2 m; loud nearby footfalls can warn of a Veilhound.
- **Smell:** carrion around 4–6 m, live-prey odour around 2–3 m, and prey-habitat chemistry over similar local distances.
  Separate carcass attraction from living-prey search; scavenging can reduce the need to kill.
- **Touch/tremor:** strong substrate vibration sensing around 2 m, reaching roughly 3 m for heavy movement.
  Grounded prey, perched prey and airborne prey should couple differently to this channel.
- **Conspecifics/threats:** close chemical presence prevents crowding; a detected Veilhound can interrupt feeding or ambush.
- **Lure:** strongest effective deception while stationary, with a physical luminous position near an attainable interception zone.
  Brightness attracts prey; movement reveals the hunter and reduces the deception’s effectiveness.
- **Ecological prediction:** improved small-prey encounter rates and more scavenging, with bellwings becoming a supplementary opportunity.
  It should remain able to answer litter-feeder and capgnawer abundance instead of becoming dependent on pollinators.

**For the Veilhound, I recommend keeping the branch’s scale while making its evidence imperfect.**
Wrysk’s wolf/lion analogy supports good vision, hearing and smell together.
Wolf natural-history evidence supports strong olfaction and night vision, but exact detection distances remain context-dependent and incompletely measured. [NPS wolf ecology](https://home.nps.gov/yell/learn/nature/wolf.htm)

- **Sight:** retain 16 m daylight detection of exposed grazer-sized bodies and 240° coverage.
  Start night detection around 8–10 m; static, camouflaged or partly hidden targets should identify at shorter distances.
- **Colour/motion:** good form and motion discrimination; a limited colour channel is sufficient.
  It does not need seedporter-grade fruit discrimination or a UV flower channel.
- **Hearing:** retain roughly 10 m for substantial movement; louder calls may carry 12–16 m.
  Scale emission by gait, body size and surface, rather than making a snail and a running grazer equally audible.
- **Smell:** retain approximately 6 m for live-prey evidence; stronger carrion sources can be useful around 8 m.
  Coarse local scent persists briefly after passage, allowing investigation after visual contact is lost.
- **Touch/tremor:** normal contact and about 1 m of useful local footfall evidence; not its principal search channel.
- **Conspecifics:** identify nearby packmates visually, hear emitted calls, and use local den/pack odour.
  Shared information should have a sender, a location estimate, and an age.
- **Brightness:** can notice lanternjaw glow without being attracted to it as food.
  A luminous hunter may reveal itself to a larger predator.
- **Ecological prediction:** more acquisitions and approaches, followed by more grazer kills if reachability and hunger permit.
  Long-term predator abundance may rise, so the initial kill-rate improvement is not the whole population response.

**The resulting detection asymmetries should be conditional, not a universal predator-first ordering.**

| Encounter | Expected first useful detection | Ecological consequence |
|---|---|---|
| Veilhound versus exposed frondgrazer | Veilhound at long range; grazer during approach | Search improves while vigilance still creates escapes |
| Veilhound behind the grazer | Often grazer hearing before grazer sight | Rear approaches remain possible but cease being silent by definition |
| Veilhound behind foliage | Whichever receives stronger sound/scent first | Cover reduces precise pursuit without erasing all evidence |
| Veilhound versus loftstrider | Often mutual long-range awareness | Pack convergence and feeding displacement matter more than surprise |
| Moving lanternjaw versus shredder | Tremor may warn the shredder first | Slow approach and still ambush gain value |
| Still lanternjaw versus bellwing | Bellwing detects the lure before the hunter | Deceptive attraction creates an encounter |
| Lanternjaw versus snail | Usually hunter first | Retraction and inaccessible wet refuges matter more than running |
| Grounded seedporter versus hunter | Usually seedporter in daylight | Climbable escape routes become consequential |
| Sleeping bellwing versus hunter | Usually hunter first | Roost height and reach determine risk |

These are predictions from the recommended sensory relationships, not measured encounter outcomes.
A larger nominal range cannot compensate for the wrong heading, an occluding terrace, sleep, or a source that emits almost nothing.
Nor should detecting a predator automatically trigger flight: a distant static shape can justify watching.

**Flowers and lures should compete through a shared receptor, with different evidence behind them.**

- Give every flower a simulated UV/blue contrast signal associated with its flower tissue and orientation.
  A whole plant should not become a flower beacon merely because it is green or visually bright on the display.
- Keep UV reflectance separate from emission: reflective flowers need incident illumination.
  A night glowcap or lanternjaw lure emits its own light; it does not require moonlight to remain detectable.
- The lure need not emit UV specifically: a receptor sensitive to both UV and blue can respond to visible cyan emission.
  The relevant quantity is spectral overlap and contrast to the background, not the word “UV.”
- Flower scent should advertise a flower patch imperfectly; mouth taste establishes whether nectar remains.
  Empty flowers may remain visually conspicuous briefly, allowing useful memory of unrewarding visits.
- A still lure should produce approach, close inspection and attempted feeding or landing.
  It should never add nectar, teleport the bellwing, or automatically grant the predator a successful strike.
- **Fact:** current prey eligibility excludes bellwings in flight or falling, even though strike geometry separately checks physical overlap.
  That is narrower than Wrysk’s “grounded or within reach” direction. [predation.rs:108](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/predation.rs:108), [step.rs:1979](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/step.rs:1979)
- **Recommendation:** let physical interception determine whether a low-flying bellwing can be struck.
  Keep high flight safe through distance and geometry, with escape opportunity during the strike sequence.
- **Fact:** bellwing is diurnal and lanternjaw nocturnal in the physiology table. [body.rs:1412](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:1412), [body.rs:1225](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/body.rs:1225)
- **Recommendation:** expect lure encounters mainly during activity overlap, shaded daytime hunger, or twilight.
  Do not silently make bellwings fly all night; sleeping animals should not chase lights without an explicit arousal mechanism.
- Avoid presenting simple light attraction as a complete account of real moth behaviour.
  Experimental work shows that nearby artificial light can disrupt flight orientation rather than merely attract a direct approach. [Fabian et al., 2024](https://www.nature.com/articles/s41467-024-44785-3)

**The highest destabilisation risks are predator omniscience, persistent alarm, and deceptive food traps.**

- Unattenuated hearing and smell across the entire 6 m depth could remove nearly every local refuge.
  Combined with exact target coordinates, this would make terrain concealment largely decorative.
- Improved hunter survival on abundant prey can sustain pressure on a rarer alternative prey.
  Bellwings or snails could decline even while lanternjaws remain well fed.
- A lure can reduce bellwing recruitment through kills, wasted flight, interrupted feeding and attraction to unsafe roosts.
  Count all four pathways before attributing a decline solely to strike power.
- Glowcap attraction without chemical discrimination could trap bellwings at non-food lights.
  Failed-visit memory and stronger scent–brightness agreement at real flowers should limit this.
- Permanent vigilance can lower grazer reproduction even when few animals are killed.
  Measure feeding interruption separately from mortality.
- Better grazer or fruit finding can intensify plant consumption.
  A successful sense mechanism is not automatically a stabilising ecological change.
- The reported growing stock is frondgrazers; the answering role is the Veilhound, with acquisition currently implicated.
  For fungal excess, inspect capgnawer access and recruitment; for litter or film accumulation, inspect their respective consumers.
- Treat these other stock imbalances as questions to measure, not conditions demonstrated by this review.
  This follows the project’s explicit “roles before rate tuning” rule. [WORKING_POLICY.md:195](/home/wrysk/wryskware/cubarium/WORKING_POLICY.md:195)

**The cheapest adequate mechanism is a shared evidence sampler, not a full physical sensory simulation.**

- **Sight:** obtain nearby candidate bodies and resource patches from a spatial index.
  Apply range, field of view, apparent size, illumination and contrast filters before ray traversal.
- Use one eye-to-target ray initially, with an additional exposed-body sample when the centre is hidden.
  Targeted rays prevent small prey from falling forever between a sparse fixed fan’s rays.
- Keep a small sampled fan for nearby terrain, openings, overhead clearance and general vegetation.
  Long-range target detection and near-field collision guidance have different sampling needs.
- Terrain blocks sight; foliage supplies accumulated attenuation; a few leaves need not behave like a stone wall.
  Reuse occupancy and traversal machinery, with the same rule for hunter and prey.
- **Hearing:** each body emits a scalar movement signal from actual speed, gait, size and substrate.
  Add explicit call or wingbeat emission where appropriate; hovering need not be acoustically silent.
- Compute received strength with bounded inverse-distance attenuation and a terrain transmission factor.
  An occluded source can remain faintly audible; it should not retain the precision of an unobstructed sighting.
- Begin with a direct-ray terrain-loss approximation; add coarse propagation around obstacles only if scene tests show it is needed.
  Do not solve diffraction or trace many reflected sound paths.
- **Smell:** retain the existing surface fields for detritus, fungus and film.
  Their different source identities are already useful distinctions, not machinery to replace with one generic odour.
- For floral odour, carcasses and moving bodies, start with bounded source kernels sampled at the receptor and nearby offsets.
  Aggregate them into coarse cue classes and gradients rather than exposing exact source IDs.
- Use coarse air connectivity to reject sealed or disconnected spaces; a local flood can route cue influence around corners.
  Do not use an animal’s walkability graph as an air graph: scent can cross places the animal cannot walk.
- Start without wind; isotropic, attenuated odour is sufficient to produce search and reacquisition.
  Add wind later only if directional plumes materially change a measured ecological question.
- A short-lived coarse deposited scent grid can supply trails if current-source kernels prove insufficient.
  Avoid a separate full three-dimensional field for every species and modality.
- **Tremor:** reuse movement emission with substrate contact and connectivity.
  Flying bodies emit no ground footfalls; bodies on disconnected supports should not exchange ordinary substrate tremor.
- **Brightness:** sum visible source contrast in directional bins, separately from local ambient illumination.
  Keep display bloom, tone mapping and colour grading entirely out of the biological calculation.
- **Memory:** retain last evidence with confidence decay and a bounded search time.
  Losing sight should end exact tracking, while still permitting investigation of the last observed patch.

**The operation count is modest at 50–150 bodies if remote sensing is staggered.**
The simulation currently uses 20 ticks per second; walker manifests sample at 0.25 s, while bellwing’s controller samples each tick. [voxel/lib.rs:95](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel/src/lib.rs:95), [manifest.rs:770](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/manifest.rs:770), [manifest.rs:1165](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/manifest.rs:1165)

- Recommend remote body/food sensing around 4 Hz initially, staggered by animal; keep immediate contact and flight control at tick rate.
  Both rates remain knobs where they affect behaviour.
- A brute-force sweep is 2,450–22,350 directed body pairs per complete update.
  At 4 Hz on a 20 Hz simulation, that averages 490–4,470 cheap pair checks per tick.
- A 16 m axis-aligned ray spans about 128 voxels; oblique traversal crosses more.
  An unobstructed worst-case sweep of all pairs therefore reaches millions of cell visits per full update.
- After staggering, range/FOV rejection and spatial filtering, budget roughly tens of thousands to about a million cell visits per tick.
  This is an operation-count estimate, not a measured runtime guarantee.
- Hearing, smell and tremor can reuse pair distances, source emissions and coarse terrain relationships.
  They should not each rebuild the same neighbourhood search.
- Plant count matters independently of animal count: aggregate flowers, fruit, caps and carcasses into indexed local patches.
  Querying every stand for every animal would undermine the otherwise small body-pair cost.
- A 0.5 m coarse air grid for this box has 24,576 cells; one float channel is about 96 KiB before metadata.
  Surface-only wet/ground cues can use substantially fewer nodes.
- Precompute terrain connectivity, static ray directions, support geometry and coarse transmission data.
  Update body bins each tick; refresh resource emissions when stocks change and terrain caches when terrain changes.
- Reuse route components, but cache path fields independently from sensory observations.
  The branch’s approach currently computes a distance field over the leader’s whole component; repeated route work could exceed sensing cost. [pack.rs:562](/home/wrysk/wryskware/cubarium/.claude/worktrees/veilhound-search/crates/cubarium-voxel-fauna/src/pack.rs:562)
- Measure sampler time, ray cells, candidate pairs and route expansions separately at 50, 100 and 150 bodies.
  Quote milliseconds only after that benchmark, including a dense open scene and a heavily occluded scene.

**Heuristics and neural controllers should consume the same evidence, including uncertainty.**
The branch explicitly leaves its body perception separate from the neural cone. [perception.rs:1](/home/wrysk/wryskware/cubarium/.claude/worktrees/veilhound-search/crates/cubarium-voxel-fauna/src/perception.rs:1)
That is acceptable for an interim Veilhound repair, but not as the teaching interface for the full pass.

- Create one perception result that both teachers and neural input encoding read.
  World queries may implement sensing; controllers should not inspect hidden prey, inaccessible stock totals or exact unsensed positions.
- Preserve Self, contact, wetness, taste and motor feedback.
  Add wound, gut/satiety, local illumination, activity state and time since important evidence where behaviour needs them.
- Replace generic `body` with perceived prey, threat, conspecific and unknown evidence.
  Classification requires enough evidence; a rustle need not reveal species, and a static predator need not become harmless.
- The current classifier explicitly changes a sufficiently distant still predator from threat to other.
  Replace that hard erasure with lower detectability and retained confidence after identification. [predation.rs:126](/home/wrysk/wryskware/cubarium/crates/cubarium-voxel-fauna/src/predation.rs:126)
- Encode directional visual evidence with proximity, confidence and motion; include elevation for flyers and climbers.
  Keep traversability distinct from whether the target is visible.
- Encode hearing as directional intensity and change, with separate call evidence where supported.
  Encode smell as local concentration, directional gradient and trend—not a precise distance to a known animal.
- Add left/right tremor with validity, and brightness/UV evidence distinct from ordinary body or food recognition.
  Add a sensor capability mask so unavailable senses differ from valid zero readings.
- If teachers need selected targets, expose the same bounded evidence tracks to brains: bearing, elevation, estimated proximity, class confidence and age.
  Any shared navigation helper must receive those perceived goals, not recover hidden target coordinates.
- Predators should investigate weak evidence, stalk or approach strong evidence, strike only in physical reach, and stop when sufficiently fed.
  Prey should orient, assess, flee or freeze according to cue confidence and their locomotion.
- Bellwings should ascend or move away from looming danger when clearance permits; ground-style “turn and run” is insufficient.
  Lanternjaws should trade patrol against bounded waiting at promising encounter sites, extinguishing wasted lure use when appropriate.
- Consolidate these changes into one new manifest contract replacing the unbuilt P2 layout.
  Version channel meaning, normalisation, classification rules and effective sensory configuration without pinning a golden digest.
- **Brains should wait for that shared contract and functioning teachers, not for ecological equilibrium.**
  Then clone teachers and co-train interacting lineages; do not spend a training round teaching policies from observations that cannot explain their teachers’ actions.

**The predictions should be checked through encounter funnels and stock flows.**

| Change | Predicted immediate effect | Readouts that distinguish success from another failure |
|---|---|---|
| Veilhound remote sensing | More detections, approaches and chase starts | Detection → approach → reachable target → chase → strike → kill |
| Grazer vigilance | Earlier escapes, fewer captures per chase | First-detection distance, alert lead time, escape rate, feeding time lost |
| Lanternjaw chemistry | More carcass use and small-prey encounters | Carrion intake, search distance, kills by prey, failed reachable meals |
| Still lure | More bellwing inspections and close approaches | Lure-on time, approaches, landings, aborted visits, attempts, kills |
| Bellwing floral smell | Faster reacquisition of nectar | Time to first meal, between-meal distance, empty visits, flight expenditure |
| Capgnawer fungal cues | More feeding across neighbouring colonies | Cap consumption, fungal stock, occupied patches, capgnawer recruitment |
| Snail wet chemistry | Better local film removal | Film stock, scraped area, disturbance downtime, snail predation |
| Seedporter vision | Better fruit finding and grove transfer | Fruit intake, climb/glide outcomes, ground exposure, seed delivery |
| Loftstrider panorama | Earlier pack avoidance and redistributed browsing | Group approach geometry, browse locations, canopy removal, juvenile losses |

Log first detection separately for hunter and prey, including modality combinations and whether a clear sightline existed.
Measure kills per predator-hour, per hunting-hour and per encounter; raw kill totals confound abundance, appetite and opportunity.
Measure prey births, maturation, predation deaths, starvation and other deaths separately.
For bellwings, distinguish predation while grounded, perched, launching and physically intercepted in low flight.
Measure false lure visits, foregone nectar intake and subsequent energy shortage alongside direct deaths.
Track foliage, fungal caps, litter, film, fruit, nectar and carrion so an apparent population improvement does not hide resource collapse.
A predicted reduction in grazer abundance is not a claim about its magnitude, nor proof of a stable long-run population.

**I would implement and verify this in the following order.**

1. Finish the Veilhound repair and retain its detection/approach distinction.
   Correct range geometry, disabled-sense semantics, modality reporting and observation age before copying its machinery elsewhere.
2. Establish the shared perception result and migrate the existing external flee, tremor, patrol and carcass queries into it.
   Introduce the complete neural layout once, with unused capabilities explicitly masked.
3. Add frondgrazer and loftstrider vigilance alongside the improved hunter.
   Then add seedporter vision and preserve its distinct landing, climbing and food decisions.
4. Extend the existing decomposer chemistry with appropriate local light and disturbance sensing.
   Verify that capgnawers and snails still spend enough time consuming the stocks their roles answer.
5. Add floral odour, UV/blue targets and bellwing sensory control.
   Test ordinary flower finding before introducing deceptive lures into the same scene.
6. Connect still lure attraction and physically reachable bellwing predation.
   Compare still-lit, moving-lit and unlit hunters with otherwise identical sources and animals.
7. Run staged paired ecological comparisons, then teacher cloning and joint training.
   Do not change sensing, diet, reproduction and strike power simultaneously when trying to explain a result.

**Mechanism tests should construct scenes from the supplied config and assert causal relationships.**

- Put targets just inside and outside the configured three-dimensional range; repeat across the ring seam.
- Place targets inside and outside the configured FOV, and exercise panoramic coverage without inventing a rear blind spot.
- Disable each sense explicitly; even overlapping coordinates must not make a disabled channel report a detection.
- Insert and remove a wall; vary foliage attenuation independently; verify hunter and prey use the same visibility rule.
- Move the target vertically while holding horizontal distance fixed; high prey must not acquire unlimited range.
- Compare large and small targets, moving and still targets, and daylight versus dim illumination.
- Emit a call from a stationary animal; a speed threshold alone must not suppress it.
- Hold source identity constant while changing gait or substrate; received sound should follow the configured emission rule.
- Test sealed air spaces, connected openings and an animal-inaccessible gap separately.
- Verify fungal, litter, film and carrion cues respond to their own sources without unintended cross-channel substitution.
- Break wet connectivity and substrate contact; film chemistry and ordinary footfall tremor should change appropriately.
- Place a reflective flower in darkness, then illuminate it; place an emissive lure in darkness as a separate case.
- Offer scent alone, brightness alone and both together; verify the configured attraction rule, including valid zero signals.
- Move the lit hunter and hold it still; test the resulting change in evidence and approach behaviour.
- Verify a false flower provides no nectar and that unsuccessful inspection eventually ends.
- Place bellwings outside and inside the strike volume at different heights and mobility states.
- Let prey disappear after detection; confidence must decay and exact pursuit must not follow the hidden body.
- Check pack evidence sharing with and without an audible communication path; preserve sender and observation age.
- Feed the same perception record to teacher and neural encoders; neither may gain information absent from that record.
- Round-trip nondefault sensory config, verify registry coverage, and compare ecological settings across all four live presets.
- Assert schema compatibility behaviour without asserting today’s digest, tuned ranges, population totals or desired kill rates.

Keep these scenes short—single queries or a few controller intervals wherever possible.
For ecological measurements, branch paired arms from the same valid start, inspect initial geometry, and check movement, feeding and mechanism counters early.
Use the project’s staged, checkpointed runs and stop invalid or already-resolved arms rather than waiting through a long census. [WORKING_POLICY.md:160](/home/wrysk/wryskware/cubarium/WORKING_POLICY.md:160)
Across the prescribed multi-seed comparisons, judge the spread and the causal readouts before extending duration. [WORKING_POLICY.md:143](/home/wrysk/wryskware/cubarium/WORKING_POLICY.md:143)
The acceptance criterion for this pass is that each animal’s behaviour responds coherently to available evidence, with its ecological role visibly answering the relevant resource or prey stock.