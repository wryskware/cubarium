---
design_status: exploration
date: 2026-10-11
---

# Ethology facts for loftstrider vs Veilhound behaviour design

Evidence only. No simulation recommendations. Compiled 2026-10-11 for the behaviour-AI design thread.

## How to read this file

Source tags on each fact:
- **[R]** full text (or the relevant sections) was fetched and read in this session.
- **[A]** only the abstract, a database record or a search-engine summary of the abstract was seen. Numbers are as reported there.
- **[S]** secondary: popular article, institutional page, conservation-NGO page, or a later paper citing the original. Treat as lead, not fact.
- **[U]** could not be verified, or no source was found. Listed so the designer knows it is open.

"n" is sample size. "BL" is body lengths. Distances are metres unless stated. Where a number comes from one study it says so.

## 0. Body sizes, for scaling

- Loftstrider (3.4 m tall browser) and Veilhound (about 1.4 m long plus tail) are the targets.
- Real giraffe: adults stand roughly 4.3 to 5.7 m (cows lower), about 800 to 1,900 kg. [U] General-reference values, not fetched here. A 3.4 m loftstrider is therefore smaller than a real adult giraffe. It is nearer a large elephant's shoulder-plus-trunk height or a subadult giraffe.
- Real giraffe calf: roughly 1.7 to 2 m and 50 to 100 kg at birth. [U] General-reference value.
- African elephant cow: shoulder roughly 2.2 to 2.6 m, 2 to 3.5 t. Newborn calf roughly 90 to 120 kg. [U] General-reference values.
- Lion: females about 120 to 150 kg. Wolf: about 30 to 50 kg. [U] General-reference values.
- Escobedo et al. 2014 model parameters: wolf 35 to 40 kg, prey 350 to 400 kg (about 10:1). [R]
- Lion preferred prey mass 190 to 550 kg, peak preference about 350 kg; giraffe, buffalo, wildebeest, zebra and gemsbok significantly preferred. Hayward & Kerley 2005, J Zool 267/270:309, 32 studies, 48 locations. [A]
- Mean prey biomass in Hwange lion hunts 258 kg (SD 241). Loveridge et al. 2006. [R]
- African wild dogs typically kill prey of about 1.5 to 2 times their own mass; cooperation mainly raises the mass of prey that can be killed. Creel et al. 2026, Animal Behaviour (summary of the literature). [A]
- Prey flight distances listed for context: pronghorn 235, mule deer 149 to 250, elk 85 to 201, bison 101 m. Janosov et al. 2017 Table 1, citing a source not checked. Unit meaning (flight initiation vs escape run) not clear. [S]

## 1. Prey defence by large herbivores

### 1.1 Giraffe

**Share of lion diet and preference**
- Ruaha NP, Tanzania: giraffe were the largest prey category, 27% (171 of 641) of recorded lion kills across 16 species. Muneza et al. 2022, J Zool 316:49. [A]
- Hwange NP, Zimbabwe: giraffe 9% of lion kills found, 9% (1,150 kg) of biomass of observed hunts. Loveridge et al. 2006. [R]
- Giraffe is among the lion's significantly preferred prey species (see section 0). [A]
- In Tarangire, adult giraffes are not the lions' preferred prey (B. Kissui, unpublished, cited in Lee et al.). Lions are the main natural predator and may be the main cause of calf death. [R]

**Survival of attacks: claw-mark studies**
- Serengeti: 702 individually known giraffes, 3 areas, 2008 to 2010. Lion claw marks on 13% of giraffes older than 1 year. No calf (0 to 1 year) carried marks, taken to mean calves rarely survive an attack. Marks mostly on hindquarters and flanks, so non-lethal attacks come mainly from behind. Among adults over 5 years, marks were more common in females than males, plausibly from calf defence. Strauss & Packer 2013, J Zool 289:134. [A]
- Ruaha: 26% (140 of 548) of giraffes carried lion marks (claw marks, bite marks, missing tails). Marks were more frequent in adults and males, i.e. those more likely to survive an attack. Muneza et al. 2022. [A]
- Marks record survived attempts only. They cannot count attacks or date them. [A]
- Sex bias in kills conflicts between sources. One Serengeti paper says lions kill more male giraffe (Owen-Smith 2008, via a search snippet); a Kenyan study found no sex discrimination. Low confidence either way. [S]

**Calf mortality**
- Range of first-year calf survival estimates across African studies: 0.2 to 0.7 (Pratt & Anderson 1982; Lee et al. 2016, 2017), as summarised in a 2025 Nubian giraffe translocation paper (Conservation Science and Practice, csp2.70127). [S]
- Serengeti first-year mortality 58 to 73%, citing Foster & Dagg 1972 and Pellew 1983. Seen only second-hand in a conservation article. A secondary source also attributes 58 to 73% of Serengeti giraffe mortality to lions, citing Strauss & Packer. Neither verified against the originals. [S]
- Giraffe Conservation Foundation FAQ (advocacy source): Serengeti Masai giraffe calves: over 20% dead within a month, about half by 6 months, nearly 60% by one year. [S]
- Eastern Transvaal Lowveld, lions the only predator: about 48% of calves die in the first year. Author not identified in the record seen. [S]
- Tarangire Ecosystem, Tanzania, 5 subpopulations, 3 years: 860 adult females (neonatal survival via apparent reproduction), 449 calves tracked. Local lion pressure (lion density divided by primary prey density) was significantly negatively related to neonate and calf survival. When migratory wildebeest and zebra herds were locally absent, calf survival was 11% lower and apparent reproduction 37% lower. Lee et al. 2016, Ecology and Evolution (PMC5167056). [R]
- Calving rate varied little (CV 0.07) while reproductive success varied a lot (CV 0.39), so Lee et al. read the variation as neonatal predation mortality. [R]
- Life-history numbers quoted there: weaned at about 9 months, independent at about 14 months, birth interval 620 +/- 49 days. [R]
- Newborn giraffe are kept hidden in the neonatal period; the mother-calf bond lasts at least 12 to 16 months. [S] (giraffeconservation.org)
- Calves joining "nursery"/crèche groups of 2 to 3 mother-calf pairs with one adult babysitting is widely repeated in popular sources. [S] The one controlled test found the opposite: in two enclosed Kenyan populations adult females with calves were in smaller groups, against the crèche hypothesis. Muller et al. 2018, J Zool 306:77, 591 group records. [A]

