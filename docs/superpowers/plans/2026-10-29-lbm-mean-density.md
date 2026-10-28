# Frame-end mean-density candidate plan

**Approved scope:** one same-device1000-frame paired `fork_a Fine` baseline/control experiment, unchanged prescribed velocities/70:30 split, MRT+Guo/LSQ-FD/nu=.05/h=.25/dt=.0011904762. Independent research copy only. Main agent owns cloud/SSH/run/retrieve/stop; worker owns code, <=3-frame local smoke and analysis. No production default, parameter scan or extra long run.

## Definition and timing

After each14-step game frame, read controlled raw populations and compute meanrho in CPU f64 over fluid cells. Let `delta=meanrho-1`, upload one f32 scalar. Check every `rho-delta>0`; otherwise stop. One GPU pass applies the same spatial density offset to each fluid cell:

`gnew = g + feq(rho-delta,u)-feq(rho,u) = g-delta*w*(1+3cu+4.5cu²-1.5u²)`.

Use original `u=(j+F/2)/rho`, keep F fixed. Ideal moments: rho'=rho-delta, j'=j-delta*u, so physical u is unchanged, spatial density differences remain, and f-feq(rho,u) is unchanged relative to the new equilibrium. Guo-corrected non-equilibrium second moment `Σccf-rho*u*u-cs²rho*I+(uF+Fu)/2` is also preserved when u/F are unchanged. This is a separate frame-end experiment retaining MRT, **not** an implementation of OpenLB's BGK statistics scheduling.

OpenLB Latt2008 TR3 and current constRhoBgkCollision/defineNEq motivate mean-offset/equilibrium-difference handling; they do not validate this MRT/Guo/LSQ adaptation. Sources supplied and checked by the main agent: https://www.openlb.net/wp-content/uploads/2011/12/olb-tr3.pdf and https://www.openlb.net/DoxyGen/html/d3/d3f/structolb_1_1lbm.html . Direct web re-open failed in this worker; no additional literature claims are inferred.

## Observable checks / predeclared limits

- Independent CPU raw moments before/after **each actual GPU correction**, not the host assigneddelta: mean residual≤5e-7; max per-cell `abs((rho_after-rho_before)+delta_f32)`≤5e-7; max physical lattice-velocity change≤5e-7; max non-equilibrium stress-component change≤5e-7 (density*lattice velocity²). A single nonzero-Guo-force moment-only correction starts at rho=1.02 and explicitly writes latticeF=[.02,-.01] to theGPU uniform (no fluid step), then checks the same limits, without fluid advancement or a coupled-force acceptance claim.
- Stop on nonpositive/nonfinite rho/u, nonpositive intendedrho, or violation of these local transformation limits. No clamp/retry/fallback or tolerance relaxation.
- Baseline and control both fresh initialization, same GPU, pulse, timestep and per-step readbacks; both check valid fields every step. Record actual removed mass `Mpre-Mpost`, solver increment `Mpre-Mpreviouspost`, cumulative compensation and budget closure. Mean near1 is **designed behavior**, not evidence mass-injection/pressure errors are solved.
- Compare matched frame-end global and central16×16-block velocity, separately demeanedrho (pressure-variation proxy), and non-equilibrium stress fields. Exploratory guardrails: velocity relativeL2≤5%, demeanedrho relativeL2≤5%, stress relativeL2≤10%; report absoluteRMS/max too. These assess departure from a mean-offset-only effect, not physical accuracy. Exceeding a comparison guardrail is reported as a candidate limitation while the predeclared1000-frame run continues if all state/local checks remain valid.
- Higher-order evolution stress may change despite instantaneous residual preservation. Baseline drift is not a physical ground truth. No migration/entity-coupling or larger density-removal envelope is claimed.

## Files / execution

New `docs/research/2026-10-29-lbm-mean-density/`: copiedLSQ module + one normalization uniform/pass, raw-moment observer and one paired harness, build/analysis scripts, frameCSV, one force-checkCSV and three finalraw snapshots only. Reuse existing geometryweight; no GPU reduction, new dependencies or permanent tests.

- [x] Implement/format/freshbuild and <=3-frame local smoke, then freeze source/binary/weights SHA for main-agent hardware run.
- [x] Run paired1000 frames once on hardware; stop if invalid, no extra cases or altered limits.
- [x] Retrieve, verify actual compensation/moments/paired-field changes, confirm stopped instance, archive short report and commit experiment only.

Execution: local2-frame paired smoke and one nonzero-force moment-only check passed. Frozen binary SHA2c451ec2b3090af4db77f98184a8c26fb20bde76c3a3fc28195d97032ca7351e ran the1000-frame pair once onRTX3060. All predeclared local/comparison checks passed; mean control also removes global periodic mean-pressure variation, which the demeanedrho comparison excludes. Actual compensation and small evolving-field differences reported without claiming physical mass conservation. Main agent retrieved results and confirmed stopped/exited.
