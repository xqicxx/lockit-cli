import { invoke } from '@tauri-apps/api/core';
import './styles.css';

type CredentialType =
  | 'api_key'
  | 'github'
  | 'account'
  | 'coding_plan'
  | 'password'
  | 'token'
  | 'ssh_key'
  | 'custom';

type RedactedCredential = {
  id: string;
  name: string;
  type: CredentialType;
  service: string;
  key: string;
  fields: Record<string, string>;
  metadata: Record<string, string>;
  updated_at: string;
};

type AppState = {
  unlocked: boolean;
  selectedId: string | null;
  query: string;
  credentials: RedactedCredential[];
  syncStatus: string;
  detailSecret: string | null;
  error: string | null;
};

const state: AppState = {
  unlocked: false,
  selectedId: null,
  query: '',
  credentials: [],
  syncStatus: 'NOT_CONFIGURED',
  detailSecret: null,
  error: null,
};

const sampleCredentials: RedactedCredential[] = [
  {
    id: 'sample-openai',
    name: 'OPENAI_API_KEY',
    type: 'api_key',
    service: 'openai',
    key: 'default',
    fields: { value: 'sk-p••••7890' },
    metadata: {},
    updated_at: '2026-04-28T14:41:00Z',
  },
  {
    id: 'sample-github',
    name: 'GITHUB_PAT',
    type: 'token',
    service: 'github',
    key: 'default',
    fields: { value: 'ghp_••••wxyz' },
    metadata: {},
    updated_at: '2026-04-28T13:09:00Z',
  },
  {
    id: 'sample-qwen',
    name: 'QWEN_COOKIE',
    type: 'coding_plan',
    service: 'qwen',
    key: 'bailian',
    fields: { value: 'sess••••7781' },
    metadata: {},
    updated_at: '2026-04-27T22:11:00Z',
  },
];

function selectedCredential(): RedactedCredential | null {
  return state.credentials.find((item) => item.id === state.selectedId) ?? state.credentials[0] ?? null;
}

function filteredCredentials(): RedactedCredential[] {
  const query = state.query.trim().toLowerCase();
  if (!query) return state.credentials;
  return state.credentials.filter((item) => [item.name, item.service, item.type, item.key].join(' ').toLowerCase().includes(query));
}

async function loadVault() {
  if (!state.unlocked) return;
  try {
    state.credentials = await invoke<RedactedCredential[]>('list_credentials');
    state.selectedId = state.credentials[0]?.id ?? null;
    state.syncStatus = await invoke<string>('sync_status');
  } catch (error) {
    state.error = String(error);
  }
  render();
}

async function unlock() {
  const password = (document.querySelector<HTMLInputElement>('#password')?.value ?? '').trim();
  if (!password) return;
  try {
    await invoke('unlock_vault', { password });
    state.unlocked = true;
    state.error = null;
    await loadVault();
  } catch (error) {
    state.error = String(error);
    render();
  }
}

async function createVault() {
  const password = (document.querySelector<HTMLInputElement>('#password')?.value ?? '').trim();
  if (!password) return;
  try {
    await invoke('init_vault', { password });
    await unlock();
  } catch (error) {
    state.error = String(error);
    render();
  }
}

async function revealSelected() {
  const selected = selectedCredential();
  if (!selected) return;
  if (selected.id.startsWith('sample-')) {
    state.detailSecret = 'Demo secret; unlock a real vault to reveal actual values.';
    render();
    return;
  }
  try {
    state.detailSecret = await invoke<string>('reveal_secret', { id: selected.id, field: 'value' });
  } catch (error) {
    state.error = String(error);
  }
  render();
}

function renderUnlock() {
  return `
    <section class="panel unlock-panel">
      <div class="topline">
        <div class="brand-lockup"><img class="brand-icon" src="/lockit-icon.svg" alt="" /><strong>LOCKIT</strong></div>
        <span>VAULT_V2</span>
      </div>
      <div class="hero-label">SECURE PANEL</div>
      <h1>Small vault.<br/>Sharp edges.</h1>
      <p class="muted">Unlock with your master password. Google tokens and sync configuration are stored outside the vault in the OS credential store.</p>
      <input id="password" class="input" type="password" placeholder="Master password" autocomplete="current-password" />
      <div class="button-row">
        <button id="unlock" class="button primary">UNLOCK</button>
        <button id="create" class="button secondary">CREATE</button>
      </div>
      ${state.error ? `<p class="error">${escapeHtml(state.error)}</p>` : ''}
    </section>
  `;
}

