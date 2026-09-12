#!/usr/bin/env python3
"""Shared inline-XBRL (iXBRL) parsing for ESEF measurement v2 (#331 PR-A,
ADR 0112). Stdlib only (`zipfile`, `xml.etree.ElementTree`, `decimal`).

Parses a loose `.xhtml` file or an ESEF report package (ZIP) into raw
occurrence rows per the shared data contract (`occurrences_v2.json`). Emits
every fact from every eligible member; never dedupes or filters by concept.
The private v1 reference labeler
(`private/realdata/spikes/esef-positional-gt/label_esef.py`) picked the
first ZIP member (`:259`), collapsed duplicate (concept, context) pairs
before comparing values (`:385`), looked up `segment` directly under
`context` instead of under `entity` (`:200`), and filtered to its concept
map before emitting (`:331`) -- silently losing conflicting evidence. This
module fixes all four; conflict detection over the full occurrence set is
the caller's job (`label_esef_v2.py`).
"""
from __future__ import annotations

import zipfile
from decimal import Decimal, InvalidOperation
from io import BytesIO
import xml.etree.ElementTree as ET

XML_NS = "{http://www.w3.org/XML/1998/namespace}"


def local_name(tag: str) -> str:
    """Strip a Clark-notation namespace ('{uri}local') down to the local name."""
    return tag.split("}", 1)[1] if tag.startswith("{") else tag


def is_zip(data: bytes) -> bool:
    return zipfile.is_zipfile(BytesIO(data))


def zip_members(data: bytes) -> list[tuple[str, bytes]]:
    """Every `.xhtml`/`.html` member of a ZIP package, sorted by member name
    for deterministic processing order (selection.py determinism depends on
    this, not on ZIP directory order)."""
    with zipfile.ZipFile(BytesIO(data)) as z:
        names = sorted(n for n in z.namelist() if n.lower().endswith((".xhtml", ".html")))
        return [(n, z.read(n)) for n in names]


def sniff_ixbrl(data: bytes) -> bool:
    """True if `data` parses as XML and carries at least one iXBRL context
    and one numeric/non-numeric fact (`ix:` namespace + `xbrli:context`)."""
    try:
        root = ET.fromstring(data)
    except ET.ParseError:
        return False
    tags = {local_name(e.tag) for e in root.iter()}
    return "context" in tags and ("nonFraction" in tags or "nonNumeric" in tags)


def namespace_map(data: bytes) -> dict[str, str]:
    """prefix -> namespace URI, collected from every `xmlns:` declaration in
    the document (root and nested). v1 split lexical QNames blindly with no
    verification the prefix was ever bound; this map lets a caller confirm a
    concept's prefix resolves to a real namespace. Last declaration for a
    given prefix wins -- documents that re-bind a prefix mid-tree are not
    expected in this corpus."""
    nsmap: dict[str, str] = {}
    for _, (prefix, uri) in ET.iterparse(BytesIO(data), events=("start-ns",)):
        nsmap[prefix or ""] = uri
    return nsmap


def qname_local(qname_attr: str, nsmap: dict[str, str]) -> tuple[str, bool]:
    """(local_name, prefix_resolved). `local_name` is always the text after
    the first ':'; `prefix_resolved` is False when the prefix carries no
    bound namespace in `nsmap` -- a filing defect. The caller decides what
    to do with an unresolved prefix (this module still emits the fact)."""
    if ":" not in qname_attr:
        return qname_attr, "" in nsmap
    prefix, local = qname_attr.split(":", 1)
    return local, prefix in nsmap


def expand_qname(qname_attr: str, nsmap: dict[str, str]) -> tuple[str, bool]:
    """(expanded_qname, resolved). Amendment M (#331 astra r1 finding 11): the
    stored `concept_qname` must be the EXPANDED Clark-notation form
    `{namespace-uri}Local`, never the lexical `prefix:Local` -- a lexical
    qname lets an unresolved or extension-taxonomy prefix silently pass as
    whatever the local name happens to spell (e.g. a company's own custom
    `Revenue` concept reading as IFRS `Revenue`). When the prefix has no
    bound namespace, `resolved` is False and the expanded form falls back to
    the lexical qname (still returned, so the fact stays evidence, but never
    mistaken for a resolved identity)."""
    local, resolved = qname_local(qname_attr, nsmap)
    if not resolved:
        return qname_attr, False
    prefix = qname_attr.split(":", 1)[0] if ":" in qname_attr else ""
    return f"{{{nsmap[prefix]}}}{local}", True


def _child(elem, tag):
    return next((c for c in elem if local_name(c.tag) == tag), None)


