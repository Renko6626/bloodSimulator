"""Independently check archived LBM blocks and render the research overview."""

import csv
import json
import math
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt


ROOT = Path(__file__).resolve().parent
RESULTS = ROOT / "results"
MAC = ROOT.parent / "2026-10-23-coupling" / "results"
CASES = ([(scene, n, 1.0, .25) for scene in ("straight", "channel", "blob4") for n in (32, 64)]
         + [(scene, 32, 2.0, .25) for scene in ("straight", "channel", "blob4")]
         + [("blob4", 64, 2.0, .25), ("blob4", 32, 1.0, 4.0)])
OPT_CASES = [(scene,32,tau,.25) for tau in (1.0,2.0) for scene in ("straight","channel","blob4")]
OPT_CASES += [("blob4",64,2.0,.25)]


def rows(path):
    with path.open(newline="") as stream:
        return list(csv.DictReader(stream))


def close(a, b, atol=1e-8, rtol=1e-7):
    if not math.isclose(a, b, abs_tol=atol, rel_tol=rtol):
        raise ValueError(f"mismatch: {a} versus {b}")


def slug(scene, n, tau, gamma):
    return f"{scene}_n{n}_tau{tau:g}_g{gamma:g}"


def field(scene, n, tau, gamma):
    data = rows(RESULTS / "fields" / f"{slug(scene, n, tau, gamma)}.csv")
    if len(data) != n*n:
        raise ValueError(f"incomplete field: {scene}, {n}, {tau}, {gamma}")
    return data


