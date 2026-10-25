"""Render the fundamental EP/CF study from its Markdown source.

Requires reportlab and the `marked` Node module; see scripts/pdf/README.md.
"""

from __future__ import annotations

import html
import json
import os
import shutil
import subprocess
from pathlib import Path

from reportlab.lib import colors
from reportlab.lib.enums import TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle
from reportlab.lib.units import cm
from reportlab.platypus import (
    LongTable,
    PageBreak,
    Paragraph,
    Preformatted,
    SimpleDocTemplate,
    Spacer,
    TableStyle,
)


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "docs" / "VC_EP_CF_FUNDAMENTALS.md"
OUTPUT = ROOT / "output" / "pdf" / "vc_ep_cf_fundamentals.pdf"

INK = colors.HexColor("#17252A")
MUTED = colors.HexColor("#52636B")
ACCENT = colors.HexColor("#176B62")
LINE = colors.HexColor("#C9D4D5")
PALE = colors.HexColor("#EBF3F1")

STYLES = {
    "title": ParagraphStyle(
        "Title", fontName="Helvetica-Bold", fontSize=18, leading=22,
        textColor=INK, spaceAfter=12, keepWithNext=True,
    ),
    "h2": ParagraphStyle(
        "H2", fontName="Helvetica-Bold", fontSize=12, leading=15,
        textColor=ACCENT, spaceBefore=3, spaceAfter=8, keepWithNext=True,
    ),
    "h3": ParagraphStyle(
        "H3", fontName="Helvetica-Bold", fontSize=10, leading=13,
        textColor=INK, spaceBefore=8, spaceAfter=5, keepWithNext=True,
    ),
    "body": ParagraphStyle(
        "Body", fontName="Helvetica", fontSize=8.8, leading=12.4,
        textColor=INK, alignment=TA_LEFT, spaceAfter=7,
    ),
    "list": ParagraphStyle(
        "List", fontName="Helvetica", fontSize=8.6, leading=12,
        textColor=INK, leftIndent=13, firstLineIndent=-11, spaceAfter=4,
    ),
    "code": ParagraphStyle(
        "Code", fontName="Courier", fontSize=7.25, leading=9.0,
        textColor=INK, backColor=colors.HexColor("#F4F7F6"),
        borderColor=LINE, borderWidth=0.4, borderPadding=7,
        spaceBefore=4, spaceAfter=8,
    ),
    "table_head": ParagraphStyle(
        "TableHead", fontName="Helvetica-Bold", fontSize=7.0, leading=9,
        textColor=INK,
    ),
    "table": ParagraphStyle(
        "TableCell", fontName="Helvetica", fontSize=7.0, leading=9,
        textColor=INK,
    ),
}


def markdown_tokens() -> list[dict]:
    node = os.environ.get("PDF_NODE") or shutil.which("node")
    if not node:
        raise RuntimeError("Node.js is required to parse the Markdown source")
    env = os.environ.copy()
    local_modules = Path(__file__).resolve().parent / "node_modules"
    if not env.get("NODE_PATH") and local_modules.is_dir():
        env["NODE_PATH"] = str(local_modules)
    script = (
        'const fs=require("fs"); const {marked}=require("marked"); '
        'process.stdout.write(JSON.stringify(marked.lexer('
        'fs.readFileSync(process.argv[1],"utf8"))));'
    )
    result = subprocess.run(
        [node, "-e", script, str(SOURCE)],
        capture_output=True,
        text=True,
        check=True,
        env=env,
    )
    return json.loads(result.stdout)


def inline(tokens: list[dict]) -> str:
    pieces = []
    for token in tokens:
        kind = token["type"]
        children = token.get("tokens")
        if kind == "text":
            pieces.append(inline(children) if children else html.escape(token.get("text", "")))
        elif kind == "strong":
            pieces.append(f"<b>{inline(children or [])}</b>")
        elif kind == "em":
            pieces.append(f"<i>{inline(children or [])}</i>")
        elif kind == "codespan":
            pieces.append(
                f'<font name="Courier" size="7.5">'
                f'{html.escape(token.get("text", ""))}</font>'
            )
        elif kind == "link":
            label = inline(children or [])
            href = html.escape(token.get("href", ""), quote=True)
            pieces.append(f'<link href="{href}" color="#176B62">{label}</link>')
        elif kind == "br":
            pieces.append("<br/>")
        elif children:
            pieces.append(inline(children))
        else:
            pieces.append(html.escape(token.get("text", "")))
    return "".join(pieces)


