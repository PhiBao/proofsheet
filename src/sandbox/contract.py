"""Proofsheet execution contract.

This module is copied into /work for every run. Both the analyst script and the
independent checker import it, so ambiguous parsing (dates, money, quantities,
discounts) is resolved identically. The model is told not to reimplement it.

Keep this file dependency-free so it works in the same image as pandas.
"""

from __future__ import annotations

import datetime
from typing import Optional, Sequence

try:
    import pandas as _pd
except Exception:  # pragma: no cover
    _pd = None

DATE_FORMATS = {
    "YYYY-MM-DD": "%Y-%m-%d",
    "YYYY/MM/DD": "%Y/%m/%d",
    "ISO datetime": "%Y-%m-%dT%H:%M:%S",
    "Mon DD, YYYY": "%b %d, %Y",
    "Month DD, YYYY": "%B %d, %Y",
    "MM/DD/YYYY": "%m/%d/%Y",
    "DD-MM-YYYY": "%d-%m-%Y",
    "MM-DD-YYYY": "%m-%d-%Y",
    "DD/MM/YYYY": "%d/%m/%Y",
    "YYYY-MM-DD HH:MM:SS": "%Y-%m-%d %H:%M:%S",
    "MM/DD/YY": "%m/%d/%y",
    "DD/MM/YY": "%d/%m/%y",
}

try:
    from contract_config import DATE_PRECEDENCE as _DEFAULT_DATE_PRECEDENCE
    from contract_config import DISCOUNT_MODE as _DEFAULT_DISCOUNT_MODE
except Exception:  # pragma: no cover - the orchestrator always writes the config
    _DEFAULT_DATE_PRECEDENCE = []
    _DEFAULT_DISCOUNT_MODE = "percent_to_fraction"

DATE_PRECEDENCE = _DEFAULT_DATE_PRECEDENCE
DISCOUNT_MODE = _DEFAULT_DISCOUNT_MODE

EMPTY = {"", "nan", "none", "null", "n/a", "na", "-"}


def _s(value) -> str:
    if value is None:
        return ""
    return str(value).strip()


def _empty(value) -> bool:
    return _s(value).lower() in EMPTY


def parse_date(value, precedence: Optional[Sequence[str]] = None):
    """Parse using the approved label precedence. First matching format wins.

    `precedence` may be omitted (the run's approved config is used), a sequence of
    labels, or a single string such as "YYYY-MM-DD > MM/DD/YYYY".
    """
    s = _s(value)
    if _empty(s):
        return None
    if precedence is None:
        precedence = DATE_PRECEDENCE
    if isinstance(precedence, str):
        precedence = [p.strip() for p in precedence.split(">") if p.strip()]
    for label in precedence:
        fmt = DATE_FORMATS.get(label.strip())
        if not fmt:
            continue
        try:
            parsed = datetime.datetime.strptime(s, fmt)
            # Returning a pandas Timestamp keeps `.dt` accessors working when the
            # result is mapped over a Series. Fall back to a plain datetime.
            return _pd.Timestamp(parsed) if _pd is not None else parsed
        except ValueError:
            continue
    return None


def parse_number(value) -> Optional[float]:
    s = _s(value)
    if _empty(s):
        return None
    negative = s.startswith("(") and s.endswith(")")
    if negative:
        s = s[1:-1]
    for symbol in ("$", "€", "£", "¥", "₹"):
        s = s.replace(symbol, "")
    s = s.replace(",", "").replace(" ", "").replace("\u00a0", "")
    if s.endswith("%"):
        s = s[:-1]
    try:
        number = float(s)
    except ValueError:
        return None
    if number != number or number in (float("inf"), float("-inf")):
        return None
    return -number if negative else number


def parse_money(value) -> Optional[float]:
    return parse_number(value)


def parse_quantity(value) -> Optional[float]:
    s = _s(value)
    if _empty(s):
        return None
    try:
        return float(s)
    except ValueError:
        return None


def parse_discount(value, mode: Optional[str] = None) -> float:
    """Percent text becomes a fraction. Plain decimals are already fractions."""
    if mode is None:
        mode = DISCOUNT_MODE
    s = _s(value)
    if _empty(s):
        return 0.0
    if s.endswith("%"):
        try:
            return float(s[:-1].strip()) / 100.0
        except ValueError:
            return 0.0
    try:
        number = float(s)
    except ValueError:
        return 0.0
    if mode == "as_decimal" and abs(number) > 1.0:
        return number / 100.0
    return number


def normalize_status(value) -> str:
    return _s(value).lower()


def normalize_country(value) -> str:
    return _s(value).rstrip(".").strip().title()


def normalize_text(value) -> str:
    return " ".join(_s(value).split())


def rows_used(df) -> int:
    return int(len(df))
