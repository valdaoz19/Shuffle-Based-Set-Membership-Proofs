const API = '';

/** Cached demo setup from last scenario selection (avoid reset on every verify). */
let activeSetup = null;
let latestResult = null;

/** Fallback when API/server belum di-rebuild (field skenario baru belum ada). */
const SCENARIO_DEFAULTS = {
  compliant: {
    scenario: 'compliant',
    user_key: 'scenario_compliant',
    history_cents: 1_200_000,
    suggested_amount_cents: 200_000,
    projected_total_cents: 1_400_000,
    expected_status: 'APPROVED',
  },
  exceeded: {
    scenario: 'exceeded',
    user_key: 'scenario_exceeded',
    history_cents: 1_900_000,
    suggested_amount_cents: 500_000,
    projected_total_cents: 2_400_000,
    expected_status: 'REJECTED',
  },
};

function formatSgd(cents) {
  if (cents == null || Number.isNaN(Number(cents))) return '—';
  return `SGD ${(Number(cents) / 100).toFixed(2)}`;
}

function setText(id, text) {
  const el = document.getElementById(id);
  if (el) el.textContent = text;
}

function apiHeaders(extra = {}) {
  const key = localStorage.getItem('aml_api_key') || '';
  return key
    ? { ...extra, 'x-api-key': key }
    : extra;
}

async function fetchJson(path, options = {}) {
  const res = await fetch(`${API}${path}`, {
    ...options,
    headers: apiHeaders(options.headers ?? {}),
  });
  const data = await res.json();
  if (!res.ok) {
    throw new Error(data.error ?? 'Request failed');
  }
  return data;
}

function normalizeSetup(data, scenarioId) {
  const id = data.scenario ?? scenarioId ?? 'compliant';
  const defaults = SCENARIO_DEFAULTS[id] ?? SCENARIO_DEFAULTS.compliant;
  const history = data.history_cents ?? defaults.history_cents;
  const suggested = data.suggested_amount_cents ?? defaults.suggested_amount_cents;

  return {
    ...defaults,
    ...data,
    scenario: id,
    user_key: data.user_key ?? defaults.user_key,
    history_cents: history,
    suggested_amount_cents: suggested,
    projected_total_cents:
      data.projected_total_cents ?? history + suggested,
    expected_status: data.expected_status ?? defaults.expected_status,
    message:
      data.message ??
      `Skenario '${id}' siap. Hasil diharapkan: ${defaults.expected_status}.`,
  };
}

async function loadConfig() {
  const res = await fetch(`${API}/api/v1/config`);
  const cfg = await res.json();
  setText('cfg-threshold', cfg.threshold_display);
  setText('cfg-window', `${cfg.window_days} days`);
  setText('cfg-max', `SGD ${(cfg.max_tx_cents / 100).toFixed(2)}`);
  setText('cfg-api-key', cfg.api_key_required ? 'Required' : 'Disabled');
}

function formatCommitment(txCommitment) {
  return typeof txCommitment === 'string'
    ? txCommitment
    : JSON.stringify(txCommitment);
}

function renderSetup(data, scenarioId = 'compliant') {
  const setup = normalizeSetup(data, scenarioId);
  activeSetup = setup;

  setText('tx-commitment', formatCommitment(setup.tx_id_commitment));
  setText('scenario-summary', setup.message);
  setText(
    'scenario-history',
    `Riwayat dalam window: ${formatSgd(setup.history_cents)}`,
  );
  setText(
    'scenario-projected',
    `Total proyeksi (riwayat + transaksi baru): ${formatSgd(setup.projected_total_cents)}`,
  );

  const expectedBadge = document.getElementById('expected-badge');
  if (expectedBadge) {
    expectedBadge.textContent = setup.expected_status;
    expectedBadge.className = `px-2 py-0.5 rounded text-xs font-semibold ${
      setup.expected_status === 'APPROVED'
        ? 'bg-emerald-900/60 text-emerald-300'
        : 'bg-rose-900/60 text-rose-300'
    }`;
  }

  const amountInput = document.getElementById('amount');
  if (amountInput) {
    amountInput.value = (setup.suggested_amount_cents / 100).toFixed(2);
  }
}

async function loadDemoSetup(scenarioId) {
  const res = await fetch(`${API}/api/v1/demo/setup`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ scenario: scenarioId }),
  });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error ?? 'Demo setup failed');
  }
  const data = await res.json();
  renderSetup(data, scenarioId);
  return activeSetup;
}

function centsFromInput(value) {
  return Math.round(parseFloat(value) * 100);
}