def paragraph(token: dict, style: str = "body") -> Paragraph:
    return Paragraph(inline(token.get("tokens", [])), STYLES[style])


def make_table(token: dict, width: float) -> LongTable:
    headers = token["header"]
    rows = [headers, *token["rows"]]
    count = len(headers)
    if count == 4:
        col_widths = [0.22 * width, 0.26 * width, 0.26 * width, 0.26 * width]
    else:
        col_widths = [width / count] * count
    rendered = [
        [
            Paragraph(inline(cell.get("tokens", [])),
                      STYLES["table_head" if row_index == 0 else "table"])
            for cell in row
        ]
        for row_index, row in enumerate(rows)
    ]
    table = LongTable(rendered, colWidths=col_widths, repeatRows=1, hAlign="LEFT")
    table.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, 0), PALE),
        ("ROWBACKGROUNDS", (0, 1), (-1, -1), [colors.white, colors.HexColor("#F8FAF9")]),
        ("LINEBELOW", (0, 0), (-1, 0), 0.7, ACCENT),
        ("LINEBELOW", (0, 1), (-1, -1), 0.25, LINE),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("LEFTPADDING", (0, 0), (-1, -1), 5),
        ("RIGHTPADDING", (0, 0), (-1, -1), 5),
        ("TOPPADDING", (0, 0), (-1, -1), 5),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 5),
    ]))
    return table


def footer(canvas, doc):
    canvas.saveState()
    canvas.setStrokeColor(LINE)
    canvas.line(1.7 * cm, 1.25 * cm, A4[0] - 1.7 * cm, 1.25 * cm)
    canvas.setFont("Helvetica", 7.5)
    canvas.setFillColor(MUTED)
    canvas.drawString(1.7 * cm, 0.92 * cm, "EP vs CF | Fundamental VC study")
    canvas.drawRightString(A4[0] - 1.7 * cm, 0.92 * cm, str(doc.page))
    canvas.restoreState()


def build():
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    doc = SimpleDocTemplate(
        str(OUTPUT), pagesize=A4,
        leftMargin=1.7 * cm, rightMargin=1.7 * cm,
        topMargin=1.65 * cm, bottomMargin=1.6 * cm,
        title="Extended Pedersen vs Catalano-Fiore: Fundamental VC Study",
        author="ZKP AML Research",
        subject="Theoretical vector commitment comparison and pseudocode",
    )
    story = []
    width = A4[0] - 3.4 * cm
    for token in markdown_tokens():
        kind = token["type"]
        if kind == "heading":
            depth = token["depth"]
            style = "title" if depth == 1 else "h2" if depth == 2 else "h3"
            story.append(paragraph(token, style))
        elif kind == "paragraph":
            story.append(paragraph(token))
        elif kind == "code":
            story.append(Preformatted(token["text"], STYLES["code"], maxLineLength=95))
        elif kind == "table":
            story.append(make_table(token, width))
            story.append(Spacer(1, 0.2 * cm))
        elif kind == "list":
            for index, item in enumerate(token["items"]):
                marker = f'{token.get("start", 1) + index}.' if token["ordered"] else "-"
                story.append(Paragraph(f"{marker} {inline(item['tokens'])}", STYLES["list"]))
            story.append(Spacer(1, 0.15 * cm))
        elif kind == "hr":
            story.append(PageBreak())
        elif kind == "space":
            continue
        else:
            raise ValueError(f"Unsupported Markdown token: {kind}")
    doc.build(story, onFirstPage=footer, onLaterPages=footer)
    print(OUTPUT)


if __name__ == "__main__":
    build()
