"""Render recorded benchmark samples; requires matplotlib==3.10.0."""
import argparse
import json
from pathlib import Path
import statistics

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.lines import Line2D
from matplotlib.ticker import NullLocator


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    report = json.loads(args.results.read_text())
    families = [("chain", "Straight-line arithmetic"),
                ("diamond", "Branching"), ("nested_cfg", "Nested branching")]
    styles = {"rust": ("#2764A5", "-", "o", "Rust"),
              "python": ("#B87916", "--", "s", "Python")}
    plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 12,
                         "axes.labelcolor": "#333333", "text.color": "#222222",
                         "axes.edgecolor": "#BBBBBB", "savefig.facecolor": "white"})
    fig, axes = plt.subplots(1, 3, figsize=(14, 5.8), sharex=True, sharey=True)
    for ax, (family, title) in zip(axes, families):
        rows = sorted((r for r in report["results"] if r["family"] == family), key=lambda r: r["size"])
        assert [r["size"] for r in rows] == [1000, 10000, 100000]
        for implementation, (color, line, marker, _) in styles.items():
            medians, lows, highs = [], [], []
            for row in rows:
                samples = row[implementation]["samples_seconds"]
                assert len(samples) == 30 and row[implementation]["warmups"] == 5
                median = statistics.median(samples)
                assert abs(median - row[implementation]["median_seconds"]) < 1e-12
                q1, _, q3 = statistics.quantiles(samples, n=4, method="inclusive")
                medians.append(median * 1000)
                lows.append((median - q1) * 1000)
                highs.append((q3 - median) * 1000)
            ax.errorbar([r["size"] for r in rows], medians, yerr=[lows, highs],
                        color=color, linestyle=line, marker=marker, linewidth=2,
                        markersize=6, capsize=4, elinewidth=1.3)
        largest = rows[-1]
        ratio = largest["python"]["median_seconds"] / largest["rust"]["median_seconds"]
        ax.text(.96, .18, f"100k operations\n{ratio:.1f}× faster in Rust",
                ha="right", va="center", transform=ax.transAxes, fontsize=12)
        ax.set_title(title, fontsize=14, pad=14)
        ax.set_xscale("log")
        ax.set_yscale("log")
        ax.set_xlim(750, 135000)
        ax.set_ylim(.55, 10000)
        ax.set_xticks([1000, 10000, 100000], ["1,000", "10,000", "100,000"])
        ax.set_yticks([1, 10, 100, 1000, 10000], ["1", "10", "100", "1,000", "10,000"])
        ax.xaxis.set_minor_locator(NullLocator())
        ax.yaxis.set_minor_locator(NullLocator())
        ax.grid(axis="y", color="#E7E7E7", linewidth=.8)
        ax.set_axisbelow(True)
        ax.spines[["top", "right"]].set_visible(False)
        ax.set_xlabel("Arithmetic operations", labelpad=10)
    axes[0].set_ylabel("Round-trip time (ms, log scale)", labelpad=10)
    fig.suptitle("Kirin JSON round-trip latency", x=.06, y=.97, ha="left", fontsize=20)
    handles = [Line2D([0], [0], color=color, linestyle=line, marker=marker,
                      linewidth=2, label=label) for color, line, marker, label in styles.values()]
    fig.legend(handles=handles, loc="upper right", bbox_to_anchor=(.98, .98), ncol=2, frameon=False)
    fig.text(.06, .865, "Native IR → JSON → native IR. Medians of 30 trials after 5 warmups; error bars show the middle 50% of trials.", fontsize=11)
    fig.text(.06, .06, "Local ARM64 macOS run, 15 October 2026. Optimized Rust build; Python 3.13.5. Both axes use logarithmic spacing.", fontsize=10)
    fig.text(.06, .025, "Different native JSON formats; results apply to these representative payloads. Parsing, execution checks, I/O and final cleanup are not timed.", fontsize=10)
    fig.subplots_adjust(left=.08, right=.98, bottom=.22, top=.78, wspace=.10)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(args.output, dpi=180)
    plt.close(fig)


if __name__ == "__main__":
    main()