function renderResult(result) {
  latestResult = result;
  const panel = document.getElementById('result-panel');
  const approved = result.status === 'APPROVED';

  panel.classList.remove('hidden', 'border-emerald-800', 'border-rose-800');
  panel.classList.add(approved ? 'border-emerald-800' : 'border-rose-800');

  const badge = document.getElementById('decision-badge');
  badge.textContent = result.status;
  badge.className = `px-4 py-1.5 rounded-full text-sm font-bold tracking-wide ${
    approved ? 'bg-emerald-900 text-emerald-300' : 'bg-rose-900 text-rose-300'
  }`;

  setText(
    'exec-time',
    `ZK-MNNP verification: ${result.execution_time_ms} ms`,
  );
  setText('result-message', result.message);
  document.getElementById('audit-actions').classList.remove('hidden');
  document.getElementById('audit-output').classList.add('hidden');

  const proofOutput = document.getElementById('proof-output');
  if (proofOutput) {
    proofOutput.textContent = JSON.stringify(
      {
        request_id: result.request_id,
        proof_audit_hash: result.proof_audit_hash,
        replay_warning: result.replay_warning,
        audit_recorded: result.audit_recorded,
        crypto_proof_metadata: result.crypto_proof_metadata,
      },
      null,
      2,
    );
  }

  document.getElementById('proof-section').classList.add('hidden');
  setText(
    'toggle-proof',
    'Show Technical Academic Proof (ZKP Metadata)',
  );
}

async function renderAuditOutput(loader) {
  const output = document.getElementById('audit-output');
  output.classList.remove('hidden');
  output.textContent = 'Loading...';
  try {
    const data = await loader();
    output.textContent = JSON.stringify(data, null, 2);
  } catch (err) {
    output.textContent = String(err);
  }
}

document.getElementById('toggle-proof').addEventListener('click', () => {
  const section = document.getElementById('proof-section');
  const btn = document.getElementById('toggle-proof');
  const hidden = section.classList.toggle('hidden');
  btn.textContent = hidden
    ? 'Show Technical Academic Proof (ZKP Metadata)'
    : 'Hide Technical Academic Proof (ZKP Metadata)';
});

document.getElementById('api-key').addEventListener('input', (e) => {
  localStorage.setItem('aml_api_key', e.target.value.trim());
});

document.getElementById('view-audit-proof').addEventListener('click', () => {
  if (!latestResult?.request_id) return;
  renderAuditOutput(() => fetchJson(`/api/v1/proofs/${latestResult.request_id}`));
});

document.getElementById('verify-audit-hash').addEventListener('click', () => {
  if (!latestResult?.request_id) return;
  renderAuditOutput(() =>
    fetchJson(`/api/v1/proofs/${latestResult.request_id}/verify`, {
      method: 'POST',
    }),
  );
});

document.getElementById('view-ledger').addEventListener('click', () => {
  renderAuditOutput(() => fetchJson('/api/v1/ledger/commitments'));
});

document.querySelectorAll('input[name="scenario"]').forEach((radio) => {
  radio.addEventListener('change', async (e) => {
    const scenarioId = e.target.value;
    document.getElementById('result-panel').classList.add('hidden');
    activeSetup = null;
    try {
      await loadDemoSetup(scenarioId);
    } catch (err) {
      setText('scenario-summary', String(err));
    }
  });
});

document.getElementById('verify-form').addEventListener('submit', async (e) => {
  e.preventDefault();

  const loading = document.getElementById('loading');
  const runBtn = document.getElementById('run-check');
  loading.classList.remove('hidden');
  runBtn.disabled = true;
  runBtn.classList.add('opacity-60', 'cursor-not-allowed');

  try {
    const selected = document.querySelector('input[name="scenario"]:checked');
    const scenarioId = selected?.value ?? 'compliant';

    if (!activeSetup || activeSetup.scenario !== scenarioId) {
      await loadDemoSetup(scenarioId);
    }

    const amountCents = centsFromInput(document.getElementById('amount').value);

    const payload = {
      amount_cents: amountCents,
      scenario: activeSetup.scenario,
    };
    if (activeSetup.tx_id_commitment) {
      payload.tx_id_commitment = activeSetup.tx_id_commitment;
    }
    if (activeSetup.user_key) {
      payload.user_key = activeSetup.user_key;
    }

    const res = await fetch(`${API}/api/v1/verify`, {
      method: 'POST',
      headers: apiHeaders({ 'Content-Type': 'application/json' }),
      body: JSON.stringify(payload),
    });

    if (!res.ok) {
      const err = await res.json();
      throw new Error(err.error ?? 'Verification request failed');
    }

    const result = await res.json();
    renderResult(result);
  } catch (err) {
    document.getElementById('result-panel').classList.remove('hidden');
    document.getElementById('decision-badge').textContent = 'ERROR';
    document.getElementById('decision-badge').className =
      'px-4 py-1.5 rounded-full text-sm font-bold bg-amber-900 text-amber-300';
    setText('result-message', String(err));
  } finally {
    loading.classList.add('hidden');
    runBtn.disabled = false;
    runBtn.classList.remove('opacity-60', 'cursor-not-allowed');
  }
});

loadConfig();
loadDemoSetup('compliant');

const savedApiKey = localStorage.getItem('aml_api_key');
if (savedApiKey) {
  document.getElementById('api-key').value = savedApiKey;
}
