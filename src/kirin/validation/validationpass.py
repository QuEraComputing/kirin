from abc import ABC, abstractmethod
from typing import Any, Generic, TypeVar
from dataclasses import field, dataclass

from kirin import ir
from kirin.ir.exception import ValidationError

T = TypeVar("T")


class ValidationPass(ABC, Generic[T]):
    """Base class for a validation pass.

    Each pass analyzes an IR method and collects validation errors.
    """

    @abstractmethod
    def name(self) -> str:
        """Return the name of this validation pass."""
        ...

    @abstractmethod
    def run(self, method: ir.Method) -> tuple[Any, list[ValidationError]]:
        """Run validation and return (analysis_frame, errors).

        Returns:
            - analysis_frame: The result frame from the analysis
            - errors: List of validation errors (empty if valid)
        """
        ...

    def get_required_analyses(self) -> list[type]:
        """Return list of analysis classes this pass depends on.

        Override to declare dependencies (e.g., [AddressAnalysis, AnotherAnalysis]).
        The suite will run these analyses once and cache results.
        """
        return []

    def set_analysis_cache(self, cache: dict[type, Any]) -> None:
        """Receive cached analysis results from the suite.

        Override to store cached analysis frames/results.
        Example:
            self._address_frame = cache.get(AddressAnalysis)
        """
        pass


@dataclass
class ValidationSuite:
    """Compose multiple validation passes and run them together.

    Caches analysis results to avoid redundant computation when multiple
    validation passes depend on the same underlying analysis.

    Example:
        suite = ValidationSuite([
            NoCloningValidation,
            AnotherValidation,
        ])
        result = suite.validate(my_kernel)
        print(result.format_errors())
    """

    passes: list[type[ValidationPass]] = field(default_factory=list)
    fail_fast: bool = False
    _analysis_cache: dict[type, Any] = field(default_factory=dict, init=False)

    def add_pass(self, pass_cls: type[ValidationPass]) -> "ValidationSuite":
        """Add a validation pass to the suite."""
        self.passes.append(pass_cls)
        return self

    def validate(self, method: ir.Method) -> "ValidationResult":
        """Run all validation passes and collect results."""
        all_errors: dict[str, list[ValidationError]] = {}
        all_frames: dict[str, Any] = {}
        self._analysis_cache.clear()

        for pass_cls in self.passes:
            validator = pass_cls()
            pass_name = validator.name()

            try:
                required = validator.get_required_analyses()
                for required_analysis in required:
                    if required_analysis not in self._analysis_cache:
                        analysis = required_analysis(method.dialects)
                        analysis.initialize()
                        frame, _ = analysis.run(method)
                        self._analysis_cache[required_analysis] = frame

                validator.set_analysis_cache(self._analysis_cache)

                frame, errors = validator.run(method)
                all_frames[pass_name] = frame

                for err in errors:
                    if isinstance(err, ValidationError):
                        try:
                            err.attach(method)
                        except Exception:
                            pass

                if errors:
                    all_errors[pass_name] = errors
                    if self.fail_fast:
                        break

            except Exception as e:
                import traceback

                tb = traceback.format_exc()
                all_errors[pass_name] = [
                    ValidationError(
                        method.code, f"Validation pass '{pass_name}' failed: {e}\n{tb}"
                    )
                ]
                if self.fail_fast:
                    break

        return ValidationResult(all_errors, all_frames)


@dataclass
class ValidationResult:
    """Result of running a validation suite."""

    errors: dict[str, list[ValidationError]]
    frames: dict[str, Any] = field(default_factory=dict)
    is_valid: bool = field(default=True, init=False)

    def __post_init__(self):
        from bloqade.analysis.validation.nocloning.lattice import May, Must

        for _, frame in self.frames.items():
            if frame is None:
                continue
            for node, value in frame.entries.items():
                if isinstance(value, (Must, May)):
                    self.is_valid = False

    def error_count(self) -> int:
        """Total number of violations across all passes.

        Counts violations directly from frames using the same logic as test helpers.
        """
        from bloqade.analysis.validation.nocloning.lattice import May, Must

        total = 0
        for pass_name, frame in self.frames.items():
            if frame is None:
                continue

            for node, value in frame.entries.items():
                if isinstance(value, Must):
                    total += len(value.violations)
                elif isinstance(value, May):
                    total += len(value.violations)

        return total

    def get_frame(self, pass_name: str) -> Any:
        """Get the analysis frame for a specific pass."""
        return self.frames.get(pass_name)

    def format_errors(self) -> str:
        """Format all errors with their pass names."""
        if self.is_valid:
            return "\n\033[32mAll validation passes succeeded\033[0m"

        lines = [
            f"\n\033[31mValidation failed with {self.error_count()} violation(s):\033[0m"
        ]
        for pass_name, pass_errors in self.errors.items():
            lines.append(f"\n\033[31m{pass_name}:\033[0m")
            for err in pass_errors:
                err_msg = err.args[0] if err.args else str(err)
                lines.append(f"  - {err_msg}")
                if hasattr(err, "hint"):
                    hint = err.hint()
                    if hint:
                        lines.append(f"    {hint}")

        return "\n".join(lines)

    def raise_if_invalid(self):
        """Raise an exception if validation failed."""
        if not self.is_valid:
            first_errors = next(iter(self.errors.values()))
            raise first_errors[0]