**Kicks and defence**
- No peer-reviewed measurement of giraffe kick force or lethality against lions was found. The widely quoted "2,000 psi" has no traceable source. [U]
- A researcher quoted by National Geographic (2019): an adult male giraffe can kill a lion easily with one kick. A cooperating group of lions climbing on a giraffe's back failed in a roughly five-hour Klaserie/Kruger attack. Panthera's Luke Hunter says successful kills require getting the giraffe to the ground, by tripping it as it gallops or by lions climbing onto it to push it down. [S]
- Safari-operator video accounts describe mothers bucking and charging with neck outstretched to free a calf from lionesses, and a mother guarding a dead calf overnight against jackals, a hyena and briefly lions. [S] Anecdote only, no rates.
- The Ruaha study's framing (WildCRU summary): adult giraffe are harder to capture because they fend off attacks by kicking or outrunning lions. [S]
- Lions are reported to take adults weakened by drought and starvation (Hirst 1969), malnutrition (Brenneman et al. 2009), and young or old animals. Review in Muneza et al. 2022, seen via summary. [S]

**Where giraffe are exposed**
- Giraffe are "particularly vulnerable while drinking" because of the splayed or bent-leg posture. Drinking is preceded by longer scanning. They avoid waterholes when lion encounter risk is high. Burger et al. 2020 citing Seeber et al. 2012 and Valeix et al. 2009. [R as quoted]
- Lions in Hwange: preferred prey (buffalo, kudu, giraffe) avoided risky waterholes and spent longer drinking when lion risk was high. Valeix et al. 2009, Ecology. [A]
- Whether lions preferentially hit lying giraffes: nothing found. [U]

**Vigilance**
- Giraffe group size had little effect on scanning in bulls or cows. Lone bulls scanned less than bulls in groups. Cows scanned more when an adult bull was close. Presence of calves did not change scanning. Predation risk did not significantly alter vigilance; a baseline level seems constant. Cameron & du Toit 2005, Animal Behaviour 69:1337. [A]
- Southern giraffe, 51 animals, lion roar-grunt playback, two South African reserves. Both predator-naive and lion-experienced giraffe oriented rapidly to the roar. Vigilance lasted 513 +/- 421 s (n=24) in the population where lions were reintroduced 5 years earlier versus 49 +/- 46 s (n=27) in the lion-free reserve. Baotic & Szipl 2025, Frontiers in Ecology and Evolution 13:1634218. [R]
- In Hwange only kudu, not giraffe, changed vigilance significantly in the presence of lions (one Hwange study, summarised). [S]

### 1.2 Elephant

**Calf protection and mobbing**
- Amboseli, Kenya, 39 family groups, playback of 1 or 3 lion roars, male or female. Herds responded more to 3 than 1. All herds responded; only groups led by matriarchs aged 60 or more discriminated the more dangerous male roars. The response: matriarch raises head and ears, others draw quickly into a tight huddle around her; older-led groups bunched faster and were more likely to aggressively mob the speakers. Bunching with calves in the centre and adults facing outward is the general defensive pattern described in the same coverage (not specifically measured in the playback).  McComb et al. 2011, Proc R Soc B (summarised by Ed Yong, National Geographic). [S]
- Reported Amboseli attack on a calf: females responded with extended roaring and rumbling, which kept the lion at bay and drew in distant family members. ElephantVoices ethogram summary. [S]
- In 7 of 8 observed Hwange lion kills of elephants the victim was a lone calf that had lost contact with its family. Only one was killed with adults nearby. Adults are "too large and aggressive" to be tackled even by a large pride; juveniles "usually well defended". Loveridge et al. 2006. [R]
- Hwange herd attacks, reconstructed from tracks: 4 male and 3 female lions ambushed a small family unit. A subadult was caught but escaped after dragging a male lion about 10 m; a newborn (under 1 month) was killed because the herd "panicked and ran" and did not defend it. [R]
- Another Hwange hunt: lions (1 adult female, 2 males and 1 female of 3.5 years) attacked a small family at a water point in scrub. A lioness jumped on a 5 to 6 year old that lagged behind; two cows came back and defended it and it escaped. [R]