def collect_value_text(elem) -> str:
    """Fact text with `ix:exclude` subtrees dropped (ESEF spec: excluded
    content is not part of the numeric value)."""
    parts = [elem.text or ""]
    for child in list(elem):
        if local_name(child.tag) != "exclude":
            parts.append(collect_value_text(child))
        parts.append(child.tail or "")
    return "".join(parts)


_WS_CHARS = " \t\r\n\xa0"


def apply_transform(raw_text: str, format_attr: str | None) -> tuple[Decimal | None, str | None]:
    """Applies the ixt transform named by `format_attr` to `raw_text`.
    Returns (decimal_or_None, note); `note` is set whenever the value could
    not be cleanly parsed (unknown transform, unparsable text) so the caller
    can mark `parse_status: "unparsed"` without silently swallowing it.
    Supports `num-dot-decimal`, `num-comma-decimal`, `zerodash`/`fixed-zero`
    (namespace/version prefix on `format_attr` ignored)."""
    fmt_local = format_attr.split(":", 1)[1] if format_attr and ":" in format_attr else format_attr
    if fmt_local in ("fixed-zero", "zerodash"):
        return Decimal(0), None
    text = (raw_text or "").strip()
    if text in ("", "-", "–", "—"):
        return None, "empty value"
    cleaned = "".join(ch for ch in text if ch not in _WS_CHARS)
    if fmt_local == "num-dot-decimal":
        cleaned = cleaned.replace(",", "")
    elif fmt_local == "num-comma-decimal":
        cleaned = cleaned.replace(".", "").replace(",", ".")
    elif fmt_local in (None, ""):
        pass
    else:
        return None, f"unrecognized format {format_attr!r}"
    try:
        return Decimal(cleaned), None
    except InvalidOperation:
        return None, f"unparsable numeric text {raw_text!r}"


def parse_contexts(root) -> dict[str, dict]:
    """context id -> {entity_identifier, period, dimensions}. Dimensions
    come from BOTH `entity/segment` (v1 bug: looked directly under
    `context` -- fixed here) and `scenario`."""
    contexts: dict[str, dict] = {}
    for ctx in root.iter():
        if local_name(ctx.tag) != "context":
            continue
        cid = ctx.get("id")
        if cid is None:
            continue

        entity_el = _child(ctx, "entity")
        entity_identifier = None
        segment_el = None
        if entity_el is not None:
            ident_el = _child(entity_el, "identifier")
            entity_identifier = (ident_el.text or "").strip() if ident_el is not None else None
            segment_el = _child(entity_el, "segment")
        scenario_el = _child(ctx, "scenario")

        period = None
        period_el = _child(ctx, "period")
        if period_el is not None:
            instant_el = _child(period_el, "instant")
            start_el = _child(period_el, "startDate")
            end_el = _child(period_el, "endDate")
            if instant_el is not None:
                period = {"instant": (instant_el.text or "").strip()}
            elif start_el is not None and end_el is not None:
                period = {"start": (start_el.text or "").strip(), "end": (end_el.text or "").strip()}

        dimensions = []
        for holder in (segment_el, scenario_el):
            if holder is None:
                continue
            for member in holder.iter():
                if local_name(member.tag) == "explicitMember":
                    dimensions.append({"axis": member.get("dimension"), "member": (member.text or "").strip()})
                elif local_name(member.tag) == "typedMember":
                    dimensions.append({"axis": member.get("dimension"), "member": "".join(member.itertext()).strip()})

        contexts[cid] = {"entity_identifier": entity_identifier, "period": period, "dimensions": dimensions}
    return contexts


def parse_units(root) -> dict[str, str]:
    """unit id -> the raw measure text as declared (e.g. 'iso4217:PLN'),
    literal per the shared contract -- never stripped to a bare currency
    code here (`label_esef_v2.py` derives the ISO code for `currency`)."""
    units: dict[str, str] = {}
    for u in root.iter():
        if local_name(u.tag) != "unit":
            continue
        uid = u.get("id")
        if uid is None:
            continue
        measure_el = _child(u, "measure")
        divide_el = _child(u, "divide")
        if measure_el is not None:
            units[uid] = (measure_el.text or "").strip()
        elif divide_el is not None:
            num_el = _child(divide_el, "unitNumerator")
            den_el = _child(divide_el, "unitDenominator")
            num_measure = _child(num_el, "measure") if num_el is not None else None
            den_measure = _child(den_el, "measure") if den_el is not None else None
            num = (num_measure.text or "").strip() if num_measure is not None else ""
            den = (den_measure.text or "").strip() if den_measure is not None else ""
            units[uid] = f"{num}/{den}"
    return units


