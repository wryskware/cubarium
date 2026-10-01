---
status: Wrysk said go 2026-10-01
date: 2026-10-01
owner: Fable (organism line)
follows: design/handoffs/voxel-predation-2026-09-28.md (P1)
---

# P1 addendum: lanternjaw hatchlings start as parasites

**Why.** In P1's census every hatchling starved: a hatchling (body 0.0034) can only take
prey up to 0.0051, and the smallest adult prey is 0.0125. Wrysk's call (2026-10-01): the
hatchlings start as parasites. The model is freshwater-mussel glochidia, which clamp onto a
passing fish, feed on it with little harm, and drop off as juveniles.

**Don't tune for the current ecology.** The x30 world already starves its grazers. A
large tuning pass over every parameter comes later. This package builds the mechanism with
plausible defaults and exposes every number as a knob.

## Every number is a knob (Wrysk, 2026-10-01)

- Each new number is a named, serde-visible config field (`FaunaConfig` or the lanternjaw's
  species/physiology config, whichever owns similar fields), with a doc comment giving its
  unit and meaning.
- No inline literals and no new `const`s for ecological numbers. Only pure numerics stay
  `const`: tolerances, epsilons.
- These fields will become rows in the parameter registry
  (`design/ecology-parameter-registry-2026-09-28.md`). List each one in your return so the
  integrator can add its row.

## Behaviour

1. **Hatching needs a host.** An egg that passes its threshold at dusk and wins the hatch
   draw looks for a host. A host is the nearest living non-lanternjaw animal within
   `parasite_attach_radius_m` of the egg that has fewer than `parasite_max_per_host`
   riders. With no host, the egg stays dormant and tries again at a later dusk. Its
   viability loss and footfall count carry on as before.
2. **Attached.** A rider has no motion, controller, sensing or strikes of its own. Its
   position is its host's, at a fixed body anchor, including across face seams. It cannot
   be struck or eaten on its own.
3. **Drain.** Each tick a rider takes `parasite_drain_of_host_basal` × the host's basal
   upkeep from the host (default 0.01). The drain is a transfer through the existing
   three-currency pro-rata path, so the conservation ledgers close. The rider pays its own
   upkeep and grows from what it takes, by the normal growth rule.
4. **Drop off.** When the rider's body reaches `parasite_detach_body_fraction` × the adult
   body (default 0.24), it detaches at the host's current position. It is then a free
   juvenile lanternjaw on the ambush heuristic.
5. **Host dies** (any departure, including being killed or removed): every rider detaches
   at once at that spot, whatever its size.
6. **The drain never kills the host directly.** The host just pays more upkeep; if it
   starves, that counts as starvation as usual. No protection rule.

## Also

- **Snapshot:** save the attached state as the host's id. The fauna snapshot validator
  checks that the host exists, is not a lanternjaw, and has at most `parasite_max_per_host`
  riders. Schema: main's next free number. Old worlds are refused (no migration).
- **Census columns:** riders attached, drop-offs, riders freed by a host's death.
- **Look:** a plain-voxel interim look, a small body on the host's back. Workers don't
  invent a final look.
- **The ambush hatchling path** is used only after drop-off now. Leave it in place.

## Defaults (plausible, not tuned)

| field | default | unit |
|---|---|---|
| `parasite_attach_radius_m` | about one prey body length at the shipped mass | m |
| `parasite_max_per_host` | 1 | count |
| `parasite_drain_of_host_basal` | 0.01 | fraction of host basal upkeep |
| `parasite_detach_body_fraction` | 0.24 | fraction of adult body |

## Rules

Same as P1. Work in the `predation` worktree, commit explicit paths only, never merge to
main, follow the CPU limits in `design/handoffs/ecology-cleanup-2026-09-28.md` "Rules for
every package", and no sims or long runs. Check with `cargo nextest run -p <crate>` for
each crate touched. Use 60-min checks only if asked. Stop all background shells before
returning.