**Who is taken, when, how**
- Hwange 1998 to 2004: 44 juvenile elephant kills; mean age 2.4 +/- 1.5 years (range newborn to 7.5, n=42); modal class newborn to 1 year; 88% under 4. Elephants were 22% of lion diet by kills found and 23% by the abstract's count; 16% of biomass in observed hunts. Concentrated in the dry season of low-rainfall years, at waterpoints, where crowding and stress separate calves from family. Loveridge et al. 2006, J Zool 270:523. [R]
- Lion group size on 23 elephant-kill occasions: lone males 3 times (oldest calf taken by a single lion was 3 years); male-only groups mean 1.8 (range 1 to 4); female-only mean 7 +/- 2.9 (range 4 to 10); mixed 3.6 females plus 2 males. Mean pride size in Hwange is 2.5 adult females, so pride size at elephant kills (about 7 adult females) was well above average. [R]
- Lions cannot suffocate an elephant calf by throat or nose clamp, so kills take time and likely need cooperation. [R] (citing Joubert & Joubert 1994)
- Savuti, Chobe NP, Botswana: lions kill elephants in the late dry season (Aug to Nov), increasingly since 1985; a 30-lion pride killed one elephant every three days; 7 of 8 elephants killed were aged 4 to 11 (weaned, less maternally dependent). Observed with infrared equipment. Power & Compion 2009, African Zoology 44:36. [A]
- Chobe lions mostly take male elephants aged 4 to 15 who are still associated with a matriarch's herd (Gravett et al. 2017, citing Joubert 2006 and Power & Compion 2009). [R as quoted]
- Joubert 2006 (Afr J Ecol), secondhand via forum quotes: 74 elephants killed 1993 to 1996, 26 older than 9, mostly at night, more on waning-moon than bright-moon nights. [U]
- Spotted hyenas also take newborn and very young elephant calves in Hwange (dry season Sep to Nov 1999). Salnicki et al. 2001, Koedoe 44(2). [A]

### 1.3 Moose, bison, musk ox, Cape buffalo

**Moose (wolves, Isle Royale, Mech 1966, Fauna Series 7)**
- 66 hunts involving 132 moose watched in 68 hours. The pack tested 77 moose (held at bay or chased long distances) and killed 6, a "predation efficiency" of 7.8%. [S] (summary of the monograph, npshistory.com; original not read)
- Moose that stood at bay until wolves gave up: 36 of 36 survived. Moose that ran: 5 of 41 killed (12%). [S]
- A moose is usually killed within 10 minutes, but wounded animals can hold wolves off for several hours. Of 57 kills found, 18 were calves and most others were 8 to 15 years old; none 1 to 6. [S]
- Mech & Peterson 2003 open with 15 wolves surrounding a moose on Isle Royale; the moose held the pack off for about 5 minutes before the wolves left. [S] (chapter abstract)
- Scandinavia, 252 wolf attacks on moose: mean chase 76 m (unit printed "in", presumably metres), success 43%, at least one moose injured but not killed in 11% of attacks, fewer moose stood their ground than in North American studies. Wikenros et al. 2009, Acta Theriol 54:207. [A] Success 45% or 64% by two methods (185 attacks); only age of the breeding male was significant, not group size or snow depth. Sand et al. 2006, Animal Behaviour. [A]

**Bison (wolves)**
- Wood Buffalo NP, early summer, packs of 4 to 6 wolves, 14 recorded interactions: wolves preferred cow-calf herds. Five calf-defence moves: run to the cow, run to a herd, run to the nearest bull, get in front of and in the centre of a stampeding herd, run through water. Cows with young calves led a fleeing herd, bulls often at the rear. Cows, and especially bulls, sometimes charged wolves to defend the calf. Killing attempts lasted from minutes to 11 hours. Carbyn & Trottier 1988, Arctic 41(4):297. [R]
- A 5-hour episode in the same paper: four wolves seized a calf ten times; cow and bulls drove them off each time; the calf ended wounded and the wolves visibly exhausted and panting. In another, a cow with 7 to 11 bulls held four wolves off for 2 hours on a ridge. In one sequence wolves closed to about 300 m of a lagging calf and the cow and a bull turned back to defend it; wolves repeatedly "avoided the charges" of bulls. Herd stood huddled with tails raised, grunting and snorting. [R]
- Yellowstone: wolves attack bison groups less than elk groups and watch bison more. After capture, wolves killed elk in 81% and bison in 20% of captures. Harassment (lunge and feint) instead of pursuit never produced a kill when wolves selected a bison from a group; every attack on a solitary bison used harassment and none succeeded. MacNulty, Mech & Smith 2007, J Mammalogy 88:595, 517 elk and 134 bison encounters. [R]
- Elk that first confronted wolves then fled were less likely to be killed than elk that fled first and confronted last (the latter were probably weak). Same paper. [R]
- More wolves were killed by elk than by bison (5 versus 1; Yellowstone Wolf Project, in litt., quoted there), but elk were 79% of encounters, so this is not a per-encounter risk. [R]
- 14 bison kills in the first 4 years after reintroduction, all late winter, victims in poor condition, injured or young. Smith et al. 2000, J Mammalogy 81:1128. [A]
- A Yellowstone biologist (Cassidy) says 15% of collared wolves were killed by prey in hunts, and a bison once threw a wolf 10 to 15 m. Quoted, not a published rate. [S]

**Musk ox**
- Park Service description: against a lone wolf, musk oxen form a line; against a surrounding pack, a tight outward-facing circle with calves inside. Occasionally one animal charges and returns. An animal separated from the group is much easier to kill. A few grizzlies have learned to kill muskoxen inside the circle. nps.gov/gaar "Muskox: Circle Defense". [S]
- Ellesmere Island, July, 23 summers: packs of 2 to 12 adult wolves killed 7 calves, 1 yearling and 5 adults, at 2.9 to 32 km from the den; no relation of pack size to calf vs adult. Mech 2010, Canadian Field-Naturalist 124:258. [R]
- No quantitative success rate for circle vs flight was found. Blog figures (7 to 10% wolf success, 23 to 40 s formation time) cite nothing and are discarded. [U]