function renderVault() {
  const items = filteredCredentials();
  const selected = selectedCredential();
  const value = selected?.fields.value ?? '••••';
  return `
    <section class="panel vault-panel">
      <header class="topline">
        <div class="brand-lockup"><img class="brand-icon" src="/lockit-icon.svg" alt="" /><strong>LOCKIT</strong></div>
        <span class="sync-state">${escapeHtml(state.syncStatus)}</span>
      </header>
      <div class="search-block">
        <label>VAULT / SEARCH</label>
        <input id="query" class="input" value="${escapeHtml(state.query)}" placeholder="Search secret..." />
      </div>
      <div class="button-row compact">
        <button id="add" class="button primary">ADD</button>
        <button id="sync" class="button secondary">SYNC</button>
      </div>
      <nav class="tabs"><span class="active">ALL</span><span>API</span><span>ACCOUNT</span><span>SSH</span></nav>
      <div class="content-split ${selected ? 'has-detail' : ''}">
        <div class="list">
          ${items.map((item) => `
            <button class="secret-row ${item.id === selected?.id ? 'selected' : ''}" data-id="${item.id}">
              <span>${escapeHtml(item.name)}</span>
              <small>${escapeHtml(item.type)} · ${escapeHtml(item.service)}</small>
            </button>
          `).join('')}
        </div>
        ${selected ? `
          <article class="detail">
            <button id="back" class="back">← VAULT</button>
            <h2>${escapeHtml(selected.name.replaceAll('_', ' '))}</h2>
            <div class="secret-value">${escapeHtml(state.detailSecret ?? value)}</div>
            <div class="button-row compact">
              <button id="reveal" class="button primary">REVEAL</button>
              <button id="copy" class="button secondary">COPY</button>
            </div>
            <dl>
              <dt>TYPE</dt><dd>${escapeHtml(selected.type)}</dd>
              <dt>SERVICE</dt><dd>${escapeHtml(selected.service)}</dd>
              <dt>KEY</dt><dd>${escapeHtml(selected.key)}</dd>
            </dl>
          </article>
        ` : ''}
      </div>
      <footer><span>${state.credentials.length} ITEMS</span><span>${escapeHtml(state.syncStatus)}</span></footer>
      ${state.error ? `<p class="error">${escapeHtml(state.error)}</p>` : ''}
    </section>
  `;
}

function render() {
  const app = document.querySelector('#app')!;
  if (!state.unlocked && state.credentials.length === 0) {
    state.credentials = sampleCredentials;
  }
  app.innerHTML = state.unlocked ? renderVault() : renderUnlock();
  bindEvents();
}

function bindEvents() {
  document.querySelector('#unlock')?.addEventListener('click', unlock);
  document.querySelector('#create')?.addEventListener('click', createVault);
  document.querySelector('#reveal')?.addEventListener('click', revealSelected);
  document.querySelector('#copy')?.addEventListener('click', async () => {
    const selected = selectedCredential();
    if (!selected) return;
    const value = state.detailSecret ?? selected.fields.value ?? '';
    await navigator.clipboard.writeText(value);
    if (!selected.id.startsWith('sample-')) {
      await invoke('copy_secret_event', { id: selected.id, field: 'value' });
    }
  });
  document.querySelector('#sync')?.addEventListener('click', async () => {
    state.syncStatus = await invoke<string>('sync_status');
    render();
  });
  document.querySelector('#query')?.addEventListener('input', (event) => {
    state.query = (event.target as HTMLInputElement).value;
    render();
  });
  document.querySelectorAll<HTMLButtonElement>('.secret-row').forEach((row) => {
    row.addEventListener('click', () => {
      state.selectedId = row.dataset.id ?? null;
      state.detailSecret = null;
      render();
    });
  });
  document.querySelector('#back')?.addEventListener('click', () => {
    state.selectedId = null;
    state.detailSecret = null;
    render();
  });
}

function escapeHtml(value: string) {
  return value.replace(/[&<>'"]/g, (char) => ({
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '\'': '&#39;',
    '"': '&quot;',
  }[char] ?? char));
}

render();
