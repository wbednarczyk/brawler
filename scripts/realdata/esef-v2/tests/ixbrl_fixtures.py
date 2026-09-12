"""Synthetic iXBRL document builder for the esef-v2 tooling tests. Not a
`test_*.py` file itself -- `unittest discover` skips it; the test modules
import it. Every fixture uses the synthetic issuer `ZZZ` (content-free)."""
from __future__ import annotations

import zipfile
from io import BytesIO

NS_ATTRS = (
    'xmlns="http://www.w3.org/1999/xhtml" '
    'xmlns:ix="http://www.xbrl.org/2013/inlineXBRL" '
    'xmlns:xbrli="http://www.xbrl.org/2003/instance" '
    'xmlns:xbrldi="http://xbrl.org/2006/xbrldi" '
    'xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2021-01-01/ifrs-full" '
    'xmlns:ixt="http://www.xbrl.org/inlineXBRL/transformation/2020-02-12"'
)


def make_instance(*, contexts: str = "", units: str = "", facts: str = "", lang: str = "pl") -> bytes:
    """A minimal, structurally-flat iXBRL document: contexts/units/facts are
    siblings under <body> since every parser lookup in `esef_ixbrl.py` walks
    the whole tree (`root.iter()`), not a fixed ESEF path -- so the wrapper
    boilerplate real filings use (ix:header/ix:resources) is not needed to
    exercise the parsing logic under test."""
    return f"""<html {NS_ATTRS} xml:lang="{lang}">
<head></head>
<body>
{contexts}
{units}
{facts}
</body>
</html>""".encode("utf-8")


def context(cid: str, *, start: str | None = None, end: str | None = None, instant: str | None = None,
            entity: str = "ZZZ", segment: str = "", scenario: str = "") -> str:
    if instant:
        period = f"<xbrli:instant>{instant}</xbrli:instant>"
    else:
        period = f"<xbrli:startDate>{start}</xbrli:startDate><xbrli:endDate>{end}</xbrli:endDate>"
    return f"""<xbrli:context id="{cid}">
  <xbrli:entity>
    <xbrli:identifier scheme="www.example.org">{entity}</xbrli:identifier>
    {segment}
  </xbrli:entity>
  <xbrli:period>{period}</xbrli:period>
  {scenario}
</xbrli:context>"""


def explicit_member(axis: str, member: str) -> str:
    return f'<xbrldi:explicitMember dimension="{axis}">{member}</xbrldi:explicitMember>'


def segment(*members_xml: str) -> str:
    return f"<xbrli:segment>{''.join(members_xml)}</xbrli:segment>"


def scenario(*members_xml: str) -> str:
    return f"<xbrli:scenario>{''.join(members_xml)}</xbrli:scenario>"


def unit(uid: str, measure: str = "iso4217:PLN") -> str:
    return f'<xbrli:unit id="{uid}"><xbrli:measure>{measure}</xbrli:measure></xbrli:unit>'


def divide_unit(uid: str, num: str, den: str) -> str:
    return (
        f'<xbrli:unit id="{uid}"><xbrli:divide>'
        f'<xbrli:unitNumerator><xbrli:measure>{num}</xbrli:measure></xbrli:unitNumerator>'
        f'<xbrli:unitDenominator><xbrli:measure>{den}</xbrli:measure></xbrli:unitDenominator>'
        f"</xbrli:divide></xbrli:unit>"
    )


def non_fraction(name: str, context_ref: str, unit_ref: str, text: str, *, decimals: str = "-3",
                  scale: str = "3", sign: str | None = None, fmt: str = "ixt:num-dot-decimal",
                  elem_id: str | None = None) -> str:
    sign_attr = f' sign="{sign}"' if sign else ""
    id_attr = f' id="{elem_id}"' if elem_id else ""
    return (
        f'<ix:nonFraction name="{name}" contextRef="{context_ref}" unitRef="{unit_ref}" '
        f'decimals="{decimals}" scale="{scale}" format="{fmt}"{sign_attr}{id_attr}>{text}</ix:nonFraction>'
    )


def zip_package(members: dict[str, bytes]) -> bytes:
    buf = BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        for name, data in members.items():
            z.writestr(name, data)
    return buf.getvalue()