**Cape buffalo**
- Addo, South Africa, after lions were reintroduced in 2003: buffalo formed larger herds, shifted to shorter-grass open ground where lions are seen sooner, fewer were killed in larger herds, and males defended herds during attacks so more calves survived. Tambling et al. 2012, Ecology 93:1297. [A] (via CACE summary)
- Lake Manyara: chance of being killed fell steeply with distance from lion cover; buffalo were more vigilant at night, and edge animals more than central. Prins & Iason 1989, Behaviour 108:262. [A] (abstract fragment from a listing)
- Males from separate herds that normally fight unite to rescue a threatened youngster, calf distress calls bring defenders, one lion releases when a bull charges. [S] popular and safari-operator accounts only. No playback or rate study was found.
- Small lion groups do not kill buffalo; larger groups do (Funston et al. 1998; Scheel & Packer 1991, as cited in a 2026 wild-dog paper). Male lions are frequent, successful buffalo hunters, taking mostly calves and old bulls away from herds. Funston et al. [S]

## 2. Sleep and rest exposure

### 2.1 Giraffe

- Zoo, 152 nights of video, 8 animals: total sleep 4.6 h per day (juvenile excluded), REM-type sleep only 4.7% of that; main nocturnal period 20:00 to 07:00 split in two, plus afternoon naps. Tobler & Schwierin 1996, J Sleep Res 5:21. [A]
- Wild Angolan giraffe, Okapuka Ranch, Namibia, 77 animals over 24 nights (dry July to August 2016, wet February to March 2017), night vision and thermal imaging. Lions absent from the ranch (one lioness in a fenced enclosure; leopard and cheetah present). Burger et al. 2020, Ecology and Evolution (PMC7083675). [R]
  - Observation window was 2 h before to 2 h after sunset. Over the 2 h after sunset: dry season 25.8% active, 68.2% lying, 6.0% in REM-like posture (RSP); wet season 48.0% active, 49.9% lying, 2.1% RSP.
  - First RSP about 30 min after sunset, about 15.8 +/- 18.3 min after sitting down. RSP events: mean 121.9 +/- 156.1 s, median 54.5 s (dry 155.2 s, wet 85.8 s); 152 events, 50 individuals, 14 of 24 nights.
  - Daytime groups up to 25, fission-fusion. Night groups 3 to 12. Resting sites: single trees or sparse bushes in open ground, good visibility. Animals within a radius of up to 30 m (six animals over about 50 m); older animals on the periphery; smaller groups (2 to 4) closer together.
  - Guarding: in a 13-animal group over 3 h at least three stayed vigilant while others lay and slept; at least one giraffe was vigilant standing in all observation periods. Simultaneous RSP rare, maximum 3 at once. RSP duration did not correlate with group size (Spearman rs = -0.035).
  - Rest period after sunset 3 to 5 h; groups often left the site near midnight (search-summary figure, [A]).
- Whether wild total daily recumbent time is known: not found. Popular sites give 30 min to 5 h, unsourced. [U]

### 2.2 Elephant

- Two wild matriarchs, Chobe, 35 days, trunk actiwatch plus GPS collar: about 2 h sleep per day on average (1.8 and 2.3 h), polyphasic, mostly 02:00 to 06:00. Standing and recumbent sleep, but recumbent only every third or fourth day (about 12% of total sleep time in one animal). Went without sleep up to 46 h on five occasions while moving about 30 km in 10 h, possibly after predation or poaching disturbance or a musth bull. No sleep rebound. Chose a new sleep site each night. Gravett et al. 2017, PLoS ONE 12:e0171903. [R] n=2 animals, so low confidence for generalising.
- Captive elephants: about 3 to 7 h per day. A teaching page citing Tobler 1992 says undisturbed captive elephants lie down 30 to 120 min at a time. [U for the lying figure]

### 2.3 When predation on resting or sleeping animals happens

- Etosha lions hunt "almost exclusively at night" on the plains. Stander 1992. [R]
- Success is higher in the dark: lion hunting success highest on dark nights in South African and Ugandan studies; Serengeti/Ngorongoro lions fed more near the new moon. Packer et al. 2011, Proc R Soc B (PMC3140494). [A]
- Savuti lion kills of elephants mostly at night (see 1.2, Joubert, unverified). The Jouberts' 1997 account describes confused calves separated from herds at the Savuti waterhole being killed at night. [S]
- No source found that quantifies kills of lying giraffe or lying elephants. Elephants lie down rarely (above), giraffe lie in groups with some standing guard. [U]

## 3. Signals

### 3.1 Prey alarm and distress

**Giraffe**
- 947 h of zoo audio, three European zoos: the snort is the most common vocalisation and appears in alarmed, annoyed and approach contexts. It is broadband without harmonics (a burst of air). No dedicated alarm call identified. No infrasound found; earlier infrasound claims did not replicate. Nocturnal "humming" (harmonic, sustained) exists. Calf bleats are anecdotal. Baotic et al. 2015, BMC Research Notes 8:425. [A]
- Lion roar playback triggers fast orienting in all giraffe, with longer vigilance in lion-experienced animals (see 1.1). [R]

**Elephant**
- Rumble information transmits efficiently to about 1.5 km (male rumbles in Addo, field propagation test). Contact calls reported audible to about 2 km. Seismic transmission of calls reported to 16 km (secondary). PMC6210294; Wikipedia "Elephant communication". [S]
- Predator-related alarm calls played to breeding herds at Mushara, Namibia as seismic signals made herds clump and leave the water hole sooner (preliminary). O'Connell-Rodwell et al. [S]
- Bee-threat alarm rumbles make others retreat faster. King et al. 2010. [A]
- Distress-call response latency and distance for calves: not found. [U]
- Elephant hearing: audible range about 17 Hz to 10.5 kHz at 60 dB SPL (Asian elephant, one animal). Heffner & Heffner 1982, J Comp Physiol Psychol 96:926. [A]

