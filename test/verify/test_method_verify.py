import pytest

from kirin.prelude import basic


def test_verification(capsys):
    @basic
    def my_kernel(arg1, arg2):
        return arg1 + arg2

    with pytest.raises(Exception):

        @basic
        def main():
            my_kernel(5)  # type: ignore

        captured = capsys.readouterr()
        assert "test/verify/test_method_verify.py" in captured.err
        assert "line 11" in captured.err
        assert (
            "Verification failed for main: expected 3 arguments, got 1" in captured.err
        )
