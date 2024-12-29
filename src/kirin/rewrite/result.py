from dataclasses import field, dataclass


@dataclass
class RewriteResult:
    terminated: bool = field(default=False, kw_only=True)
    has_done_something: bool = field(default=False, kw_only=True)
    exceeded_max_iter: bool = field(default=False, kw_only=True)

    def join(self, other: "RewriteResult") -> "RewriteResult":
        return RewriteResult(
            terminated=self.terminated or other.terminated,
            has_done_something=self.has_done_something or other.has_done_something,
            exceeded_max_iter=self.exceeded_max_iter or other.exceeded_max_iter,
        )