**Buffalo and bison**
- Bison under wolf attack grunted and gave loud snorts; calves stayed in "pods" in the middle of the herd. Carbyn & Trottier 1988. [R]
- Calf distress calling pulling in defenders is reported from safari accounts for Cape buffalo. [S] No primary study.

### 3.2 Predator coordination cues

- Wolves: no vocal or visual signal specific to hunting is described in the Yellowstone ethogram; the states are search, approach, watch, attack-group, attack-individual, capture. MacNulty et al. 2007. [R] Howling mainly coordinates pack activity and territory (Harrington & Mech 1978, Can J Zool, Minnesota homesites; 1979 experiment with radio-collared wolves). [A]
- Wolf howl range: about 10 km in forest and 16 km on tundra (Mech & Boitani, secondhand); at least 9 km in woods and 16 km in open (Harrington & Mech 1979; Henshaw & Stephenson 1974, from a dissertation citation). Maximum, not typical. [S]
- Lions: group hunts are silent stalks; roaring serves regrouping afterwards. Tail and ear signals are widely repeated in popular sources but not sourced to primary studies. [S/U]. Stander's 486 cooperative hunts are described entirely through positions and stare-then-stalk behaviour ("all lionesses stared at prey with an alert posture and facial expression"), with no signalling mentioned (absence from the text is not proof none exists). Roles adjust to who is present and where they are. Stander 1992. [R]
- Spotted hyenas: cooperative hunts reported as stigmergic coordination without communication; vocal communication mainly over food competition with lions (conference abstract). [S]
- Muro et al. 2011 and Escobedo et al. 2014 show pack-style encirclement without any communication in a model (section 5). [A/R]

### 3.3 Hearing ranges where known

- Elephant: see 3.1. Dog (proxy for wolf, no wolf audiogram found): about 62 Hz to 45 kHz at 60 dB SPL, best near 8 kHz, four dogs 4.5 to 30 kg. Heffner 1976. [A]
- Cattle (proxy for buffalo): about 23 Hz to 35 kHz at 60 dB. Heffner & Heffner 1983, Behav Neurosci 97:299. [A]
- Cat 45 Hz to 64 kHz (LSU table, from Heffner data). No lion or giraffe audiogram found. [U]
- Lion roar audibility (often quoted as about 8 km): not verified. [U]

## 4. Predator tactics

### 4.1 Wolves

**Testing herds, selecting targets** (Yellowstone, MacNulty, Mech & Smith 2007, >2,150 h of observation) [R]
- Wolves approach to cut distance and judge vulnerability, then watch, then attack the group; attack-group usually ends in selecting one animal by pursuit. Selection shows as higher stride frequency: 2.06 +/- 0.08 to 2.43 +/- 0.06 strides/s (n=18 elk).
- If wolves fail to select an individual from an elk group they typically abandon the encounter. With bison, attacking the group more often leads to more watching than abandoning.
- Elk are about 3 times as vulnerable as bison; wolves attack elk about twice as often.
- Wolves killed elk and bison in 81% and 20% of captures.
- Isle Royale: of 77 moose tested 7.8% killed; none of 36 that stood their ground. (section 1.3) [S]
- Prey chosen are mainly young, old or debilitated: Isle Royale kills were calves or 8 to 15 year old moose, none aged 1 to 6. [S]

**Pack size and success**
- Elk: a single wolf has about a 2% chance of killing an adult elk (NPS Yellowstone Science 24-1 summary). Success rises up to 4 wolves; overall P(kill | encounter) peaked at 4 and did not improve beyond 3 to 4. Each added wolf below the threshold raised the odds by about 44 to 45%. Individual effort declines beyond the threshold, most for the riskiest task (kill, then select, then attack) and most in non-breeders: free riding. 469 wolf-elk encounters, 97% watched from the ground. MacNulty et al. 2012, Behav Ecol 23:75. [R]
- Bison: the capture threshold was 9 to 13 wolves (best model 11); the attack threshold 3 to 6 (best 4); each added wolf below the threshold raised the odds of capture by 40%. Analysis used 218 (attack) and 106 (capture of adult bison) encounters out of 239; 60% watched in Pelican Valley. Bison were about 3 times harder than elk; bison captured were rarely killed. MacNulty et al. 2014, PLoS ONE 9:e112884. [R]
- The authors suspect large packs avoid bison where less dangerous prey exist, because the profitability after injury risk is low. Wood Buffalo wolves, who hunt mainly bison, live in packs of 8.6 +/- 0.7. [R]
- Moose (Scandinavia): no significant effect of group size; success 43 to 64%. [A]
- Per-capita food intake in wolves is maximal in packs of 2 (cited in Escobedo et al. 2014 from Vucetich et al. 2004). [R as quoted]
- Top hunters are 2 to 3 years old; heavier wolves hunt elk better; males outperform females. NPS summary of MacNulty. [S]

**Participation**
- Only part of a pack hunts: "hunting group" is the subset in a foraging state (pack 2 to 27 wolves, prey groups 2 to 800 in MacNulty 2007). MacNulty 2014 explicitly distinguishes hunting group from pack size. [R]
- Isle Royale: one 15 to 16-wolf pack was the large pack; hunt-level counts not given. [S]

**Long engagements and give-up**
- Bison calf attacks lasted minutes to 11 hours (Carbyn & Trottier; wolves rested 20 to 100 m away between rushes, watching). [R]
- Moose: usually killed within 10 minutes; wounded moose may hold wolves off for hours. [S]
- Scandinavia: mean moose chase about 76 m, shorter when the attack succeeded. [A]
- "Wolves give up after one or two kilometres" is repeated in a modelling paper without a primary citation. [U]

