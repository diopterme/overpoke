const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

const GRACE_CHOICES = [0, 5, 10, 15, 20, 30];
const WEEKDAYS = ['일', '월', '화', '수', '목', '금', '토'];
const $ = id => document.getElementById(id);

// 마지막으로 저장된(또는 불러온) 값. 폼과 비교해 변경 여부를 판단한다
let saved = null;
let holidays = [];

const pad = n => String(n).padStart(2, '0');
const localDate = d => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const hhmm = t => t.slice(0, 5);
const addMinutes = (t, min) => {
  const total = Number(t.slice(0, 2)) * 60 + Number(t.slice(3, 5)) + min;
  return `${pad(Math.floor(total / 60) % 24)}:${pad(total % 60)}`;
};

// 백엔드는 "09:00:00"으로 저장하고 time 입력칸은 "09:00"을 쓴다. 비교를 위해 맞춘다
const normalize = view => ({
  settings: {
    ...view.settings,
    work_start: hhmm(view.settings.work_start),
    work_end: hhmm(view.settings.work_end),
  },
  autostart: view.autostart,
});

function readForm() {
  return {
    settings: {
      ...saved.settings, // 폼에 없는 필드는 그대로 보존
      work_start: $('workStart').value,
      work_end: $('workEnd').value,
      end_grace_min: Number($('grace').value),
      popup_position: document.querySelector('input[name=pos]:checked').value,
      ntfy: {
        enabled: $('ntfyOn').checked,
        server: $('ntfyServer').value,
        topic: $('ntfyTopic').value,
        token: $('ntfyToken').value,
      },
      telegram: {
        enabled: $('telegramOn').checked,
        bot_token: $('telegramToken').value,
        chat_id: $('telegramChat').value,
      },
      extra_holidays: holidays,
    },
    autostart: $('autostart').checked,
  };
}

const isDirty = () => saved !== null && JSON.stringify(readForm()) !== JSON.stringify(saved);

function fillForm({ settings, autostart }) {
  $('workStart').value = settings.work_start;
  $('workEnd').value = settings.work_end;
  const choices = GRACE_CHOICES.includes(settings.end_grace_min)
    ? GRACE_CHOICES
    : [...GRACE_CHOICES, settings.end_grace_min].sort((a, b) => a - b); // 파일을 직접 고친 값도 보이게
  $('grace').replaceChildren(...choices.map(m => new Option(`${m}분`, String(m))));
  $('grace').value = String(settings.end_grace_min);
  document.querySelector(`input[name=pos][value=${settings.popup_position}]`).checked = true;
  $('ntfyOn').checked = settings.ntfy.enabled;
  $('ntfyServer').value = settings.ntfy.server;
  $('ntfyTopic').value = settings.ntfy.topic;
  $('ntfyToken').value = settings.ntfy.token;
  $('telegramOn').checked = settings.telegram.enabled;
  $('telegramToken').value = settings.telegram.bot_token;
  $('telegramChat').value = settings.telegram.chat_id;
  $('autostart').checked = autostart;
  holidays = [...settings.extra_holidays];
  renderHolidays();
  syncToggles();
}

