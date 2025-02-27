"""This module contains the dialects for choosing different lowering strategies.

The dialects defined inside this module do not provide any new statements, it only
provide different lowering strategies for existing statements.
"""

from . import cf as cf, call as call, func as func, range as range