**Injury and death from prey**
- Yellowstone: elk killed 5 wolves and bison 1 (Wolf Project, in litt., quoted in MacNulty et al. 2007; period not stated there). Elk were 79% of encounters. [R as quoted]
- The risk of injury from kicks, trampling and antler stabs rises as wolves go from attacking to selecting to killing and proximity increases. [R]
- Cassidy (Yellowstone): 15% of collared wolves killed by prey in hunts. Not a published rate. [S]

### 4.2 Lions

**Roles and formations** (Etosha, Namibia, Stander 1992, Behav Ecol Sociobiol 29:445; 486 coordinated hunts, 4 prides, individually branded lions, night-vision viewing at 20 to 100 m) [R]
- Formation: "wings" circle the prey while "centres" wait for it to move toward them. Wings stalked a mean 320 m (SD 256, n=271); centres a mean 30 m (SD 31, n=278).
- Wings often initiate the attack. Centres capture prey in flight from the others. In successful cooperative hunts on large fleet-footed prey, 73% (n=92) of captures came from the ambush position. A "rush" capture follows a chase of up to 150 m.
- Each lioness repeatedly takes the same position. Hunts where most lionesses held their preferred positions succeeded more: preference rating above 0.5 gave 17% success (n=337) against 7% (n=61) below 0.5.
- Flexible: roles shift with group composition and where companions are.
- Group definition: pride members within 200 m of each other. Hunting group mean 3.6 lionesses in Etosha. All lionesses present participated in every hunt (cheating less likely than in the Serengeti).
- Prey group size in these hunts averaged 17 (range 1 to 200). Prey: springbok, zebra, wildebeest 83% of kills; 28 hunts on gemsbok, giraffe and ostrich together.

**Group size and success**
- Etosha: solitary hunts rare (1%) and 2.3% successful; hunts where all lions stalked directly ("conformist") 14% successful; cooperative hunts 27% successful and 68% of kills; success rose with group size (rs=0.82). Overall capture success all species 15%. Stander 1992 (both papers). [R]
- Serengeti: Scheel & Packer 1991 measured participation in 64 communal hunts on four prey species. Lions refrain more (free-ride) on easier prey. Cooperation is common on larger, harder prey. Animal Behaviour 41:697. [A]
- Foraging success is maximised by group sizes of one female and 5 to 6 females in the lean season; not by group size in the abundant season. Packer et al. 1990, Am Nat 136:1. [A]
- Serengeti numbers often quoted, solitary 17 to 19% success versus about 30% in groups, come from unnamed secondary sources. [S]
- Kruger: prey species is the most important of seven factors on success; lion sex had no effect on overall success; females favour zebra and wildebeest, males buffalo. Funston et al. 2001, J Zool 253:419. [A]
- Males kill where lines of sight are shorter (16.2 m shorter than at rest sites); females did not show the difference. Loarie et al. 2013, Animal Behaviour. [R]

**Hunting giraffe and elephants**
- Giraffe: see 1.1. Adult giraffe are taken by lions in Ruaha at 27% of kills. How: trip a galloping giraffe, or climb on and push it down (Hunter, via National Geographic). [S] A blog claim of 10 to 20% success on adults and over 50% on calves has no source. [U]
- Elephants: only calves and juveniles, mostly lone or separated, at waterpoints in the dry season; large prides (about 7 adult females or 30 lions in Savuti); also male juveniles aged 4 to 15 in Chobe; lone male lions took calves up to 3 years. See 1.2. [R/A]
- Lions separate a calf: Hwange kills were calves out of contact; the single herd raid killed a newborn only after the herd ran. [R]
- Night: see 2.3.

### 4.3 African wild dogs and spotted hyenas

- Wild dogs: Selous, 905 hunts, 404 kills (Creel & Creel 1995): success rose with the number of adults. Serengeti pack (Fanshawe & FitzGibbon 1993): larger groups hunted more successfully and defended kills better against hyenas; single dogs took adult and immature Thomson's gazelles but never wildebeest calves; pairs could not take adult wildebeest. [A]
- Wild dog hunting success is quoted as 60 to 90% in one source, conflicting with other figures. [S]
- Wild dog hunting success peaks at about 4 dogs per Escobedo et al. 2014, citing earlier work. [R as quoted]
- Wild dogs are cursorial: they search, pursue and kill communally. Prey about 1.5 to 2 times body mass. Creel et al. 2026. [A]
- Hyenas, Masai Mara: over 75% of 272 hunting attempts made alone, even against prey about 3 times their mass; zebra the one prey usually taken by groups; about a third of attempts ended in a kill; group size and hunter age predicted success, social rank did not. Holekamp et al. 1997, J Zool 242:1. [A]
- Hyena predation on very large prey (elephant, giraffe) rose with lion numbers in Hwange (Périquet et al. 2015, J Zool 297). [A]
- Cutting out individuals and harrying specific to these two species: no quantified source found. [U]

### 4.4 Hunt success by prey type, group size, age class (collected)