// 백엔드 Settings::validate와 같은 규칙. 칸별로 문구를 보여 주려고 프런트에서도 검사한다
function validate({ settings: s }) {
  const errors = {};
  if (!s.work_start || !s.work_end) errors.workTime = '시각을 입력하세요';
  else if (s.work_start >= s.work_end) errors.workTime = '근무 시작은 종료보다 빨라야 합니다';
  if (s.ntfy.enabled) {
    if (!/^https?:\/\//.test(s.ntfy.server)) errors.ntfy = '서버 주소는 http:// 또는 https:// 로 시작해야 합니다';
    else if (!s.ntfy.topic.trim()) errors.ntfy = '토픽을 입력하세요';
  }
  if (s.telegram.enabled && (!s.telegram.bot_token.trim() || !s.telegram.chat_id.trim())) {
    errors.telegram = '봇 토큰과 chat_id를 입력하세요';
  }
  return errors;
}

function showErrors(errors) {
  document.querySelectorAll('[data-err]').forEach(el => { el.textContent = errors[el.dataset.err] || ''; });
}

function setSaveMsg(text, kind) {
  $('saveMsg').textContent = text;
  $('saveMsg').className = kind || '';
}

function showPage(name) {
  document.querySelectorAll('nav [data-page]').forEach(b => {
    if (b.dataset.page === name) b.setAttribute('aria-current', 'page');
    else b.removeAttribute('aria-current');
  });
  document.querySelectorAll('main > section').forEach(sec => { sec.hidden = sec.id !== `p-${name}`; });
}

function syncToggles() {
  document.querySelectorAll('[data-toggle]').forEach(box => { $(box.dataset.toggle).hidden = !box.checked; });
}

function onFormChange() {
  syncToggles();
  $('save').disabled = !isDirty();
  if (isDirty()) setSaveMsg('');
}

function renderHolidays() {
  const today = localDate(new Date());
  const items = holidays.map(d => {
    const li = document.createElement('li');
    li.classList.toggle('past', d < today);
    const label = document.createElement('span');
    label.textContent = `${d} (${WEEKDAYS[new Date(`${d}T00:00`).getDay()]})`;
    const del = document.createElement('button');
    del.type = 'button';
    del.textContent = '✕';
    del.title = '삭제';
    del.addEventListener('click', () => {
      holidays = holidays.filter(x => x !== d);
      renderHolidays();
      onFormChange();
    });
    li.append(label, del);
    return li;
  });
  $('holidayList').replaceChildren(...items);
}

function formatNext(next) {
  if (!next) return '다음 알림: 없음';
  const [date, time] = next.split('T');
  const now = new Date();
  const tomorrow = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
  const hm = time.slice(0, 5);
  if (date === localDate(now)) return `다음 알림: 오늘 ${hm}`;
  if (date === localDate(tomorrow)) return `다음 알림: 내일 ${hm}`;
  return `다음 알림: ${Number(date.slice(5, 7))}월 ${Number(date.slice(8, 10))}일 ${hm}`;
}

async function loadStatus() {
  try {
    const status = await invoke('get_status');
    const s = saved.settings;
    $('todayText').textContent = status.day_off
      ? '휴일 · 24시간 매시 알림'
      : `평일 · ${addMinutes(s.work_end, s.end_grace_min)}부터 다음 날 ${s.work_start}까지 매시 알림`;
    $('nextText').textContent = status.muted ? '' : formatNext(status.next_alert);
    $('muteText').textContent = status.muted ? '오늘은 알림을 껐습니다 (자정에 다시 켜짐)' : '알림 켜짐';
    $('muteText').className = status.muted ? 'muted-on' : '';
    $('unmute').hidden = !status.muted;
  } catch (e) {
    $('todayText').textContent = `상태를 불러오지 못했습니다: ${e}`;
  }
}

async function save() {
  const form = readForm();
  const errors = validate(form);
  showErrors(errors);
  if (Object.keys(errors).length > 0) {
    setSaveMsg('입력값을 확인하세요', 'bad');
    // 오류 칸이 다른 메뉴에 있으면 보이지 않으므로 그 페이지로 옮긴다
    const firstErr = document.querySelector('[data-err]:not(:empty)');
    showPage(firstErr.closest('main > section').id.slice('p-'.length));
    firstErr.scrollIntoView({ block: 'center' });
    return;
  }
  $('save').disabled = true;
  try {
    await invoke('save_settings', form);
    saved = form;
    setSaveMsg('저장됨', 'ok');
    loadStatus();
  } catch (e) {
    setSaveMsg(String(e), 'bad');
    $('save').disabled = !isDirty();
  }
}

async function testPush(channel, button) {
  const out = $(`${channel}Result`);
  const { settings } = readForm();
  // 누른 채널 하나만 켠 사본으로 보낸다
  const only = {
    ...settings,
    ntfy: { ...settings.ntfy, enabled: channel === 'ntfy' },
    telegram: { ...settings.telegram, enabled: channel === 'telegram' },
  };
  button.disabled = true;
  out.className = 'result';
  out.textContent = '보내는 중…';
  try {
    await invoke('test_push', { settings: only });
    out.textContent = '✓ 보냈습니다. 휴대폰을 확인하세요';
    out.classList.add('ok');
  } catch (e) {
    out.textContent = `✕ ${e}`;
    out.classList.add('bad');
  } finally {
    button.disabled = false;
  }
}

function addHoliday() {
  const d = $('holidayInput').value;
  if (!d || holidays.includes(d)) return;
  holidays = [...holidays, d].sort();
  $('holidayInput').value = '';
  renderHolidays();
  onFormChange();
}

async function init() {
  document.querySelectorAll('nav [data-page]').forEach(b => b.addEventListener('click', () => showPage(b.dataset.page)));
  showPage('status');
  try {
    saved = normalize(await invoke('get_settings'));
  } catch (e) {
    setSaveMsg(`설정을 불러오지 못했습니다: ${e}`, 'bad');
    return;
  }
  fillForm(saved);
  loadStatus();

  document.addEventListener('input', onFormChange);
  document.addEventListener('change', onFormChange);
  $('save').addEventListener('click', save);
  $('holidayAdd').addEventListener('click', addHoliday);
  $('unmute').addEventListener('click', () => invoke('unmute_today').catch(e => setSaveMsg(String(e), 'bad')));
  $('preview').addEventListener('click', () => {
    invoke('preview_popup', { position: readForm().settings.popup_position }).catch(e => setSaveMsg(String(e), 'bad'));
  });
  document.querySelectorAll('[data-test]').forEach(b => b.addEventListener('click', () => testPush(b.dataset.test, b)));
  document.querySelectorAll('[data-reveal]').forEach(b => b.addEventListener('click', () => {
    const input = $(b.dataset.reveal);
    const isHidden = input.type === 'password';
    input.type = isHidden ? 'text' : 'password';
    b.textContent = isHidden ? '숨기기' : '보기';
  }));

  const win = getCurrentWindow();
  win.onCloseRequested(e => {
    if (!isDirty()) return;
    e.preventDefault();
    $('closeDialog').showModal();
  });
  $('closeCancel').addEventListener('click', () => $('closeDialog').close());
  $('closeDiscard').addEventListener('click', () => win.destroy());
  listen('status-changed', loadStatus);
}

init();
