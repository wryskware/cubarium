---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Codex comparison, before reading Fable's comparison

Compared [Fable's independent analysis](fable-independent-food-web-2026-09-15.md)
with [Codex's proposal](../ecological-niches-reconsideration-2026-09-15.md).
This document was completed before opening Fable's comparison. It is an independent
assessment of the two proposals, not a second blind ecological proposal: Codex
already knows its own prior work. Preserve this document; subsequent synthesis
belongs elsewhere.

## Recommendation

Combine Fable's attention to consumer/resource timing with a minimal version of
Codex's distinct food and digestive capabilities. Fable better specifies hypotheses
about the boom; Codex better addresses Wrysk's request for different ecological
roles. Neither proposal is a validated model or an implementation-ready contract.

| Issue | Assessment and recommended choice |
| --- | --- |
| Reproduction and food renewal | Fable makes this more concrete. Measure accessible net production, maintenance, maturation and reproductive surplus together. Treat slower reproduction as a candidate mechanism, not a proven stability fix. |
| Distinct diets | Keep Codex's separation of bodily capability from behavioral choice. Fable's efficiency/handling trade-offs could implement this economically, but weak fallback feeding must not support every nominal specialist indefinitely. |
| Litter versus carrion | Keep the distinction. Different consumers cannot reliably specialize on different substrates if the state erases their origins. Separate pools or equivalent composition bookkeeping are an implementation choice. |
| Plant recovery | Specify what protection means before selecting a curve. Reduced intake at low density is different from actual protected roots or structural reserves. Start with the least detailed model that supports the chosen behavior. |
| Local recycling | Fable's strongest addition: measure whether transport concentrates a subsidy and whether nutrient return is a bottleneck. Test transport and decomposition separately. A tenfold reduction is a candidate, not a conclusion. |
| Space | Use habitat differences and resource transport to create patches. Do not preserve fixed bands or add movement barriers simply to force asynchronous population curves. |
| Interdependence | Retain resource-mediated consequences and background decomposition. Defer explicit seed transport and detailed plant organs from the first implementation. |
| Predation | Include its food-access and budget requirements in the contract now. Validate it in sequence; neither an adult-only diet nor an indefinite predator deployment gate follows from the evidence. |

## Claims to qualify before adopting Fable's recommendations

1. **The diagnosed causes are hypotheses.** Historical twelve-hour counts, default
   parameters and the present live world are different evidence. Current intake,
   nutrient limitation and resource transport have not been isolated by the report.
   The newer live count of 82 is consistent with a die-off following the previously
   observed peak; it does not identify which niche survived or why.
2. **A few parameter values do not establish instability.** A stability analysis
   needs the coupled dynamics and operating state. Enrichment theory does not, by
   itself, locate Cubarium's current parameters on a stability boundary. A consumer
   doubling/recovery ratio above one is a useful experimental coordinate, not a
   sufficient general criterion for stability.
3. **A type III response is not an ungrazable residual.** `P²/(P²+K²)` stays positive
   for every positive `P`. It reduces low-density intake but provides no literal
   protected stock and does not itself guarantee recovery. Select a biological
   interpretation first; measure its consequence in the coupled system.
4. **“Concave” needs an equation.** Specify the full intake/assimilation budget,
   which quantities are being compared and the cost of breadth. The example
   efficiencies alone cannot prove segregation. A multimodal diet histogram is
   insufficient: those modes must exploit distinct resources and sustain distinct
   lineages under competition.
5. **The numerical population target is premature.** Gross plant production divided
   by nominal upkeep omits inaccessible food, other sinks, reproduction and unequal
   diets. It is at most a rough ceiling under stated assumptions. The proposal's
   150–250 consumers should not become a desired population.
6. **Several pass criteria encode taste rather than causation.** Burrowers below
   half of the population and cross-correlation below 0.5 have no demonstrated
   ecological threshold. Measure changes in food dependence, biomass, recovery and
   resource flows instead of declaring these values a pass.

The source checks were bounded: accessible research search material supports the
relevance of enrichment and spatial heterogeneity, but this review did not verify
all five cited papers in full. In particular, source access did not establish the
strong Noy-Meir/Huffaker formulations in Stage 1. Those claims should remain
qualified; they are not necessary to justify the proposed measurements.

## Changes I would make to Codex's own proposal

Prioritize budgets and actual resource dependence over adding ecological features.
Seed dispersal, microbial biomass and detailed plant structure are possible later
mechanisms, not prerequisites. Select one explicit dietary cost before adding
multiple organ, speed and maintenance penalties. Specify expected measurable
effects without promising every proposed interaction improves stability.

## Smallest next step

Write one food-web contract covering resource identity, who can obtain useful
nutrition from each resource, paid reproduction, and local resource renewal.
Choose whether low-stock plant protection represents difficult harvesting or
protected structure. Include a diet-by-food net-energy table and the accounting
for every conversion. Then measure one baseline and test one causal change at a
time with matched conditions and an explicit compute budget.

Separate success criteria: accounting correctness, actual niche differentiation,
recovery beyond the initial food subsidy, and visible purposeful behavior. A model
can pass one while failing the others. No numerical tuning should substitute for
the missing resource distinction the owner has asked us to consider.

No implementation, experiments, agents or live changes. Billed usage unavailable.