def field_coords_check(data, n):
    h = 1/n
    for i, r in enumerate(data):
        close(float(r["x"]), ((i % n)+.5)*h, atol=1e-11)
        close(float(r["y"]), ((i // n)+.5)*h, atol=1e-11)


def compare_blob(data, gamma, summary):
    mac_file = MAC / "fields" / f"field_blob_n32_m4_p{gamma:g}.csv"
    reference = rows(mac_file)
    if len(data) != 1024 or len(reference) != 1024:
        raise ValueError("blob field length mismatch")
    for a, b in zip(data, reference):
        close(float(a["x"]), float(b["x"]), atol=1e-11)
        close(float(a["y"]), float(b["y"]), atol=1e-11)
        if int(a["solid"]):
            raise ValueError("blob unexpectedly contains solid cells")
    vel_diff_sq = sum((float(a["u"])-float(b["u"]))**2+
                      (float(a["v"])-float(b["v"]))**2 for a,b in zip(data,reference))
    vel_ref_sq = sum(float(b["u"])**2+float(b["v"])**2 for b in reference)
    p_l = [float(a["p"]) for a in data]
    p_m = [float(b["p"]) for b in reference]
    ml = sum(p_l)/len(p_l)
    mm = sum(p_m)/len(p_m)
    pressure_diff_sq = sum(((a-ml)-(b-mm))**2 for a,b in zip(p_l,p_m))
    pressure_ref_sq = sum((b-mm)**2 for b in p_m)
    old = next(r for r in rows(MAC / "markers-summary.csv")
               if r["kind"]=="self" and r["shape"]=="blob" and int(r["n"])==32
               and int(r["m"])==4 and float(r["parameter"])==gamma)
    rx_l, rx_m = float(summary["reaction_x"]), float(old["reaction_x"])
    return {"mac_field": str(mac_file.relative_to(ROOT.parent)),
            "velocity_relative_l2": math.sqrt(vel_diff_sq/vel_ref_sq),
            "pressure_demeaned_relative_l2": math.sqrt(pressure_diff_sq/pressure_ref_sq),
            "lbm_reaction_x": rx_l, "mac_reaction_x": rx_m,
            "reaction_x_difference": rx_l-rx_m,
            "reaction_x_relative_difference": (rx_l-rx_m)/abs(rx_m),
            "lbm_force_law_velocity_relative_rms": float(summary["force_law_velocity_relative_rms"])}


def analyze_optimized(base_cases):
    directory = RESULTS / "optimized"
    if not (directory / "summary.csv").exists() and not (directory / "samples.csv").exists():
        return None
    if not (directory / "summary.csv").exists() or not (directory / "samples.csv").exists():
        raise ValueError("optimized archive has only one combined CSV")
    source = {r["case"]:r for r in rows(directory / "summary.csv")}
    expected = {slug(*spec) for spec in OPT_CASES}
    if set(source) != expected:
        raise ValueError("optimized case coverage differs from planned seven cases")
    sample_by_case = {}
    all_samples = rows(directory / "samples.csv")
    for r in all_samples:
        sample_by_case.setdefault(r["case"],[]).append(r)
    if len(all_samples) != 30*len(OPT_CASES) or set(sample_by_case) != expected:
        raise ValueError("optimized archive must contain seven complete 30-block cases")
    baseline_rows = {r["case"]:r for r in rows(RESULTS / "summary.csv")}
    result = {"definitions": {
        "scope":"fixed precomputed stream destinations; same equations and 1/60 model-time block",
        "p95":"nearest rank: sorted[ceil(0.95 * 30)-1]",
        "speed_ratio":"baseline mean / optimized mean for corresponding case; exploratory sequential CPU observations",
        "equivalence":"all non-timing summary columns and entire field CSV are byte identical to baseline",
    }, "cases":{}}
    for scene,n,tau,gamma in OPT_CASES:
        name = slug(scene,n,tau,gamma)
        r = source[name]
        b = baseline_rows[name]
        for key,value in r.items():
            if key == "case" or key.endswith("_ms") or key.startswith("post_ms") or key == "us_per_fluid_cell_step":
                continue
            if value != b[key]:
                raise ValueError(f"optimized numerical summary differs: {name}, {key}")
        if (directory / "fields" / f"{name}.csv").read_bytes() != (RESULTS / "fields" / f"{name}.csv").read_bytes():
            raise ValueError(f"optimized field differs: {name}")
        samples = sample_by_case[name]
        local = rows(directory / "blocks" / f"{name}.csv")
        if len(samples) != 30 or len(local) != 30:
            raise ValueError(f"optimized block count differs: {name}")
        dt = (tau-.5)/(3*n*n)
        steps = math.ceil((1/60)/dt)
        elapsed = []
        for i,(a,l) in enumerate(zip(samples,local),1):
            if any(a[k] != l[k] for k in ("block","steps","physical_time","elapsed_ms")):
                raise ValueError(f"optimized sample copies differ: {name}")
            if int(a["block"]) != i or int(a["steps"]) != steps:
                raise ValueError(f"optimized steps differ: {name}")
            close(float(a["physical_time"]),steps*dt,atol=1e-11)
            elapsed.append(float(a["elapsed_ms"]))
        mean = sum(elapsed)/len(elapsed)
        p95 = sorted(elapsed)[math.ceil(.95*len(elapsed))-1]
        close(mean,float(r["post_ms_mean"]),atol=1e-5)
        close(p95,float(r["post_ms_p95"]),atol=1e-5)
        close(float(r["post_physical_time"]),30*steps*dt,atol=1e-7)
        if int(r["post_steps"]) != 30*steps or int(r["post_block_steps"]) != steps:
            raise ValueError(f"optimized total steps differ: {name}")
        result["cases"][name] = {"scene":scene,"n":n,"tau":tau,"gamma":gamma,
                                 "blocks":30,"block_steps":steps,"block_physical_time":steps*dt,
                                 "post_ms_mean":mean,"post_ms_p95":p95,
                                 "baseline_post_ms_mean":base_cases[name]["post_ms_mean"],
                                 "baseline_post_ms_p95":base_cases[name]["post_ms_p95"],
                                 "baseline_mean_over_optimized_mean":base_cases[name]["post_ms_mean"]/mean,
                                 "numerical_summary_and_field_equal":True}
    return result


def analyze():
    if not (RESULTS / "summary.csv").exists() or not (RESULTS / "samples.csv").exists():
        raise RuntimeError("benchmark is incomplete; wait for the runner to finish")
    combined = {r["case"]: r for r in rows(RESULTS / "summary.csv")}
    all_samples = rows(RESULTS / "samples.csv")
    if len(combined) != len(CASES):
        raise ValueError("missing or duplicate summary cases")
    old_samples = {}
    for r in all_samples:
        old_samples.setdefault(r["case"], []).append(r)
    if set(old_samples) != set(combined):
        raise ValueError("sample case coverage differs from summary")
    output = {"definitions": {
        "block": "fixed 1/60 model second, frozen geometry, no Rapier",
        "post_p95": "nearest rank: sorted[ceil(0.95 * N)-1] with N=30",
        "velocity_relative_l2": "sqrt(sum over all cells and both components of (LBM-MAC)^2 / sum MAC^2)",
        "pressure_relative_l2": "same ratio after subtracting each field's own cell mean",
        "reaction_relative_difference": "(LBM reaction_x - MAC reaction_x)/abs(MAC reaction_x)",
        "pressure_caveat": "LBM c_s^2(rho-1) physical pressure versus MAC mean-zero Stokes pressure; both fields separately demeaned",
        "model_caveat": "LBM lattice-center Peskin stencil and finite-Re Navier-Stokes differ from MAC face stencil and steady Stokes",
        "timing_caveat": "post samples are one continuous frozen-geometry trajectory after one cold settle, not independent game frames",
    }, "cases": {}}
    if len(all_samples) != 30*len(CASES):
        raise ValueError(f"expected 330 block samples, found {len(all_samples)}")
    for scene,n,tau,gamma in CASES:
        name = slug(scene,n,tau,gamma)
        summary = combined[name]
        if summary["scene"] != scene or int(summary["n"]) != n or float(summary["tau"]) != tau or float(summary["gamma"]) != gamma:
            raise ValueError(f"summary case mismatch: {name}")
        if summary["converged"] != "true" or summary["unstable"] != "false":
            raise ValueError(f"not converged: {name}")
        sample = old_samples[name]
        case_sample = rows(RESULTS / "blocks" / f"{name}.csv")
        if len(sample) != 30 or len(case_sample) != 30:
            raise ValueError(f"expected 30 blocks: {name}")
        for a,b in zip(sample,case_sample):
            if any(a[k] != b[k] for k in ("block","steps","physical_time","elapsed_ms")):
                raise ValueError(f"combined samples differ: {name}")
        dt = (tau-.5)/(3*n*n)
        block_steps = math.ceil((1/60)/dt)
        block_time = block_steps*dt
        times = []
        for i,r in enumerate(sample,1):
            if int(r["block"]) != i or int(r["steps"]) != block_steps:
                raise ValueError(f"block indexing/steps incorrect: {name}")
            close(float(r["physical_time"]), block_time, atol=1e-11)
            times.append(float(r["elapsed_ms"]))
        mean = sum(times)/len(times)
        p95 = sorted(times)[math.ceil(.95*len(times))-1]
        close(float(summary["post_ms_mean"]), mean, atol=1e-5)
        close(float(summary["post_ms_p95"]), p95, atol=1e-5)
        if int(summary["post_block_steps"]) != block_steps or int(summary["post_steps"]) != 30*block_steps:
            raise ValueError(f"post steps inconsistent: {name}")
        close(float(summary["post_physical_time"]), 30*block_time, atol=1e-7)
        if abs(float(summary["mass_relative_error"])) >= 1e-7:
            raise ValueError(f"mass error too large: {name}")
        if scene == "blob4" and (abs(float(summary["force_balance_abs"])) >= 1e-9 or
                                 float(summary["force_law_velocity_relative_rms"]) >= 1e-5):
            raise ValueError(f"force check failed: {name}")
        data = field(scene,n,tau,gamma)
        field_coords_check(data,n)
        result = {"scene":scene,"n":n,"tau":tau,"gamma":gamma,"blocks":30,
                  "block_steps":block_steps,"block_physical_time":block_time,
                  "post_ms_mean":mean,"post_ms_p95":p95,
                  "settle_steps":int(summary["settle_steps"]),
                  "settle_physical_time":float(summary["settle_physical_time"]),
                  "solve_ms_single_observation":float(summary["solve_ms"]),
                  "mass_relative_error":float(summary["mass_relative_error"]),
                  "max_speed":float(summary["max_speed"])}
        if scene == "straight":
            h = 1/n
            truth = [float(summary["force_amplitude"])*((i//n)+.5)*h*(1-((i//n)+.5)*h)/2 for i in range(n*n)]
            err_sq = sum((float(r["u"])-t)**2+float(r["v"])**2 for r,t in zip(data,truth))
            ref_sq = sum(t*t for t in truth)
            flux = sum(float(data[j*n]["u"])*h for j in range(n))
            result["poiseuille_relative_l2"] = math.sqrt(err_sq/ref_sq)
            result["flux"] = flux
            result["flux_relative_error"] = flux/(float(summary["force_amplitude"])/12)-1
            close(result["poiseuille_relative_l2"],float(summary["poiseuille_relative_l2"]),atol=1e-6)
            close(flux,float(summary["flux"]),atol=1e-7)
        elif scene == "channel":
            h = 1/n
            widths = []
            true_widths = []
            fluxes = []
            for x in range(n):
                xx = (x+.5)*h
                low = .12+.04*math.sin(2*math.pi*xx)
                high = .88-.08*math.sin(2*math.pi*xx)
                true_widths.append(high-low)
                widths.append(h*sum(1-int(data[y*n+x]["solid"]) for y in range(n)))
                fluxes.append(h*sum(float(data[y*n+x]["u"]) for y in range(n)))
            result["effective_width_min"] = min(widths)
            result["effective_width_max"] = max(widths)
            result["true_width_min_at_columns"] = min(true_widths)
            result["true_width_max_at_columns"] = max(true_widths)
            result["effective_width_mae"] = sum(abs(a-b) for a,b in zip(widths,true_widths))/n
            result["effective_width_max_abs_error"] = max(abs(a-b) for a,b in zip(widths,true_widths))
            result["full_box_flux_min"] = min(fluxes)
            result["full_box_flux_max"] = max(fluxes)
        else:
            result["reaction_x"] = float(summary["reaction_x"])
            result["reaction_y"] = float(summary["reaction_y"])
            result["force_balance_abs"] = float(summary["force_balance_abs"])
            if n == 32:
                result["mac_comparison"] = compare_blob(data,gamma,summary)
        output["cases"][name] = result
    optimized = analyze_optimized(output["cases"])
    if optimized is not None:
        output["optimized"] = optimized
    return output


def plot(output):
    cases = output["cases"]
    navy, blue, orange, gold, grey = "#18364C", "#367DA2", "#CC693D", "#B48A34", "#66747E"
    plt.rcParams.update({"font.family":"DejaVu Sans","font.size":9,"axes.titlesize":11,
                         "axes.labelsize":9,"axes.edgecolor":grey,"text.color":navy,
                         "axes.labelcolor":navy,"xtick.color":navy,"ytick.color":navy,
                         "figure.facecolor":"white","axes.facecolor":"white","savefig.facecolor":"white"})
    fig,axs = plt.subplots(2,2,figsize=(13,8.5),layout="constrained")
    fig.suptitle("D2Q9 LBM research spike: fixed geometry and model-time update",size=15,weight="bold")
    ax = axs[0,0]
    order = [slug(s,n,t,g) for s,n,t,g in CASES]
    colors = {"straight":blue,"channel":gold,"blob4":orange}
    opt = output.get("optimized",{}).get("cases",{})
    for i,name in enumerate(order):
        r = cases[name]
        color = colors[r["scene"]]
        baseline_y = i-.13 if name in opt else i
        ax.plot([r["post_ms_mean"],r["post_ms_p95"]],[baseline_y,baseline_y],color=grey,lw=2,solid_capstyle="round")
        ax.scatter([r["post_ms_mean"]],[baseline_y],color="white",edgecolor=grey,s=28,zorder=3)
        if name in opt:
            optimized_y = i+.13
            o = opt[name]
            ax.plot([o["post_ms_mean"],o["post_ms_p95"]],[optimized_y,optimized_y],color=color,lw=2,solid_capstyle="round")
            ax.scatter([o["post_ms_mean"]],[optimized_y],color=color,s=28,zorder=3)
    ax.set_yticks(range(len(order)),[f'{cases[k]["scene"]}  {cases[k]["n"]}²  τ{cases[k]["tau"]:g}'+
                                     ("  Γ4" if cases[k]["gamma"]==4 else "") for k in order])
    ax.invert_yaxis()
    ax.set_xlabel("Mean (dot) to nearest-rank p95 (line), ms / block")
    ax.set_title("A  Fixed 1/60 model-time cost: baseline (open), fixed-link (filled)")
    ax.grid(axis="x",color="#DEE5E8",lw=.7)
    ax = axs[0,1]
    ax.plot([i/200 for i in range(201)],[.1*(i/200)*(1-i/200)/2 for i in range(201)],color=navy,lw=2,label="Poiseuille analytic")
    for n,tau,color,style in [(32,1,blue,"-"),(32,2,orange,"--"),(64,1,gold,"-")]:
        data = field("straight",n,tau,.25)
        ax.plot([float(data[y*n]["y"]) for y in range(n)],
                [float(data[y*n]["u"]) for y in range(n)],style,color=color,lw=1.8,marker="o",ms=2.5,
                label=f"LBM {n}² τ{tau:g}")
    ax.set(xlabel="y / H",ylabel="uₓ, physical",title="B  Straight channel velocity, fₓ=0.1")
    ax.legend(frameon=False,fontsize=8)
    ax.grid(color="#E8ECED",lw=.6)
    ax = axs[1,0]
    for i,gamma in enumerate((.25,4.0)):
        comp = cases[slug("blob4",32,1,gamma)]["mac_comparison"]
        ax.scatter([i-.12],[abs(comp["mac_reaction_x"])],color=navy,marker="D",s=45,
                   label="MAC Stokes" if i==0 else None)
        for j,tau in enumerate((1.0,2.0)):
            rr = cases[slug("blob4",32,tau,gamma)]["mac_comparison"] if slug("blob4",32,tau,gamma) in cases else None
            if rr:
                ax.scatter([i+.04+j*.14],[abs(rr["lbm_reaction_x"])],color=[blue,orange][j],s=45,
                           label=f"LBM τ{tau:g}" if i==0 else None)
    ax.set_xticks([0,1],["Γ=0.25","Γ=4"])
    ax.set(xlabel="Total four-point resistance",ylabel="|reactionₓ| per unit depth",
           title="C  Fixed four-point blob, n=32")
    ax.legend(frameon=False,fontsize=8,loc="center")
    ax.grid(axis="y",color="#E8ECED",lw=.6)
    ax = axs[1,1]
    n=32
    data = field("channel",n,1,.25)
    xx=[(x+.5)/n for x in range(n)]
    true=[.76-.12*math.sin(2*math.pi*x) for x in xx]
    effective=[sum(1-int(data[y*n+x]["solid"]) for y in range(n))/n for x in range(n)]
    ax.plot(xx,true,color=navy,lw=2,label="Geometric width")
    ax.step(xx,effective,where="mid",color=gold,lw=2,label="LBM open-site width")
    ax.set(xlabel="x / L",ylabel="Channel width",title="D  Curved-channel staircase, n=32")
    ax.legend(frameon=False,fontsize=8)
    ax.grid(color="#E8ECED",lw=.6)
    fig.savefig(RESULTS / "overview.png",dpi=180)
    fig.savefig(RESULTS / "overview.svg")
    plt.close(fig)


if __name__ == "__main__":
    result=analyze()
    (RESULTS / "analysis.json").write_text(json.dumps(result,indent=2,allow_nan=False)+"\n")
    plot(result)
    optimized_count = len(result.get("optimized",{}).get("cases",{}))
    print(f"checked {len(result['cases'])} baseline + {optimized_count} optimized cases / 30 blocks each; wrote analysis and overview")