- Lion, large fleet-footed prey, Etosha: solitary 2.3%; stalking-all-direct groups 14%; cooperative 27%. [R]
- Wolf, Yellowstone: elk captured then killed 81%, bison 20% (of captures). Elk roughly 3 times as easy to catch as bison. [R]
- Wolf, elk, per hunting-group size: about 2% at 1 wolf rising to a plateau at 4 wolves. [S/R]
- Wolf, moose: 7.8% of tested (Isle Royale); 43% (Scandinavia). Standing vs running moose: 0 of 36 vs 5 of 41. [S/A]
- Wolf, muskox, July: adults and calves both killed (5 adults, 7 calves in 23 summers). [R]
- Wolf, bison calf: cow and bulls repeatedly freed a seized calf in long engagements (one 5-hour episode, calf seized ten times); that episode's final outcome is not given in the part read. [R]
- Lion vs giraffe: Serengeti no claw marks on calves = calves rarely survive an attack; 13% of >1 year olds carry marks. [A]
- Lion vs elephant: only juveniles; success concentrated on lone calves. [R]
- Age class: wolves take calves and old animals, lions take calves and subadults of elephants, calves of giraffe; adults of the largest species are rare prey. [R/A/S]

## 5. Computational and agent-based models

### 5.1 Wolf-pack encirclement from two local rules (Muro, Escobedo, Spector & Coppinger 2011, Behavioural Processes 88:192) [A]
- Agents are autonomous, interchangeable and indistinguishable. Each needs only the positions of the other agents. No communication, no hierarchy.
- Rule 1: a wolf approaches the prey until it reaches a minimum safe distance.
- Rule 2: a wolf that is close to that safe distance moves away from nearby wolves that are also close to it.
- Reproduces tracking, pursuit and encirclement. I did not read the full text, so the prey rule and parameters are not captured here. [U]

### 5.2 Escobedo, Muro, Spector & Coppinger 2014, J R Soc Interface 11:20140204 (PMC4006263) [R]
- Wolves and prey are particles with attractive and repulsive forces, Newtonian dynamics with friction, no alignment forces, no noise in the base model.
- Assumes one prey has already been isolated from the herd.
- Wolf-prey: asymmetric. Wolves always repulse the prey and are always attracted to it (long range attraction, short range repulsion); the prey's attraction weakens with distance; a wolf stops approaching at the safe distance dc so it is not harmed by horns or legs.
- Wolf-wolf: no attraction at all. The only interaction is repulsion, active when both wolves are about da from the prey (a Gaussian window), with da larger than dc, so wolves space out before they are menaced. Reasons given: individual space, collision avoidance, better view of prey, room to respond to attack.
- dc and da vary with the hunt: dc falls when prey tires or falls and rises when prey counterattacks.
- Normalised parameters: prey mass 1, wolf mass 0.1 (350 to 400 kg vs 35 to 40 kg); friction prey 2, wolf 1; prey-on-wolf coefficient 2; wolf-wolf 0.5; wolf-on-prey 0.2; dc 1; da 1.5.
- Result: wolves stay in a bounded neighbourhood of the prey even with only repulsion between wolves. For N of 5 or fewer, the stable state is a regular polygon centred on the prey with all wolves at one distance. Above N* = 5 (and dc below a critical value) the polygon is unstable, wolves spread over two or more orbits at different distances, which gives apparent leaders (inner orbit) and apparent holders-back (outer orbit), then travelling or rotating formations, oscillation and chaos. Authors tie this to the observed peak of hunting success at small pack sizes.
- Previous empirical claim cited: interference between wolves was not found, so the loss is from wolves holding back. [R]

### 5.3 Group chasing (Janosov, Viragh, Vasarhelyi & Vicsek 2017, New J Phys, arXiv:1701.00284) [R]
- Chasers chase the closest escaper; a soft repulsion between chasers; a simple linear prediction of the target's future position. Finite acceleration, a time delay, noise. Soft-walled finite arena. Catching distance equals two agent radii.
- Escaper tactics: direct escape or erratic zigzag. Encirclement emerges. Collective chasing raises success against a faster erratic escaper.

### 5.4 Selfish herd and zone models
- Hamilton 1971, J Theor Biol 31:295: each animal's "domain of danger" is the ground nearer to it than to any other animal; moving toward the nearest neighbour shrinks it. Later models found moving to the group centre or a time-minimising rule beat nearest-neighbour (King et al. 2012 sheepdog trials; James, Krause & Bennett 2004). [A]
- Couzin et al. 2002, J Theor Biol 218:1 (read): each individual obeys, in priority order, repulsion from anyone inside radius rr (about 1 unit), else alignment with neighbours in an orientation zone (0 to 15 units wide) and attraction toward those in an attraction zone (0 to 15 units wide); blind angle behind (field of perception 200 to 360 degrees); constant speed 1 to 5 units/s; turning rate 10 to 100 deg/s; angular noise 0 to 11.5 degrees; dt = 0.1 s; N = 10 to 100. Small changes in the zones shift the group between swarm, torus, dynamic parallel and highly parallel states. [R]
- Alarm and mobbing model (Watzek et al. 2021, Behav Processes): prey can detect, signal and retreat or approach. Collective signalling raised survival over individual detection in several scenarios. Signalling alone sometimes lowered detection, but that vanished when signallers also approached the predator, as in mobbing. No central leader needed. Parameters not read. [A]

### 5.5 What was not found
- No quantitative minimal-rule model of a prey ring (musk ox/bison/elephant bunching with calves inside) was found. The musk-ox circle, bison "run to the nearest bull" and elephant bunching are known only as observed behaviours. [U]
- No model reproducing lion wing-and-centre formations was found beyond Stander's descriptive data and Packer & Ruttan 1988 (game-theory costs, not read). [U]

### 5.6 Field regularities a model could be checked against
- Isolating one prey: pursue until one animal speeds up and separates (wolves, stride frequency rises ~18%); lions: centre waits at 30 m while wings detour about 320 m. [R]
- Prey side: calves run to the nearest adult or bull; cows lead a stampeding herd; bulls guard the rear (bison). Elephants bunch toward the matriarch. [R/S]

