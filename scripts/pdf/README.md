# PDF generators

Run these commands from the repository root. Python 3.10+ and Node.js 20+
are needed to regenerate both documents. The committed PDFs can be read
without installing these dependencies.

```bash
python3 -m venv .venv-pdf
.venv-pdf/bin/python -m pip install -r scripts/pdf/requirements.txt
npm ci --prefix scripts/pdf --ignore-scripts --no-audit --no-fund
.venv-pdf/bin/python scripts/pdf/generate_set_membership_pdf.py
.venv-pdf/bin/python scripts/pdf/generate_vc_ep_cf_fundamentals.py
```

`requirements.txt` and the npm lockfile pin the dependencies used by these
generators. The Node dependencies are local to this directory. `PDF_NODE` can
select another Node executable, and `NODE_PATH` can select an existing installation
of `marked`; neither override is required for the commands above.

| Generator | Source | Output |
| --- | --- | --- |
| `generate_set_membership_pdf.py` | Protocol text embedded in the generator, paired with `docs/SET_MEMBERSHIP_FORMALIZATION.md` | `output/pdf/set_membership_formalization.pdf` |
| `generate_vc_ep_cf_fundamentals.py` | `docs/VC_EP_CF_FUNDAMENTALS.md` | `output/pdf/vc_ep_cf_fundamentals.pdf` |

The first generator does not parse its Markdown counterpart. Update both sources
when revising its protocol. These documents describe membership specifications
and the fundamental VC study; the E1 implementation status is documented separately
in `docs/E1_IMPLEMENTATION.md`.

After regeneration, inspect all pages, especially formulas and dense tables.
With Poppler installed, `pdfinfo`, `pdftotext`, and `pdftoppm` can be used for
text and layout checks. Store temporary page renders under `tmp/`; do not commit
them. Regeneration overwrites the two final PDFs.