def document_lang(root) -> str | None:
    """`xml:lang` per instance: the root's declaration, else the first
    descendant that carries one."""
    lang = root.get(f"{XML_NS}lang")
    if lang:
        return lang
    for elem in root.iter():
        lang = elem.get(f"{XML_NS}lang")
        if lang:
            return lang
    return None


def parse_instance(data: bytes, package_member: str | None = None) -> dict:
    """Parses one iXBRL instance document.

    Returns {"lang": str|None, "occurrences": [row, ...], "parse_error": str|None}.
    A fatal XML parse failure returns no occurrences and sets `parse_error`;
    a per-fact problem (bad transform, unresolved context/unit, bad scale)
    still emits the occurrence row with `parse_status: "unparsed"` -- it
    stays evidence rather than vanishing.
    """
    try:
        root = ET.fromstring(data)
    except ET.ParseError as exc:
        return {"lang": None, "occurrences": [], "parse_error": str(exc)}

    nsmap = namespace_map(data)
    contexts = parse_contexts(root)
    units = parse_units(root)
    lang = document_lang(root)

    occurrences = []
    for elem in root.iter():
        if local_name(elem.tag) != "nonFraction":
            continue
        name_attr = elem.get("name")
        context_id = elem.get("contextRef")
        if not name_attr or context_id is None:
            continue  # not a usable fact -- no concept or no context to anchor it

        unit_id = elem.get("unitRef")
        decimals_attr = elem.get("decimals")
        sign_attr = elem.get("sign") if elem.get("sign") == "-" else None
        format_attr = elem.get("format")
        nil = (elem.get("nil") or "").lower() == "true"

        scale_ok = True
        try:
            scale = int(elem.get("scale", "0"))
        except ValueError:
            scale, scale_ok = 0, False

        concept_local, prefix_resolved = qname_local(name_attr, nsmap)
        expanded_qname, _expanded_resolved = expand_qname(name_attr, nsmap)
        ctx = contexts.get(context_id)
        # Amendment U / astra r2 finding 11: a NUMERIC fact with NO unitRef
        # at all is not "resolved" -- ix:nonFraction requires one per spec.
        # The old check (`unit_id is None or ...`) treated a missing unitRef
        # as trivially satisfied, letting a unitless fact read "ok" and land
        # in a slot with a null currency.
        unit_resolved = unit_id is not None and units.get(unit_id) is not None
        raw_text = "" if nil else collect_value_text(elem)
        decimal_value, _note = (Decimal(0), None) if nil else apply_transform(raw_text, format_attr)

        value = None
        if decimal_value is not None:
            signed = -decimal_value if sign_attr else decimal_value
            value = format(signed * (Decimal(10) ** scale), "f")

        parse_status = (
            "ok"
            if (ctx is not None and decimal_value is not None and scale_ok and prefix_resolved and unit_resolved)
            else "unparsed"
        )

        occurrences.append(
            {
                "package_member": package_member,
                "entity_identifier": ctx["entity_identifier"] if ctx else None,
                "concept_qname": expanded_qname,
                "concept_local": concept_local,
                "context_id": context_id,
                "period": ctx["period"] if ctx else None,
                "unit": units.get(unit_id) if unit_id else None,
                "decimals": decimals_attr,
                "scale": scale,
                "sign": sign_attr,
                "format": format_attr,
                "dimensions": ctx["dimensions"] if ctx else [],
                "value": value,
                "raw_text": raw_text,
                "parse_status": parse_status,
            }
        )
    return {"lang": lang, "occurrences": occurrences, "parse_error": None}


def parse_document(data: bytes) -> dict:
    """Top-level entry point: handles a loose file or a ZIP package.

    Returns {"instances": [{"package_member":.., "lang":.., "occurrences":[...],
    "parse_error":..}, ...], "non_ixbrl_members": int, "total_members": int}.
    For a ZIP, EVERY `.xhtml`/`.html` member is sniffed; only iXBRL-carrying
    members are parsed into instances (an auditor's opinion or an ESG report
    shipped in the same package is not an instance)."""
    if is_zip(data):
        members = zip_members(data)
        instances = []
        non_ixbrl = 0
        for name, member_bytes in members:
            if sniff_ixbrl(member_bytes):
                instances.append({"package_member": name, **parse_instance(member_bytes, name)})
            else:
                non_ixbrl += 1
        return {"instances": instances, "non_ixbrl_members": non_ixbrl, "total_members": len(members)}

    if not sniff_ixbrl(data):
        return {"instances": [], "non_ixbrl_members": 1, "total_members": 1}
    return {
        "instances": [{"package_member": None, **parse_instance(data, None)}],
        "non_ixbrl_members": 0,
        "total_members": 1,
    }