## 6. Group size effects

**On the predator side** (see 4.2 to 4.4)
- Lions in the open on large prey: group size raises success monotonically in Etosha. Wolves: success plateaus at about 4 on elk and about 11 on bison. Wild dogs: about 4. Hyenas: mostly solitary except on zebra. Free-riding lowers per-capita effort past the plateau. [R/A]

**On the prey side**
- Banff elk: wolves met larger elk groups than their availability alone predicted, and killed a higher share of elk from large groups than encounters implied. Elk seemed to adopt either small rarely-found herds or large herds that diluted risk. Hebblewhite & Pletscher 2002, Can J Zool 80:800. [A]
- Elk herd size increased with distance from cover and decreased when wolves were present for mixed herds in Gallatin (large mixed herds shrank; small bull groups grew slightly). Creel & Winnie 2005, Animal Behaviour 69:1181; Winnie 2006 thesis. [A]
- Buffalo: fewer killed in larger herds, males defended, calves survived. Tambling et al. 2012. [A]
- Giraffe: group size had little effect on individual scanning (Cameron & du Toit 2005). Groups were smaller where lions were present, and the authors judged predation not to be a driver of giraffe grouping. Muller et al. 2018. Group size did not affect RSP (sleep posture) duration, but several animals guarded (Burger et al. 2020). [A/R]
- Elephants: kills in Hwange were mostly lone calves; family groups protect calves. In one attack, a herd that fled lost a newborn; in another, two cows returning saved a lagging juvenile. [R]
- Per-capita risk versus group size for giraffe or elephant specifically: no estimate found. [U]
- Classic many-eyes and confusion-effect literature (Pulliam 1973; Roberts 1996; Beauchamp; Krause & Ruxton 2002) was not retrieved. [U]

## Facts that most constrain a simulation

1. Lion success on large fleet-footed prey is tiny alone (2.3%) and grows with group size (27% cooperative, rs 0.82), and about 73% of captures happen from an ambush position (Stander 1992, Etosha, night hunts).
2. Lion cooperative formations are wing/centre: wings detour about 320 m, centres move about 30 m; same individuals repeat roles; hunts succeed more when lions are in their usual roles (17% vs 7%). No signals are reported, only positioning on what the others do.
3. Wolf success on elk plateaus at about 4 wolves and individual effort falls past it; on bison it keeps rising to 9 to 13. Only a subset of the pack takes part.
4. Wolves test and abandon: they approach, watch, then attack; most encounters with a standing, aggressive prey end without a kill. Moose that stood at bay: 0 of 36 killed. Moose that ran: 5 of 41 killed.
5. Harassment (feinting) without pursuit did not work on bison; killing a bison needs many wolves and long engagements (minutes to 11 hours), and wolves visibly tire.
6. Adults of the biggest prey are rarely killed. Lions take elephant calves, mostly lone or separated and weaned juveniles; Hwange 7 of 8 observed kills were lone calves; adults are "too large and aggressive".
7. Calves in the family group are defended in different ways: elephants bunch, calves inside; bison cow and nearest bulls charge and shield; buffalo males defend. A herd that panics and runs loses calves.
8. Lone-calf separation is the typical failure: crowding at water in dry years, loss of contact, a lagging juvenile.
9. Giraffe calf mortality in year one is huge in lion country: first-year survival 0.2 to 0.7; Serengeti claw marks never on calves; 13% of older giraffe have survived attacks (hindquarters, so attacks from behind).
10. Giraffe adults defend by kicking, but no kick force or lethality numbers are available; treat any numbers as design choices.
11. Resting exposure: in the 2 h after sunset wild giraffe lay down 50 to 68% of the time, in groups with at least one and sometimes three or more standing guards (study site had no free-ranging lions); REM-posture bouts last about 55 s median, 2 min mean; older animals on the periphery; sites in the open with sight lines. Elephants sleep only about 2 h a day, lie down every third or fourth day, and change site nightly.
12. Lions hunt mostly at night and succeed more in the dark; elephant kills at Savuti and Hwange are mostly at night or at dry-season water points.
13. Warning signals available: giraffe snort; elephant bunching toward the matriarch, rumbles audible to about 1.5 to 2 km, hearing 17 Hz to 10.5 kHz; wolves howl for pack cohesion (heard to 9 to 16 km). Calf distress-call response latency and distance were not found.
14. Minimal encirclement rules: approach to a safe distance, repel neighbours who are also near that distance; no communication. Above about 5 wolves the pattern breaks into inner and outer orbits, a modelled counterpart to withholding effort.
15. Experience matters for prey: lion-experienced giraffe stayed vigilant ten times longer after a roar; elephant matriarchs aged 60 or more tell male from female lion roars.

## Could not verify or not found

- Giraffe kick force, lethality to lions, mother-defence rates; lying-giraffe predation; giraffe hearing.
- Elephant calf distress-call response times and distances; elephant counter-attack kill rates.
- Quantified lion success against giraffe and against elephants; Joubert 2006 original numbers.
- Cape buffalo distress-call playback or rescue rates; Prins & Iason 1989 full text.
- Muskox and bison ring-formation success rates; Tener, Gray and Heard muskox papers.
- Muro et al. 2011 prey rule and parameters; models of prey ring formation.
- Lion roar audibility; wolf and lion audiograms; many-eyes primary sources.
- Wild dog and hyena cutting-out tactics on large prey with numbers.
- Mech 1966 original monograph (only the summary seen), Pellew 1983, Foster & Dagg 1972, Funston 2001 full text.
