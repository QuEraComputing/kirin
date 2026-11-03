"""Generate an interactive HTML view of a compiled Kirin kernel."""

from pathlib import Path

from kirin.prelude import basic


@basic
def add_one(x: int) -> int:
    return x + 1


if __name__ == "__main__":
    output = add_one.visualize(Path("add_one.ir.html"))
    print(f"Wrote interactive IR inspector to {output.resolve()}")

# In a Jupyter notebook, render the same inspector inline instead:
#
# from IPython.display import HTML, display
# from kirin import ir_to_html
#
# display(HTML(ir_to_html(add_one)))
